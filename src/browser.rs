use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

use base64::prelude::*;
use bytes::{BufMut, BytesMut};
use headless_chrome::protocol::cdp::{
    types::Event,
    Input::{self, DispatchMouseEvent, DispatchMouseEventTypeOption},
    Page,
};
use image::{GenericImageView, ImageFormat, RgbaImage, SubImage};
use rayon::prelude::*;

use crate::{
    protocol::{ChunkPos, ClientCommand, MouseButton, MouseEvent, ServerCommand},
    stream::Stream,
};

pub const CHUNK_SIZE: u32 = 128;

#[derive(Clone)]
pub struct Browser {
    inner: Arc<BrowserInner>,
}

struct BrowserInner {
    _browser: headless_chrome::Browser,
    command_tx: crossbeam::channel::Sender<ServerCommand>,
    tab: Arc<headless_chrome::Tab>,
    curr_frame: Mutex<Option<Frame>>,
    curr_frame_cvar: Condvar,
    prev_frame: Mutex<Option<Frame>>,
    buttons: Mutex<u32>,
    curr_mouse_dispatch: Mutex<Option<DispatchMouseEvent>>,
    curr_mouse_dispatch_cvar: Condvar,
}

#[derive(Clone)]
struct Frame {
    image: RgbaImage,
}

#[derive(Clone)]
struct Chunk<'a> {
    image: SubImage<&'a RgbaImage>,
    pos: ChunkPos,
}

impl Browser {
    pub fn start(
        stream: Stream,
        launch_options: headless_chrome::LaunchOptions,
    ) -> anyhow::Result<Self> {
        let browser = headless_chrome::Browser::new(launch_options)?;
        let tab = browser.new_tab()?;

        println!("created tab");

        let (command_tx, command_rx) = crossbeam::channel::bounded(16);

        let browser = Browser {
            inner: Arc::new(BrowserInner {
                _browser: browser,
                command_tx,
                tab,
                curr_frame: Mutex::new(None),
                curr_frame_cvar: Condvar::new(),
                prev_frame: Mutex::new(None),
                buttons: Mutex::new(0),
                curr_mouse_dispatch: Mutex::new(None),
                curr_mouse_dispatch_cvar: Condvar::new(),
            }),
        };

        thread::spawn({
            let browser = browser.clone();
            let mut stream = stream.clone();

            move || loop {
                let command = ClientCommand::read(&mut stream).unwrap();
                browser.inner.handle_command(&command).unwrap();
            }
        });

        thread::spawn({
            let mut stream = stream.clone();
            move || loop {
                let command = command_rx.recv().unwrap();
                command.write(&mut stream).unwrap();
            }
        });

        thread::spawn({
            let browser = browser.clone();

            move || loop {
                let dispatch = browser
                    .inner
                    .curr_mouse_dispatch_cvar
                    .wait_while(
                        browser.inner.curr_mouse_dispatch.lock().unwrap(),
                        |dispatch| dispatch.is_none(),
                    )
                    .unwrap()
                    .take()
                    .unwrap();

                browser.inner.tab.call_method(dispatch).unwrap();
            }
        });

        thread::spawn({
            let browser = browser.clone();

            move || loop {
                let frame = browser
                    .inner
                    .curr_frame_cvar
                    .wait_while(browser.inner.curr_frame.lock().unwrap(), |frame| {
                        frame.is_none()
                    })
                    .unwrap()
                    .take()
                    .unwrap();

                browser.inner.handle_frame(frame).unwrap();
            }
        });

        browser.inner.tab.add_event_listener(Arc::new({
            let browser = browser.clone();

            move |event: &Event| {
                #[allow(clippy::single_match)]
                match event {
                    Event::PageScreencastFrame(event) => {
                        let data = BASE64_STANDARD.decode(&event.params.data).unwrap();

                        let image =
                            image::load_from_memory_with_format(&data, ImageFormat::Png).unwrap();
                        let frame = Frame::new(image.to_rgba8());

                        println!(
                            "received frame {}x{}",
                            frame.image.width(),
                            frame.image.height()
                        );

                        browser
                            .inner
                            .tab
                            .ack_screencast(event.params.session_id)
                            .unwrap();

                        browser.inner.curr_frame.lock().unwrap().replace(frame);
                        browser.inner.curr_frame_cvar.notify_all();
                    }
                    _ => {}
                }
            }
        }))?;

        browser.inner.tab.start_screencast(
            Some(Page::StartScreencastFormatOption::Png),
            None,
            Some(1024),
            Some(1024),
            None,
        )?;

        Ok(browser)
    }
}

impl Frame {
    fn new(image: RgbaImage) -> Self {
        Frame { image }
    }

    fn chunk(&self, chunk_pos: ChunkPos) -> Chunk {
        let width = self.image.width();
        let height = self.image.height();

        let offset_x = chunk_pos.x as u32 * CHUNK_SIZE;
        let offset_y = chunk_pos.y as u32 * CHUNK_SIZE;
        let chunk_width = CHUNK_SIZE.min(width - offset_x);
        let chunk_height = CHUNK_SIZE.min(height - offset_y);

        Chunk {
            image: self
                .image
                .view(offset_x, offset_y, chunk_width, chunk_height),
            pos: chunk_pos,
        }
    }

    fn chunks(&self) -> impl Iterator<Item = Chunk> {
        let width = self.image.width();
        let height = self.image.height();

        let chunks_x = width.div_ceil(CHUNK_SIZE) as u8;
        let chunks_y = height.div_ceil(CHUNK_SIZE) as u8;

        (0..chunks_x).flat_map(move |x| (0..chunks_y).map(move |y| self.chunk(ChunkPos::new(x, y))))
    }

    fn par_chunks(&self) -> impl IndexedParallelIterator<Item = Chunk> {
        self.chunks().collect::<Vec<_>>().into_par_iter()
    }
}

impl BrowserInner {
    fn handle_command(&self, command: &ClientCommand) -> anyhow::Result<()> {
        match dbg!(command) {
            ClientCommand::Reset => {
                let frame = self
                    .curr_frame
                    .lock()
                    .unwrap()
                    .take()
                    .or_else(|| self.prev_frame.lock().unwrap().take());

                if let Some(frame) = frame {
                    self.handle_frame(frame)?;
                }
            }
            ClientCommand::Load { url } => {
                self.tab.navigate_to(url)?;
            }
            ClientCommand::Mouse { x, y, event } => {
                let mut buttons = self.buttons.lock().unwrap();
                match event {
                    MouseEvent::Pressed(MouseButton::Left) => {
                        *buttons |= 1;
                    }
                    MouseEvent::Pressed(MouseButton::Right) => {
                        *buttons |= 2;
                    }
                    MouseEvent::Released(MouseButton::Left) => {
                        *buttons &= !1;
                    }
                    MouseEvent::Released(MouseButton::Right) => {
                        *buttons &= !2;
                    }
                    _ => {}
                }
                let dispatch = DispatchMouseEvent {
                    Type: match event {
                        MouseEvent::Move => DispatchMouseEventTypeOption::MouseMoved,
                        MouseEvent::Pressed(_) => DispatchMouseEventTypeOption::MousePressed,
                        MouseEvent::Released(_) => DispatchMouseEventTypeOption::MouseReleased,
                    },
                    x: *x as f64,
                    y: *y as f64,
                    modifiers: None,
                    timestamp: None,
                    button: match event {
                        MouseEvent::Pressed(MouseButton::Left) => Some(Input::MouseButton::Left),
                        MouseEvent::Pressed(MouseButton::Right) => Some(Input::MouseButton::Right),
                        MouseEvent::Released(MouseButton::Left) => Some(Input::MouseButton::Left),
                        MouseEvent::Released(MouseButton::Right) => Some(Input::MouseButton::Right),
                        _ => {
                            if *buttons & 1 != 0 {
                                Some(Input::MouseButton::Left)
                            } else if *buttons & 2 != 0 {
                                Some(Input::MouseButton::Right)
                            } else {
                                None
                            }
                        }
                    },
                    buttons: Some(*buttons),
                    click_count: match event {
                        MouseEvent::Pressed(_) => Some(1),
                        MouseEvent::Released(_) => Some(1),
                        _ => None,
                    },
                    force: None,
                    tangential_pressure: None,
                    tilt_x: None,
                    tilt_y: None,
                    twist: None,
                    delta_x: None,
                    delta_y: None,
                    pointer_Type: None,
                };

                self.curr_mouse_dispatch.lock().unwrap().replace(dispatch);
                self.curr_mouse_dispatch_cvar.notify_all();
            }
        }

        Ok(())
    }

    fn dispatch(&self, command: ServerCommand) -> anyhow::Result<()> {
        self.command_tx.send(command)?;
        Ok(())
    }

    fn handle_frame(&self, frame: Frame) -> anyhow::Result<()> {
        println!(
            "handling frame {}x{}",
            frame.image.width(),
            frame.image.height()
        );

        let prev_frame = self.prev_frame.lock().unwrap().take();

        let prev_frame = if let Some(prev_frame) = prev_frame {
            if prev_frame.image.width() == frame.image.width()
                && prev_frame.image.height() == frame.image.height()
            {
                Some(prev_frame)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(prev_frame) = prev_frame {
            prev_frame
                .par_chunks()
                .zip_eq(frame.par_chunks())
                .filter(|(prev_chunk, chunk)| prev_chunk.image.pixels().ne(chunk.image.pixels()))
                .for_each(|(_, chunk)| {
                    self.send_chunk(chunk).unwrap();
                })
        } else {
            self.dispatch(ServerCommand::Resize {
                width: frame.image.width(),
                height: frame.image.height(),
            })?;

            frame.par_chunks().for_each(|chunk| {
                self.send_chunk(chunk).unwrap();
            })
        }

        self.prev_frame.lock().unwrap().replace(frame);

        Ok(())
    }

    fn send_chunk(&self, chunk: Chunk) -> anyhow::Result<()> {
        let mut data = BytesMut::with_capacity(
            chunk.image.width() as usize * chunk.image.height() as usize * 4,
        );

        for (_, _, pixel) in chunk.image.pixels() {
            data.put_slice(&pixel.0);
        }

        self.dispatch(ServerCommand::ChunkData {
            chunk_pos: chunk.pos,
            data: data.freeze(),
        })
    }
}

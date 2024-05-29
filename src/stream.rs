use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::{Buf, BytesMut};
use crossbeam::channel;

pub fn stream(max_buf_size: usize) -> (Stream, Stream) {
    let pipe_a = Pipe::new(max_buf_size);
    let pipe_b = Pipe::new(max_buf_size);

    (
        Stream {
            read: pipe_a.clone(),
            write: pipe_b.clone(),
        },
        Stream {
            read: pipe_b,
            write: pipe_a,
        },
    )
}

#[derive(Debug, Clone)]
pub struct Stream {
    read: Pipe,
    write: Pipe,
}

impl Stream {
    pub fn set_read_timeout(&mut self, read_timeout: Duration) {
        self.read.set_read_timeout(read_timeout);
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read.read(buf)
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.write.flush()
    }
}

#[derive(Debug, Clone)]
pub struct Pipe {
    buf: Arc<Mutex<BytesMut>>,
    max_buf_size: usize,
    read_timeout: Duration,
    read_tx: channel::Sender<()>,
    read_rx: channel::Receiver<()>,
    write_tx: channel::Sender<()>,
    write_rx: channel::Receiver<()>,
}

impl Pipe {
    pub fn new(max_buf_size: usize) -> Self {
        let (read_tx, read_rx) = channel::bounded(0);
        let (write_tx, write_rx) = channel::bounded(0);

        Pipe {
            buf: Arc::new(Mutex::new(BytesMut::with_capacity(max_buf_size))),
            max_buf_size,
            read_timeout: Duration::MAX,
            read_tx,
            read_rx,
            write_tx,
            write_rx,
        }
    }

    pub fn set_read_timeout(&mut self, read_timeout: Duration) {
        self.read_timeout = read_timeout;
    }
}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut self_buf = self.buf.lock().unwrap();
        if self_buf.has_remaining() {
            let max = self_buf.remaining().min(buf.len());
            self_buf.copy_to_slice(&mut buf[..max]);
            if max > 0 {
                let _ = self.write_tx.try_send(());
            }
            Ok(max)
        } else {
            drop(self_buf);
            match self.read_rx.recv_timeout(self.read_timeout) {
                Ok(_) => self.read(buf),
                Err(_) => Ok(0),
            }
        }
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut self_buf = self.buf.lock().unwrap();
        let available = self.max_buf_size - self_buf.len();
        if available > 0 {
            let len = buf.len().min(available);
            self_buf.extend_from_slice(&buf[..len]);
            let _ = self.read_tx.try_send(());

            Ok(len)
        } else {
            drop(self_buf);
            self.write_rx.recv().unwrap();
            self.write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

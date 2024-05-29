use std::{
    io::{self, Read},
    thread,
    time::Duration,
};

use byteorder::{LittleEndian, ReadBytesExt};
use headless_chrome::LaunchOptions;
use roblox_browser::{browser::Browser, stream};
use tiny_http::{Header, Response, Server, StatusCode};

fn main() {
    let (mut client_stream, server_stream) = stream::stream(4 * 1024 * 1024);
    client_stream.set_read_timeout(Duration::from_secs(15));

    let server = Server::http("0.0.0.0:3000").unwrap();

    Browser::start(
        server_stream,
        LaunchOptions::default_builder()
            .idle_browser_timeout(Duration::MAX)
            .build()
            .unwrap(),
    )
    .unwrap();

    for mut req in server.incoming_requests() {
        let mut client_stream = client_stream.clone();

        thread::spawn(move || {
            let mut reader = req.as_reader();
            let max = reader.read_u32::<LittleEndian>().unwrap() as usize;

            io::copy(&mut reader, &mut client_stream).unwrap();

            let mut buf = vec![0; max];
            let amt = if max > 0 {
                client_stream.read(&mut buf).unwrap()
            } else {
                0
            };

            req.respond(Response::new(
                StatusCode(200),
                vec![
                    Header::from_bytes(&b"Content-Type"[..], &b"application/octet-stream"[..])
                        .unwrap(),
                ],
                &buf[..amt],
                Some(amt),
                None,
            ))
            .unwrap();
        });
    }
}

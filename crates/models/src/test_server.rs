//! A local HTTP server for download tests: serves one file, honours `Range: bytes=N-`,
//! answers 416 past the end and can drop the first connection midway.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

pub struct Server {
    pub url: String,
    /// Start offset of every request (0 without `Range`).
    pub requests: Arc<Mutex<Vec<u64>>>,
}

impl Server {
    /// `drop_first_after`: the first response promises the whole file but closes after that many
    /// body bytes.
    pub fn start(data: Vec<u8>, drop_first_after: Option<usize>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model.bin", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        std::thread::spawn(move || {
            let mut drop_after = drop_first_after;
            for stream in listener.incoming() {
                let start = serve(stream.unwrap(), &data, drop_after.take());
                log.lock().unwrap().push(start);
            }
        });
        Self { url, requests }
    }
}

fn serve(mut stream: TcpStream, data: &[u8], drop_after: Option<usize>) -> u64 {
    let start = read_range_start(&stream);
    let len = data.len() as u64;
    if start >= len {
        let head = format!(
            "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(head.as_bytes()).unwrap();
        return start;
    }
    let body = &data[usize::try_from(start).unwrap()..];
    let head = if start == 0 {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
    } else {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{len}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            len - 1,
            body.len()
        )
    };
    stream.write_all(head.as_bytes()).unwrap();
    let sent = drop_after.map_or(body.len(), |n| n.min(body.len()));
    // The client may hang up first (cancel): a failed write is part of the scenario.
    let _ = stream.write_all(&body[..sent]);
    start
}

fn read_range_start(stream: &TcpStream) -> u64 {
    let mut reader = BufReader::new(stream);
    let mut start = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let line = line.trim_end();
        if line.is_empty() {
            return start;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("range: bytes=") {
            start = value.trim_end_matches('-').parse().unwrap();
        }
    }
}

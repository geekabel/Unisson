//! A tiny HTTP server for tests, so they never touch Spotify.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// One request, as the server received it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    /// `GET`, `PUT`...
    pub(crate) method: String,
    /// The path and the query.
    pub(crate) target: String,
    /// The value of the `Authorization` header, if any.
    pub(crate) authorization: Option<String>,
    /// The body, as text.
    pub(crate) body: String,
}

/// What the server answers: a status, a body, and extra headers.
pub(crate) type Answer = (u16, String, Vec<(String, String)>);

/// A local HTTP server that answers each request with what a handler says.
pub(crate) struct TestServer {
    /// Base address of the server, such as `http://127.0.0.1:40123`.
    pub(crate) url: String,
    seen: Arc<Mutex<Vec<Request>>>,
}

impl TestServer {
    /// Starts a server on a free local port.
    pub(crate) fn start(handler: impl Fn(&Request) -> Answer + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&seen);
        // The thread lives until the test process exits.
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                answer(stream, &handler, &recorder);
            }
        });
        Self { url, seen }
    }

    /// Returns the requests received so far, in order.
    pub(crate) fn requests(&self) -> Vec<Request> {
        self.seen.lock().unwrap().clone()
    }

    /// Returns `METHOD target` for each request received so far.
    pub(crate) fn lines(&self) -> Vec<String> {
        self.requests()
            .iter()
            .map(|request| format!("{} {}", request.method, request.target))
            .collect()
    }
}

fn answer(stream: TcpStream, handler: &impl Fn(&Request) -> Answer, seen: &Mutex<Vec<Request>>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let target = words.next().unwrap_or_default().to_owned();
    let mut length = 0;
    let mut authorization = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim().to_owned();
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.parse().unwrap_or(0),
                "authorization" => authorization = Some(value),
                _ => {}
            }
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let request = Request {
        method,
        target,
        authorization,
        body: String::from_utf8_lossy(&body).into_owned(),
    };
    let (status, body, headers) = handler(&request);
    seen.lock().unwrap().push(request);
    let mut extra = String::new();
    for (name, value) in headers {
        extra.push_str(&format!("{name}: {value}\r\n"));
    }
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = reader.get_mut().write_all(response.as_bytes());
}

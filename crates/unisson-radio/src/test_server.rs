//! A tiny HTTP server for tests, so they never touch the real directory.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// What the server saw: the request targets and the `User-Agent` headers.
#[derive(Default)]
struct Seen {
    requests: Vec<String>,
    user_agents: Vec<String>,
}

/// A local HTTP server that answers every GET request with a canned body.
pub(crate) struct TestServer {
    /// Base address of the server, such as `http://127.0.0.1:40123`.
    pub(crate) url: String,
    seen: Arc<Mutex<Seen>>,
}

impl TestServer {
    /// Starts a server on a free local port.
    ///
    /// `handler` receives the request target (path and query) and returns
    /// the status code and the body to send back.
    pub(crate) fn start(handler: impl Fn(&str) -> (u16, String) + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen::default()));
        let recorder = Arc::clone(&seen);
        // The thread lives until the test process exits.
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                answer(stream, &handler, &recorder);
            }
        });
        Self { url, seen }
    }

    /// Returns the targets of the requests received so far, in order.
    pub(crate) fn requests(&self) -> Vec<String> {
        self.seen.lock().unwrap().requests.clone()
    }

    /// Returns the `User-Agent` of the requests received so far, in order.
    pub(crate) fn user_agents(&self) -> Vec<String> {
        self.seen.lock().unwrap().user_agents.clone()
    }
}

/// Reads one request, records it, and sends the handler's answer.
fn answer(stream: TcpStream, handler: &impl Fn(&str) -> (u16, String), seen: &Mutex<Seen>) {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // A request line looks like `GET /path?query HTTP/1.1`.
    let target = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();

    let mut user_agent = String::new();
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header.trim().is_empty() => break,
            Ok(_) => {
                if let Some((name, value)) = header.split_once(':')
                    && name.eq_ignore_ascii_case("user-agent")
                {
                    user_agent = value.trim().to_owned();
                }
            }
        }
    }

    let (status, body) = handler(&target);
    {
        let mut seen = seen.lock().unwrap();
        seen.requests.push(target);
        seen.user_agents.push(user_agent);
    }
    let response = format!(
        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = reader.get_mut().write_all(response.as_bytes());
}

/// Returns the address of a local port where nothing listens.
pub(crate) fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    url
}

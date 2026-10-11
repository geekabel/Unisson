//! Telling a file from a continuous stream.

use std::time::Duration;

use ureq::Agent;

/// How long the question may take. Playback waits for the answer, so it
/// must stay short.
const TIMEOUT: Duration = Duration::from_secs(3);

/// Tells whether a network address serves a file with an end, rather than
/// a continuous stream such as a radio.
///
/// The address itself cannot tell: radio streams often end with `.mp3`.
/// The server can: it announces the size of a file (`Content-Length`),
/// and a stream has none. The server is asked for the headers only.
///
/// Returns `false` when the server does not answer, or does not say: a
/// stream is the safe guess, since it is what such addresses usually are.
pub fn is_finite_file(url: &str) -> bool {
    let agent: Agent = Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .into();
    let Ok(response) = agent.head(url).call() else {
        return false;
    };
    response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .is_some_and(|length| length > 0)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// Starts a server that answers one request with the given headers,
    /// and returns its address.
    fn serve_once(status: &'static str, headers: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/audio.mp3", listener.local_addr().unwrap());
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert!(request_line.starts_with("HEAD "), "{request_line}");
            // Read the rest of the request, up to the empty line.
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Connection: close\r\n\r\n"
            )
            .unwrap();
        });
        address
    }

    #[test]
    fn a_file_announces_its_size() {
        let address = serve_once(
            "200 OK",
            "Content-Type: audio/mpeg\r\nContent-Length: 4096\r\n",
        );
        assert!(is_finite_file(&address));
    }

    #[test]
    fn a_stream_has_no_size() {
        let address = serve_once("200 OK", "Content-Type: audio/mpeg\r\nicy-name: FIP\r\n");
        assert!(!is_finite_file(&address));
        let empty = serve_once("200 OK", "Content-Length: 0\r\n");
        assert!(!is_finite_file(&empty));
    }

    #[test]
    fn a_server_that_refuses_or_is_absent_counts_as_a_stream() {
        let refused = serve_once("405 Method Not Allowed", "Content-Length: 12\r\n");
        assert!(!is_finite_file(&refused));
        // Nothing listens on this port.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let closed = format!("http://{}/x.mp3", listener.local_addr().unwrap());
        drop(listener);
        assert!(!is_finite_file(&closed));
        assert!(!is_finite_file("not an address"));
    }
}

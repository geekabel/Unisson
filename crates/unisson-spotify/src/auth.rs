//! Connecting to a Spotify account: the "authorization code with PKCE" flow.
//!
//! # Design
//!
//! The flow is described in the
//! [Spotify documentation](https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow):
//!
//! 1. Unisson draws a random secret, the *verifier*, and sends only its
//!    SHA-256, the *challenge*, with the authorization request.
//! 2. The browser opens the Spotify page where the user accepts. Spotify
//!    then sends the browser back to the *redirect address* with a code.
//! 3. Unisson trades the code, with the verifier, for tokens.
//!
//! No client secret is involved, so nothing secret is built into the
//! program. The redirect address is a small web server of Unisson itself,
//! on the loopback address `127.0.0.1` and a port the system picks.
//! Spotify allows a loopback address registered without a port, and
//! accepts any port for it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use unisson_core::SourceError;

/// Where Spotify shows the authorization page.
pub const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";

/// The path of the redirect address. The address to register on the
/// Spotify dashboard is [`REDIRECT_TO_REGISTER`].
pub const REDIRECT_PATH: &str = "/callback";

/// The redirect address to register on the Spotify dashboard, without a
/// port: Spotify accepts any port for a loopback address.
pub const REDIRECT_TO_REGISTER: &str = "http://127.0.0.1/callback";

/// What Unisson asks to do: read what plays, and control it.
pub const SCOPES: &str = "user-read-playback-state user-modify-playback-state";

/// How long the user has to accept in the browser.
pub const CONSENT_TIMEOUT: Duration = Duration::from_secs(300);

/// The characters a verifier may hold.
const VERIFIER_LETTERS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// Tells whether a text looks like a client ID: 32 hexadecimal digits.
pub fn is_client_id(text: &str) -> bool {
    text.len() == 32 && text.chars().all(|letter| letter.is_ascii_hexdigit())
}

/// Draws a random text of `length` characters from [`VERIFIER_LETTERS`].
///
/// # Errors
///
/// Returns [`SourceError::Unavailable`] if the system gives no randomness.
fn random_text(length: usize) -> Result<String, SourceError> {
    let mut bytes = vec![0u8; length];
    getrandom::fill(&mut bytes)
        .map_err(|error| SourceError::Unavailable(format!("no randomness: {error}")))?;
    // 66 letters do not divide 256: drawing again above the largest
    // multiple keeps every letter equally likely.
    let limit = 256 - 256 % VERIFIER_LETTERS.len();
    let mut text = String::with_capacity(length);
    let mut spare = [0u8; 1];
    for mut byte in bytes {
        while usize::from(byte) >= limit {
            getrandom::fill(&mut spare)
                .map_err(|error| SourceError::Unavailable(format!("no randomness: {error}")))?;
            byte = spare[0];
        }
        text.push(char::from(
            VERIFIER_LETTERS[usize::from(byte) % VERIFIER_LETTERS.len()],
        ));
    }
    Ok(text)
}

/// The secret of one authorization, and what is sent of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    /// The secret, kept until the code is traded for tokens.
    pub verifier: String,
    /// Its SHA-256, in base64url without padding.
    pub challenge: String,
}

impl Pkce {
    /// Draws a new secret of 64 characters.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the system gives no
    /// randomness.
    pub fn new() -> Result<Self, SourceError> {
        Ok(Self::from_verifier(random_text(64)?))
    }

    /// Builds the challenge of a given secret.
    pub fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

/// Draws the `state` of an authorization: a random text Spotify sends back,
/// so that an answer that was not asked for is recognized.
///
/// # Errors
///
/// Returns [`SourceError::Unavailable`] if the system gives no randomness.
pub fn new_state() -> Result<String, SourceError> {
    random_text(24)
}

/// Builds the address of the authorization page.
///
/// # Errors
///
/// Returns [`SourceError::Invalid`] if the address cannot be built.
pub fn authorize_url(
    client_id: &str,
    redirect_uri: &str,
    pkce: &Pkce,
    state: &str,
) -> Result<String, SourceError> {
    url::Url::parse_with_params(
        AUTHORIZE_URL,
        [
            ("response_type", "code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("code_challenge_method", "S256"),
            ("code_challenge", &pkce.challenge),
            ("scope", SCOPES),
            ("state", state),
        ],
    )
    .map(String::from)
    .map_err(|error| SourceError::Invalid(error.to_string()))
}

/// Reads the code from the request the browser makes to the redirect
/// address, such as `/callback?code=AQB...&state=xyz`.
///
/// # Errors
///
/// - [`SourceError::Invalid`] if the request is not the expected one, or
///   its `state` is not the one sent;
/// - [`SourceError::Unavailable`] if the user refused, with the reason
///   Spotify gives.
pub fn read_callback(target: &str, expected_state: &str) -> Result<String, SourceError> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != REDIRECT_PATH {
        return Err(SourceError::Invalid(format!("unexpected request: {path}")));
    }
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (name, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match name.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        return Err(SourceError::Invalid(
            "the answer does not belong to this connection".into(),
        ));
    }
    if let Some(error) = error {
        return Err(SourceError::Unavailable(format!(
            "Spotify refused the connection: {error}"
        )));
    }
    code.filter(|code| !code.is_empty())
        .ok_or_else(|| SourceError::Invalid("the answer holds no code".into()))
}

/// The small web server the browser comes back to.
#[derive(Debug)]
pub struct Callback {
    listener: TcpListener,
}

impl Callback {
    /// Opens the server on the loopback address, on a port the system
    /// picks.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if no port can be opened.
    pub fn open() -> Result<Self, SourceError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| SourceError::Unavailable(format!("cannot open a port: {error}")))?;
        Ok(Self { listener })
    }

    /// Returns the redirect address, with the port of the server.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the port cannot be read.
    pub fn redirect_uri(&self) -> Result<String, SourceError> {
        let port = self
            .listener
            .local_addr()
            .map_err(|error| SourceError::Unavailable(error.to_string()))?
            .port();
        Ok(format!("http://127.0.0.1:{port}{REDIRECT_PATH}"))
    }

    /// Waits for the browser, and returns the code it brings.
    ///
    /// Requests that are not the answer (a browser asking for an icon, for
    /// instance) are answered and ignored. The browser gets a short page
    /// that says it can be closed.
    ///
    /// # Errors
    ///
    /// - [`SourceError::Network`] if nothing came in `timeout`;
    /// - the errors of [`read_callback`] for an answer that is not valid.
    pub fn wait_for_code(
        self,
        expected_state: &str,
        timeout: Duration,
        page: &CallbackPage,
    ) -> Result<String, SourceError> {
        let deadline = Instant::now() + timeout;
        self.listener
            .set_nonblocking(true)
            .map_err(|error| SourceError::Unavailable(error.to_string()))?;
        loop {
            if Instant::now() >= deadline {
                return Err(SourceError::Network(
                    "no answer from the browser in time".into(),
                ));
            }
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                Err(error) => return Err(SourceError::Unavailable(error.to_string())),
            };
            let Some(target) = read_target(&stream) else {
                continue;
            };
            if !target.starts_with(REDIRECT_PATH) {
                respond(stream, 404, "");
                continue;
            }
            let result = read_callback(&target, expected_state);
            let text = if result.is_ok() {
                page.success
            } else {
                page.failure
            };
            respond(stream, 200, &page_html(page.title, text));
            return result;
        }
    }
}

/// The words of the page the browser shows at the end, in the language of
/// the interface.
#[derive(Debug, Clone, Copy)]
pub struct CallbackPage {
    /// The title of the page.
    pub title: &'static str,
    /// What it says when the connection worked.
    pub success: &'static str,
    /// What it says when it did not.
    pub failure: &'static str,
}

/// Reads the target of the request line, such as `/callback?code=...`.
fn read_target(stream: &TcpStream) -> Option<String> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    // A request line is short: a very long one is not a browser's.
    reader.by_ref().take(8192).read_line(&mut line).ok()?;
    let mut words = line.split_whitespace();
    if words.next()? != "GET" {
        return None;
    }
    words.next().map(str::to_owned)
}

fn respond(mut stream: TcpStream, status: u16, html: &str) {
    let response = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{html}",
        if status == 200 { "OK" } else { "Not Found" },
        html.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Builds the page shown in the browser. The texts are those of Unisson,
/// never of the request: nothing from outside is written in it.
fn page_html(title: &str, text: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{font-family:sans-serif;max-width:32rem;margin:4rem auto;padding:0 1rem;\
         line-height:1.5}}</style></head><body><h1>{title}</h1><p>{text}</p></body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_matches_the_example_of_rfc_7636() {
        // RFC 7636, appendix B.
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(
            pkce.challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn a_verifier_is_long_and_uses_allowed_letters_only() {
        let first = Pkce::new().unwrap();
        let second = Pkce::new().unwrap();
        assert_eq!(first.verifier.len(), 64);
        assert!(
            first
                .verifier
                .bytes()
                .all(|byte| VERIFIER_LETTERS.contains(&byte))
        );
        assert_ne!(first.verifier, second.verifier);
    }

    #[test]
    fn the_authorization_address_carries_every_parameter() {
        let pkce = Pkce::from_verifier("v".repeat(43));
        let address = authorize_url(
            "0123456789abcdef0123456789abcdef",
            "http://127.0.0.1:43210/callback",
            &pkce,
            "st&te",
        )
        .unwrap();
        let parsed = url::Url::parse(&address).unwrap();
        let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
        assert!(address.starts_with(AUTHORIZE_URL));
        for (name, value) in [
            ("response_type", "code"),
            ("client_id", "0123456789abcdef0123456789abcdef"),
            ("redirect_uri", "http://127.0.0.1:43210/callback"),
            ("code_challenge_method", "S256"),
            ("code_challenge", pkce.challenge.as_str()),
            ("scope", SCOPES),
            ("state", "st&te"),
        ] {
            assert!(
                pairs.contains(&(name.to_owned(), value.to_owned())),
                "{name} in {address}"
            );
        }
    }

    #[test]
    fn the_answer_of_the_browser_is_read_and_checked() {
        assert_eq!(
            read_callback("/callback?code=AQB%2Bx&state=abc", "abc"),
            Ok("AQB+x".to_owned())
        );
        assert!(matches!(
            read_callback("/callback?code=AQB&state=other", "abc"),
            Err(SourceError::Invalid(_))
        ));
        assert!(matches!(
            read_callback("/callback?error=access_denied&state=abc", "abc"),
            Err(SourceError::Unavailable(message)) if message.contains("access_denied")
        ));
        assert!(matches!(
            read_callback("/favicon.ico", "abc"),
            Err(SourceError::Invalid(_))
        ));
        assert!(matches!(
            read_callback("/callback?state=abc", "abc"),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn a_client_id_is_32_hexadecimal_digits() {
        assert!(is_client_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_client_id("0123456789abcdef0123456789abcde"));
        assert!(!is_client_id("0123456789abcdef0123456789abcdeg"));
        assert!(!is_client_id("0123456789abcdef0123456789abcd&f"));
    }

    #[test]
    fn the_browser_is_answered_and_its_code_returned() {
        let callback = Callback::open().unwrap();
        let redirect = callback.redirect_uri().unwrap();
        assert!(redirect.starts_with("http://127.0.0.1:"));
        assert!(redirect.ends_with(REDIRECT_PATH));
        let page = CallbackPage {
            title: "Unisson",
            success: "Connected.",
            failure: "Not connected.",
        };
        let browser = std::thread::spawn(move || {
            let address = redirect
                .trim_start_matches("http://")
                .trim_end_matches(REDIRECT_PATH);
            // A browser often asks for an icon first.
            let mut icon = TcpStream::connect(address).unwrap();
            icon.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n")
                .unwrap();
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(b"GET /callback?code=xyz&state=abc HTTP/1.1\r\nHost: x\r\n\r\n")
                .unwrap();
            let mut answer = String::new();
            std::io::Read::read_to_string(&mut stream, &mut answer).unwrap();
            answer
        });

        let code = callback
            .wait_for_code("abc", Duration::from_secs(5), &page)
            .unwrap();

        assert_eq!(code, "xyz");
        let answer = browser.join().unwrap();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.contains("Connected."), "{answer}");
    }

    #[test]
    fn waiting_ends_when_the_user_never_comes_back() {
        let callback = Callback::open().unwrap();
        let page = CallbackPage {
            title: "",
            success: "",
            failure: "",
        };
        let result = callback.wait_for_code("abc", Duration::from_millis(200), &page);
        assert!(matches!(result, Err(SourceError::Network(_))));
    }
}

//! The real `Http`: a blocking client that follows redirects and speaks TLS.

use std::time::Duration;

use ureq::config::RedirectAuthHeaders;
use ureq::tls::{RootCerts, TlsConfig};

use crate::ports::{Http, HttpError};

const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BODY_BYTES: u64 = 16 * 1024 * 1024;

pub struct UreqHttp {
    agent: ureq::Agent,
    auth_header: Option<String>,
}

impl UreqHttp {
    /// `auth_header` is sent as `Authorization` when given; sanitize it first
    /// (see `domain::http_header`).
    #[must_use]
    pub fn new(auth_header: Option<String>) -> Self {
        Self::from_builder(ureq::Agent::config_builder(), auth_header)
    }

    /// Like [`Self::new`], but ignoring `HTTP_PROXY` and friends, so a test
    /// reaches its local server whatever the environment says.
    #[cfg(test)]
    fn unproxied(auth_header: Option<String>) -> Self {
        Self::from_builder(ureq::Agent::config_builder().proxy(None), auth_header)
    }

    fn from_builder(
        builder: ureq::config::ConfigBuilder<ureq::typestate::AgentScope>,
        auth_header: Option<String>,
    ) -> Self {
        // Like curl: the credentials follow a redirect to the same host only.
        // Like curl: trust the system certificate store, so a corporate CA
        // (a TLS-inspecting proxy) is honoured.
        let tls = TlsConfig::builder()
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let config = builder
            .tls_config(tls)
            .timeout_global(Some(TIMEOUT))
            .redirect_auth_headers(RedirectAuthHeaders::SameHost)
            .build();
        Self {
            agent: config.into(),
            auth_header,
        }
    }
}

fn transport(url: &str, message: impl ToString) -> HttpError {
    HttpError::Transport {
        url: url.to_owned(),
        message: message.to_string(),
    }
}

/// A body that is over the limit or not text is final; anything else that
/// goes wrong while reading it (a reset, say) is a transport error.
fn read_failure(url: &str, error: ureq::Error) -> HttpError {
    let unusable = match &error {
        ureq::Error::BodyExceedsLimit(_) => true,
        ureq::Error::Io(source) => source.kind() == std::io::ErrorKind::InvalidData,
        _ => false,
    };
    if unusable {
        HttpError::Body {
            url: url.to_owned(),
            message: error.to_string(),
        }
    } else {
        transport(url, error)
    }
}

impl Http for UreqHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        let mut request = self.agent.get(url);
        if let Some(value) = &self.auth_header {
            request = request.header("Authorization", value);
        }
        match request.call() {
            Ok(mut response) => response
                .body_mut()
                .with_config()
                .limit(MAX_BODY_BYTES)
                .read_to_string()
                .map_err(|error| read_failure(url, error)),
            Err(ureq::Error::StatusCode(code)) => Err(HttpError::Status {
                url: url.to_owned(),
                code,
            }),
            Err(error) => Err(transport(url, error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    use super::*;

    /// Serves each canned response to one connection, in order, and returns
    /// the raw requests it saw.
    fn serve(responses: Vec<String>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut seen = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap();
                seen.push(String::from_utf8_lossy(&buffer[..read]).into_owned());
                stream.write_all(response.as_bytes()).unwrap();
            }
            seen
        });
        (base, handle)
    }

    fn ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn fetches_the_body_as_text() {
        let (base, server) = serve(vec![ok("hello\nworld\n")]);
        let text = UreqHttp::unproxied(None).get_text(&format!("{base}/index.tab"));
        assert_eq!(text.unwrap(), "hello\nworld\n");
        let requests = server.join().unwrap();
        assert!(
            requests[0].starts_with("GET /index.tab HTTP/1.1"),
            "{}",
            requests[0]
        );
    }

    #[test]
    fn an_error_status_is_a_status_error() {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (base, server) = serve(vec![response.to_owned()]);
        let url = format!("{base}/missing");
        let error = UreqHttp::unproxied(None).get_text(&url).unwrap_err();
        assert_eq!(error, HttpError::Status { url, code: 404 });
        server.join().unwrap();
    }

    #[test]
    fn redirects_are_followed() {
        let location = "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (base, server) = serve(vec![location.to_owned(), ok("moved")]);
        let text = UreqHttp::unproxied(None).get_text(&format!("{base}/start"));
        assert_eq!(text.unwrap(), "moved");
        let requests = server.join().unwrap();
        assert!(requests[1].starts_with("GET /final "), "{}", requests[1]);
    }

    #[test]
    fn the_auth_header_is_sent_when_given() {
        let (base, server) = serve(vec![ok("secret")]);
        let http = UreqHttp::unproxied(Some("Bearer token123".to_owned()));
        assert_eq!(http.get_text(&format!("{base}/x")).unwrap(), "secret");
        let requests = server.join().unwrap();
        assert!(
            requests[0]
                .to_lowercase()
                .contains("authorization: bearer token123"),
            "{}",
            requests[0]
        );
    }

    #[test]
    fn the_auth_header_follows_a_redirect_to_the_same_host() {
        let location = "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (base, server) = serve(vec![location.to_owned(), ok("kept")]);
        let http = UreqHttp::unproxied(Some("Bearer token123".to_owned()));
        assert_eq!(http.get_text(&format!("{base}/start")).unwrap(), "kept");
        let requests = server.join().unwrap();
        assert!(
            requests[1]
                .to_lowercase()
                .contains("authorization: bearer token123"),
            "{}",
            requests[1]
        );
    }

    /// Answers one connection with `response`, sent as raw bytes.
    fn serve_bytes(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(&response);
        });
        base
    }

    fn reply_with(body: &[u8], declared: usize) -> Vec<u8> {
        let head =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n");
        [head.as_bytes(), body].concat()
    }

    #[test]
    fn a_body_that_is_not_text_is_a_body_error() {
        let base = serve_bytes(reply_with(&[0xff, 0xfe, 0xfd], 3));
        let error = UreqHttp::unproxied(None).get_text(&format!("{base}/x"));
        assert!(matches!(error, Err(HttpError::Body { .. })), "{error:?}");
    }

    #[test]
    fn a_body_over_the_limit_is_a_body_error() {
        let size = usize::try_from(MAX_BODY_BYTES).unwrap() + 1024;
        let base = serve_bytes(reply_with(&vec![b'a'; size], size));
        let error = UreqHttp::unproxied(None).get_text(&format!("{base}/x"));
        assert!(matches!(error, Err(HttpError::Body { .. })), "{error:?}");
    }

    #[test]
    fn a_refused_connection_is_a_transport_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x", listener.local_addr().unwrap());
        drop(listener);
        let error = UreqHttp::unproxied(None).get_text(&url).unwrap_err();
        assert!(matches!(error, HttpError::Transport { .. }), "{error:?}");
    }
}

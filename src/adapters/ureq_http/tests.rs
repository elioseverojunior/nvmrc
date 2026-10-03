use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
    let location =
        "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
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
    let location =
        "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
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
fn bytes_that_are_not_text_are_fetched_whole() {
    let base = serve_bytes(reply_with(&[0xff, 0x00, 0xfd], 3));
    let body = UreqHttp::unproxied(None).get_bytes(&format!("{base}/a.tgz"));
    assert_eq!(body.unwrap(), [0xff, 0x00, 0xfd]);
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

/// Answers one connection with a body of `chunks` bytes, one byte every
/// `pause`, so the whole reply takes `chunks * pause`.
fn serve_slowly(chunks: usize, pause: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 4096];
        let _ = stream.read(&mut buffer);
        let _ = stream.write_all(&reply_with(&[], chunks));
        for _ in 0..chunks {
            thread::sleep(pause);
            let _ = stream.write_all(b"z").and_then(|()| stream.flush());
        }
    });
    base
}

/// A text call bounded at 100 ms, against a reply that takes 300 ms.
const SHORT: Timeouts = Timeouts {
    text_call: Duration::from_millis(100),
    until_response: Duration::from_secs(5),
};
const CHUNKS: usize = 6;
const PAUSE: Duration = Duration::from_millis(50);

#[test]
fn a_download_may_take_longer_than_a_text_call() {
    let base = serve_slowly(CHUNKS, PAUSE);
    let http = UreqHttp::unproxied_within(SHORT, None);
    let body = http.get_bytes(&format!("{base}/node.tar.xz"));
    assert_eq!(body.unwrap(), vec![b'z'; CHUNKS]);
}

#[test]
fn a_text_call_that_takes_too_long_is_a_transport_error() {
    let base = serve_slowly(CHUNKS, PAUSE);
    let http = UreqHttp::unproxied_within(SHORT, None);
    let error = http.get_text(&format!("{base}/index.tab")).unwrap_err();
    assert!(matches!(error, HttpError::Transport { .. }), "{error:?}");
}

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::ports::{Http, HttpError};

/// Pages by URL: each answers with a fixed body, status or transport error, and
/// every request is recorded.
#[derive(Default)]
pub struct FakeHttp {
    responses: BTreeMap<String, Result<Vec<u8>, HttpError>>,
    requests: RefCell<Vec<String>>,
}

impl FakeHttp {
    #[must_use]
    pub fn with_body(self, url: &str, body: &str) -> Self {
        self.with_bytes(url, body.as_bytes())
    }

    #[must_use]
    pub fn with_bytes(mut self, url: &str, body: &[u8]) -> Self {
        self.responses.insert(url.to_owned(), Ok(body.to_vec()));
        self
    }

    #[must_use]
    pub fn with_status(mut self, url: &str, code: u16) -> Self {
        let error = HttpError::Status {
            url: url.to_owned(),
            code,
        };
        self.responses.insert(url.to_owned(), Err(error));
        self
    }

    /// The URLs requested so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        self.requests.borrow().clone()
    }
}

impl Http for FakeHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        let bytes = self.get_bytes(url)?;
        String::from_utf8(bytes).map_err(|error| HttpError::Body {
            url: url.to_owned(),
            message: error.to_string(),
        })
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
        self.requests.borrow_mut().push(url.to_owned());
        self.responses.get(url).cloned().unwrap_or_else(|| {
            Err(HttpError::Transport {
                url: url.to_owned(),
                message: "connection refused".to_owned(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_http_serves_bytes_and_refuses_to_read_them_as_text_when_they_are_not() {
        let http = FakeHttp::default().with_bytes("http://m/a.tgz", &[0, 255]);
        assert_eq!(http.get_bytes("http://m/a.tgz").unwrap(), [0, 255]);
        let error = http.get_text("http://m/a.tgz").unwrap_err();
        assert!(matches!(error, HttpError::Body { .. }));
    }

    #[test]
    fn fake_http_answers_by_url_and_records_requests() {
        let http = FakeHttp::default()
            .with_body("http://m/index.tab", "rows")
            .with_status("http://m/gone", 404);
        assert_eq!(http.get_text("http://m/index.tab").unwrap(), "rows");
        let status = http.get_text("http://m/gone").unwrap_err();
        assert_eq!(status.to_string(), "http://m/gone: HTTP 404");
        let unknown = http.get_text("http://m/other").unwrap_err();
        assert_eq!(unknown.to_string(), "http://m/other: connection refused");
        assert_eq!(http.requests().len(), 3);
    }
}

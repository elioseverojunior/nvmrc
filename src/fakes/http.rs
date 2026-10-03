use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::ports::{Http, HttpError};

/// Pages by URL: each answers with a fixed body, status or transport error, and
/// every request is recorded.
#[derive(Default)]
pub struct FakeHttp {
    responses: BTreeMap<String, Result<String, HttpError>>,
    requests: RefCell<Vec<String>>,
}

impl FakeHttp {
    #[must_use]
    pub fn with_body(mut self, url: &str, body: &str) -> Self {
        self.responses.insert(url.to_owned(), Ok(body.to_owned()));
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

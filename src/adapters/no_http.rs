use crate::ports::{Http, HttpError};

/// The `Http` of a `Context` that was not given one: it fetches nothing.
pub struct NoHttp;

fn unavailable(url: &str) -> HttpError {
    HttpError::Transport {
        url: url.to_owned(),
        message: "network access is not available".to_owned(),
    }
}

impl Http for NoHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        Err(unavailable(url))
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
        Err(unavailable(url))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetches_nothing() {
        let error = NoHttp.get_text("http://example.test/x").unwrap_err();
        assert_eq!(
            error.to_string(),
            "http://example.test/x: network access is not available"
        );
    }
}

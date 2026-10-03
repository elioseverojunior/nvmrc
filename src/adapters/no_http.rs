use crate::ports::{Http, HttpError};

/// The `Http` of a `Context` that was not given one: it fetches nothing.
pub struct NoHttp;

impl Http for NoHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        Err(HttpError::Transport {
            url: url.to_owned(),
            message: "network access is not available".to_owned(),
        })
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

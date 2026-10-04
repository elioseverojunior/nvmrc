use thiserror::Error;

/// Why a download failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HttpError {
    #[error("{url}: HTTP {code}")]
    Status { url: String, code: u16 },
    #[error("{url}: {message}")]
    Transport { url: String, message: String },
    /// The server answered, but the body is not usable (too large, or not
    /// text). Asking again would not change that.
    #[error("{url}: {message}")]
    Body { url: String, message: String },
}

pub trait Http {
    /// Fetches `url` and returns the body as it is (an archive, say).
    ///
    /// # Errors
    /// Fails on a non-success status, on a network error, or when the body is
    /// over the size limit.
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>;

    /// Fetches `url` and returns the body as text.
    ///
    /// # Errors
    /// Fails on a non-success status, on a network error, or when the body is
    /// not text.
    fn get_text(&self, url: &str) -> Result<String, HttpError>;
}

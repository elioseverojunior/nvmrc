use thiserror::Error;

/// The most a text body (`get_text`, `get_secure_text`) may hold; a bigger
/// one is a [`HttpError::Body`]. Fetch a bigger document as bytes.
pub const MAX_TEXT_BYTES: u64 = 16 * 1024 * 1024;

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
    /// not text or over [`MAX_TEXT_BYTES`].
    fn get_text(&self, url: &str) -> Result<String, HttpError>;

    /// Like [`Self::get_text`], but `url` and every redirect it leads to must
    /// be https: a policy fetched over a channel that can be altered is not a
    /// policy.
    ///
    /// # Errors
    /// As [`Self::get_text`], and when a URL on the way is not https.
    fn get_secure_text(&self, url: &str) -> Result<String, HttpError>;

    /// Like [`Self::get_bytes`], but `url` and every redirect it leads to must
    /// be https: a package that could be swapped on the way is not a package.
    ///
    /// # Errors
    /// As [`Self::get_bytes`], and when a URL on the way is not https.
    fn get_secure_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>;
}

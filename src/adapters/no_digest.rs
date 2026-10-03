use std::io;
use std::path::Path;

use crate::ports::Digest;

/// The `Digest` of a `Context` that was not given one: it hashes nothing.
pub struct NoDigest;

impl Digest for NoDigest {
    fn sha256_file(&self, _path: &Path) -> io::Result<String> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_nothing() {
        let error = NoDigest.sha256_file(Path::new("/x")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::Digest;

/// Files by path: each has a fixed digest; any other file is not found.
#[derive(Default)]
pub struct FakeDigest {
    digests: BTreeMap<PathBuf, String>,
}

impl FakeDigest {
    #[must_use]
    pub fn with_digest(mut self, path: &str, digest: &str) -> Self {
        self.digests.insert(PathBuf::from(path), digest.to_owned());
        self
    }
}

impl Digest for FakeDigest {
    fn sha256_file(&self, path: &Path) -> io::Result<String> {
        self.digests
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_by_path() {
        let digest = FakeDigest::default().with_digest("/a", "abc");
        assert_eq!(digest.sha256_file(Path::new("/a")).unwrap(), "abc");
        assert!(digest.sha256_file(Path::new("/b")).is_err());
    }
}

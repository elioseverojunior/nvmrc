use std::io;
use std::path::Path;

use crate::ports::Archive;

/// The `Archive` of a `Context` that was not given one: it unpacks nothing.
pub struct NoArchive;

impl Archive for NoArchive {
    fn extract(&self, _archive: &Path, _destination: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_nothing() {
        let error = NoArchive
            .extract(Path::new("/a"), Path::new("/b"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}

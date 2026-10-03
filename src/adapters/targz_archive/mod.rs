use std::fs::File;
use std::io;
use std::path::Path;

use flate2::read::GzDecoder;

use crate::ports::Archive;

/// The real [`Archive`]: a gzip-compressed tar, unpacked by the `tar` crate.
/// An entry that would land outside `destination` (`..`, an absolute path, or
/// a path that goes through a symlink out of it) fails the whole extraction,
/// and the owner of a file is not restored.
pub struct TarGzArchive;

impl Archive for TarGzArchive {
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()> {
        let mut reader = tar::Archive::new(GzDecoder::new(File::open(archive)?));
        reader.set_preserve_ownerships(false);
        for entry in reader.entries()? {
            let mut entry = entry?;
            if !entry.unpack_in(destination)? {
                let message = format!("unsafe path in archive: {}", entry.path()?.display());
                return Err(io::Error::new(io::ErrorKind::InvalidData, message));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

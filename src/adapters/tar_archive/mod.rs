use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use lzma_rust2::XzReader;

use crate::ports::Archive;

const GZIP_MAGIC: &[u8] = &[0x1f, 0x8b];
const XZ_MAGIC: &[u8] = &[0xfd, b'7', b'z', b'X', b'Z', 0x00];

/// The real [`Archive`]: a gzip- or xz-compressed tar, told apart by its first
/// bytes (as `tar` does) and unpacked by the `tar` crate. An entry that would
/// land outside `destination` (`..`, an absolute path, or a path that goes
/// through a symlink out of it) fails the whole extraction, and the owner of a
/// file is not restored.
pub struct TarArchive;

impl Archive for TarArchive {
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()> {
        let mut magic = [0u8; 6];
        let read = File::open(archive)?.read(&mut magic)?;
        let file = File::open(archive)?;
        if magic[..read].starts_with(GZIP_MAGIC) {
            unpack(GzDecoder::new(file), destination)
        } else if magic[..read].starts_with(XZ_MAGIC) {
            unpack(XzReader::new(file, true), destination)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a gzip or xz archive",
            ))
        }
    }
}

fn unpack(decoder: impl Read, destination: &Path) -> io::Result<()> {
    let mut reader = tar::Archive::new(decoder);
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

#[cfg(test)]
mod tests;

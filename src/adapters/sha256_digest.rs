use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ports::Digest;

/// The real [`Digest`]: SHA-256 of a file read in chunks.
pub struct Sha256Digest;

impl Digest for Sha256Digest {
    fn sha256_file(&self, path: &Path) -> io::Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            hasher.update(&chunk[..read]);
        }
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_a_file_to_lowercase_hex() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("abc");
        std::fs::write(&file, "abc").unwrap();
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(Sha256Digest.sha256_file(&file).unwrap(), expected);
    }

    #[test]
    fn hashes_a_file_longer_than_one_chunk() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("big");
        std::fs::write(&file, vec![b'a'; 200_000]).unwrap();
        let expected = "2287d207f24a941ff3b56c04c8a25ad56b63e3023207b3bb5b4ac0c9869d74be";
        assert_eq!(Sha256Digest.sha256_file(&file).unwrap(), expected);
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let error = Sha256Digest
            .sha256_file(&root.path().join("nope"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}

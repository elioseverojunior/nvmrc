//! Subresource integrity strings (`sha512-<base64>`), as a registry gives them.

use sha2::{Digest as _, Sha512};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IntegrityError {
    #[error("the registry gives no sha512 integrity to check the download against")]
    NoSha512,
    #[error("the download does not match its sha512 integrity")]
    Mismatch,
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// RFC 4648 base64, standard alphabet, `=` padded.
fn base64(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = chunk.iter().enumerate().fold(0_u32, |word, (index, byte)| {
            word | (u32::from(*byte) << (16 - 8 * index))
        });
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (word >> (18 - 6 * index)) & 0x3f;
                encoded.push(char::from(ALPHABET[sextet as usize]));
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}

/// Checks `bytes` against the `sha512-` entries of `integrity` (entries are
/// space separated; other algorithms are ignored): one must match.
///
/// # Errors
/// [`IntegrityError::NoSha512`] without a `sha512-` entry,
/// [`IntegrityError::Mismatch`] when none matches.
pub fn verify(integrity: &str, bytes: &[u8]) -> Result<(), IntegrityError> {
    let mut entries = integrity
        .split_whitespace()
        .filter_map(|entry| entry.strip_prefix("sha512-"))
        .peekable();
    if entries.peek().is_none() {
        return Err(IntegrityError::NoSha512);
    }
    let computed = base64(&Sha512::digest(bytes));
    if entries.any(|entry| entry == computed) {
        Ok(())
    } else {
        Err(IntegrityError::Mismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let vectors = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in vectors {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
        }
    }

    fn integrity_of(bytes: &[u8]) -> String {
        format!("sha512-{}", base64(&Sha512::digest(bytes)))
    }

    #[test]
    fn a_matching_sha512_passes() {
        let empty = "sha512-z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==";
        let integrity = format!("sha1-abc {empty}");
        assert_eq!(verify(&integrity, b""), Ok(()));
    }

    #[test]
    fn a_sha1_only_integrity_is_refused() {
        let refused = Err(IntegrityError::NoSha512);
        assert_eq!(verify("sha1-abc sha256-def", b"x"), refused);
        assert_eq!(verify("", b"x"), refused);
    }

    #[test]
    fn a_mismatch_is_refused() {
        let integrity = integrity_of(b"tarball");
        assert_eq!(verify(&integrity, b"other"), Err(IntegrityError::Mismatch));
    }
}

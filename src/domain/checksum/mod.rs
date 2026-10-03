//! `SHASUMS256.txt` and the comparison of a download with it, as
//! `nvm_get_checksum` and `nvm_compare_checksum` do.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChecksumError {
    #[error("Provided checksum to compare to is empty.")]
    MissingExpected,
    #[error("Checksums do not match: '{computed}' found, '{expected}' expected.")]
    Mismatch { computed: String, expected: String },
}

/// The checksum listed for `file_name` in a `SHASUMS256.txt` (`<hash>  <name>`
/// per line), or `None` when it is not listed.
#[must_use]
pub fn expected_digest(shasums: &str, file_name: &str) -> Option<String> {
    shasums.lines().find_map(|line| {
        let mut columns = line.split_whitespace();
        let digest = columns.next()?;
        (columns.next()? == file_name).then(|| digest.to_owned())
    })
}

/// # Errors
/// - [`ChecksumError::MissingExpected`] when the mirror listed no checksum.
/// - [`ChecksumError::Mismatch`] when the download differs; `nvm.sh` also
///   accepts the expected value with a leading backslash (a `sha256sum` marker
///   for a file name with escapes).
pub fn compare(computed: &str, expected: &str) -> Result<(), ChecksumError> {
    if expected.is_empty() {
        return Err(ChecksumError::MissingExpected);
    }
    if computed == expected || computed.strip_prefix('\\') == Some(expected) {
        return Ok(());
    }
    Err(ChecksumError::Mismatch {
        computed: computed.to_owned(),
        expected: expected.to_owned(),
    })
}

#[cfg(test)]
mod tests;

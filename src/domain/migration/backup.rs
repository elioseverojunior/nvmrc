//! The names of the copies `migrate` keeps before it edits a file.

use crate::domain::timestamp::compact_utc;

/// What separates the file name from the timestamp.
const BACKUP_INFIX: &str = ".nvmrc-backup-";

/// The backup of `file_name` taken at `unix_seconds`:
/// `<file_name>.nvmrc-backup-<yyyymmddThhmmssZ>` (UTC).
#[must_use]
pub fn backup_name(file_name: &str, unix_seconds: i64) -> String {
    format!("{file_name}{BACKUP_INFIX}{}", compact_utc(unix_seconds))
}

/// The newest backup of `file_name` among the file names `candidates`: the
/// greatest timestamp among the names of exactly the [`backup_name`]
/// pattern (other files and malformed timestamps are ignored). The names
/// share their prefix, so the greatest name holds the greatest timestamp.
#[must_use]
pub fn latest_backup<'a>(file_name: &str, candidates: &'a [String]) -> Option<&'a str> {
    candidates
        .iter()
        .map(String::as_str)
        .filter(|candidate| {
            candidate
                .strip_prefix(file_name)
                .and_then(|rest| rest.strip_prefix(BACKUP_INFIX))
                .is_some_and(is_compact_utc)
        })
        .max()
}

/// Whether `stamp` has the shape of [`compact_utc`]: `yyyymmddThhmmssZ`.
fn is_compact_utc(stamp: &str) -> bool {
    let bytes = stamp.as_bytes();
    bytes.len() == 16
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'T'
        && bytes[9..15].iter().all(u8::is_ascii_digit)
        && bytes[15] == b'Z'
}

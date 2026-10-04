//! The names of the copies `migrate` keeps before it edits a file.

use crate::domain::timestamp::compact_utc;

/// What separates the file name from the timestamp.
const BACKUP_INFIX: &str = ".nvmrc-backup-";

/// The backup of `file_name` taken at `unix_seconds`:
/// `<file_name>.nvmrc-backup-<yyyymmddThhmmssZ>` (UTC), followed by
/// `-<sequence>` unless `sequence` is 0 (the name of that second is taken).
#[must_use]
pub fn backup_name(file_name: &str, unix_seconds: i64, sequence: u32) -> String {
    let stamp = compact_utc(unix_seconds);
    match sequence {
        0 => format!("{file_name}{BACKUP_INFIX}{stamp}"),
        _ => format!("{file_name}{BACKUP_INFIX}{stamp}-{sequence}"),
    }
}

/// The newest backup of `file_name` among the file names `candidates`: the
/// greatest timestamp, then the greatest sequence number, among the names of
/// exactly the [`backup_name`] pattern (other files, malformed timestamps or
/// sequence numbers are ignored). A backup dated after `now` (clock skew)
/// loses to every other one.
#[must_use]
pub fn latest_backup<'a>(file_name: &str, candidates: &'a [String], now: i64) -> Option<&'a str> {
    let present = compact_utc(now);
    candidates
        .iter()
        .filter_map(|candidate| {
            let rest = candidate.strip_prefix(file_name)?;
            let (stamp, sequence) = order_key(rest.strip_prefix(BACKUP_INFIX)?)?;
            Some(((stamp <= present.as_str(), stamp, sequence), candidate))
        })
        .max()
        .map(|(_, candidate)| candidate.as_str())
}

/// `(timestamp, sequence)` of the part after the infix, when well formed.
fn order_key(suffix: &str) -> Option<(&str, u32)> {
    let (stamp, sequence) = suffix.split_at_checked(16)?;
    if !is_compact_utc(stamp) {
        return None;
    }
    let sequence = match sequence.strip_prefix('-') {
        None if sequence.is_empty() => 0,
        Some(number) if is_sequence(number) => number.parse().ok()?,
        _ => return None,
    };
    Some((stamp, sequence))
}

/// Digits without a leading zero.
fn is_sequence(number: &str) -> bool {
    !number.starts_with('0')
        && !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
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

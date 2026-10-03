//! `nvm_is_valid_version`: whether a word looks like a version, which is how
//! `nvm exec` and `nvm run` tell a version they cannot resolve from the
//! command (or script) that follows.

/// `stable`, `unstable`, `node`, `iojs`, or a version of one to three
/// numeric parts (a `v` and an `iojs-` prefix allowed, one trailing dot
/// dropped), or a full `x.y.z` with a prerelease suffix (`-rc.1`).
#[must_use]
pub fn is_valid_version(word: &str) -> bool {
    if matches!(word, "stable" | "unstable" | "node" | "iojs") {
        return true;
    }
    let version = word.strip_prefix("iojs-").unwrap_or(word);
    let core = version.strip_prefix('v').unwrap_or(version);
    match core.split_once('-') {
        Some((base, prerelease)) => {
            is_clean_prerelease(prerelease) && base.matches('.').count() == 2 && is_numeric(base)
        }
        None => is_numeric(core.strip_suffix('.').unwrap_or(core)),
    }
}

/// One to three dot-separated runs of digits.
fn is_numeric(core: &str) -> bool {
    !core.is_empty()
        && core.split('.').count() <= 3
        && core.split('.').all(|part| !part.is_empty())
        && core
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
}

/// What follows the first `-`: letters, digits, dots and dashes, no empty
/// dot-separated part, and nothing but digits and dots after its first dot
/// (so `rc.1`, never a file extension like `rc.js`).
fn is_clean_prerelease(suffix: &str) -> bool {
    let after_first_dot = suffix.split_once('.').map_or("", |(_, after)| after);
    !suffix.is_empty()
        && suffix.split('.').all(|part| !part.is_empty())
        && after_first_dot
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
        && suffix
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
}

#[cfg(test)]
mod tests;

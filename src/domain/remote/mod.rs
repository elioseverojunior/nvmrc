//! Which remote releases `nvm ls-remote` lists, as `nvm_remote_versions` picks
//! them, before they are formatted into rows.

use thiserror::Error;

use crate::domain::index::Release;
use crate::domain::version::{Flavor, Version};

/// A release to list. [`Self::line`] is the text `nvm.sh` passes on to its
/// formatter: the version, then the LTS codename, then `*` on the newest
/// release of that codename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRow {
    pub version: Version,
    pub lts: Option<String>,
    pub latest_lts: bool,
}

impl RemoteRow {
    #[must_use]
    pub fn line(&self) -> String {
        match (&self.lts, self.latest_lts) {
            (None, _) => self.version.to_string(),
            (Some(name), false) => format!("{} {name}", self.version),
            (Some(name), true) => format!("{} {name} *", self.version),
        }
    }
}

/// What the user asked for. `lts` is the normalized codename filter: `*` for
/// any LTS release, or a (lowercase) name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub pattern: Option<String>,
    pub lts: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RemoteError {
    #[error("Implicit aliases are not supported in nvm_remote_versions.")]
    ImplicitAlias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub rows: Vec<RemoteRow>,
    /// Some part that ran found nothing (or could not be fetched): `nvm.sh`
    /// then exits 3, even when other rows were found.
    pub missing: bool,
}

/// # Errors
/// Returns [`RemoteError::ImplicitAlias`] for `stable` and `unstable`.
pub fn list(
    node: Option<&[Release]>,
    iojs: Option<&[Release]>,
    query: &Query,
) -> Result<Listing, RemoteError> {
    let Scope {
        pattern,
        node_runs,
        iojs_runs,
    } = scope(query)?;

    let mut missing = false;
    let mut node_rows = Vec::new();
    let mut iojs_rows = Vec::new();
    if node_runs {
        node_rows = node
            .map(|releases| select(releases, pattern, query.lts.as_deref(), false))
            .unwrap_or_default();
        missing |= node_rows.is_empty();
    }
    if iojs_runs {
        iojs_rows = iojs
            .map(|releases| select(releases, pattern, None, true))
            .unwrap_or_default();
        missing |= iojs_rows.is_empty();
    }
    let rows = merge(node_rows, iojs_rows);
    Ok(Listing {
        missing: missing || rows.is_empty(),
        rows,
    })
}

/// Which indexes a query reads, and the pattern left once a flavor word
/// (`node`, `iojs`) has been taken out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope<'a> {
    pub pattern: Option<&'a str>,
    pub node_runs: bool,
    pub iojs_runs: bool,
}

/// # Errors
/// Returns [`RemoteError::ImplicitAlias`] for `stable` and `unstable`.
pub fn scope(query: &Query) -> Result<Scope<'_>, RemoteError> {
    let mut flavor = query.lts.as_ref().map(|_| Flavor::Node);
    let mut pattern = query.pattern.as_deref().filter(|text| !text.is_empty());
    match pattern {
        Some("iojs" | "io.js") => (flavor, pattern) = (Some(Flavor::IoJs), None),
        Some("node") => (flavor, pattern) = (Some(Flavor::Node), None),
        Some("stable" | "unstable") => return Err(RemoteError::ImplicitAlias),
        _ => {}
    }
    Ok(Scope {
        pattern,
        node_runs: flavor != Some(Flavor::IoJs),
        iojs_runs: query.lts.is_none() && flavor != Some(Flavor::Node),
    })
}

/// Node rows come first up to `v4.0.0`, then io.js, then the rest of Node.
/// Without a `v4.0.0` row all Node rows come before io.js.
fn merge(node_rows: Vec<RemoteRow>, iojs_rows: Vec<RemoteRow>) -> Vec<RemoteRow> {
    let split = node_rows
        .iter()
        .position(|row| row.version.to_string() == "v4.0.0")
        .unwrap_or(node_rows.len());
    let mut merged = node_rows;
    let after = merged.split_off(split);
    merged.extend(iojs_rows);
    merged.extend(after);
    merged
}

/// The part of `nvm_ls_remote_index_tab` for one flavor: keep the releases
/// that fit the LTS filter and mark the newest of each codename, then keep
/// those whose version matches the pattern, ascending.
fn select(
    releases: &[Release],
    pattern: Option<&str>,
    lts: Option<&str>,
    iojs: bool,
) -> Vec<RemoteRow> {
    let pattern = pattern
        .map(|text| normalize_pattern(text, iojs))
        .filter(|text| !text.is_empty());
    let mut previous: Option<&str> = None;
    let mut rows = Vec::new();
    for release in releases {
        if !fits_lts(release, lts) {
            continue;
        }
        let name = release.lts.as_deref();
        rows.push(RemoteRow {
            version: release.version,
            lts: release.lts.clone(),
            latest_lts: name.is_some() && name != previous,
        });
        previous = name;
    }
    rows.retain(|row| {
        pattern
            .as_deref()
            .is_none_or(|text| matches_word(row, text))
    });
    rows.sort_by_key(|row| row.version);
    rows
}

fn fits_lts(release: &Release, lts: Option<&str>) -> bool {
    match (lts, release.lts.as_deref()) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some("*"), Some(_)) => true,
        (Some(wanted), Some(name)) => name.to_lowercase().contains(&wanted.to_lowercase()),
    }
}

/// Like `nvm_ensure_version_prefix`: a trailing `.` and `*` are dropped, a
/// leading digit gets the `v`, and for io.js the `iojs-` prefix is dropped.
fn normalize_pattern(pattern: &str, iojs: bool) -> String {
    let trimmed = pattern.strip_suffix('.').unwrap_or(pattern);
    if trimmed == "*" {
        return String::new();
    }
    let bare = if iojs {
        trimmed.strip_prefix("iojs-").unwrap_or(trimmed)
    } else {
        trimmed
    };
    if bare.starts_with(|first: char| first.is_ascii_digit()) {
        format!("v{bare}")
    } else {
        bare.to_owned()
    }
}

/// `grep -w`: the pattern starts the version and is not followed by a word
/// character, so `v20` and `v20.10` match `v20.10.0` but `v2` does not.
fn matches_word(row: &RemoteRow, pattern: &str) -> bool {
    let text = row.version.directory_name();
    text.strip_prefix(pattern).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|next| !(next.is_ascii_alphanumeric() || next == '_'))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LtsError {
    #[error("LTS names must be lowercase")]
    NotLowercase,
    #[error("That many LTS releases do not exist yet.")]
    TooFarBack,
}

impl LtsError {
    /// The exit status `nvm.sh` gives it.
    #[must_use]
    pub fn status(&self) -> u8 {
        match self {
            Self::NotLowercase => 3,
            Self::TooFarBack => 2,
        }
    }
}

/// Like `nvm_normalize_lts`: `-N` is the codename `N` places before the last in
/// `names` (the entries of `alias/lts`, sorted), any other name must be
/// lowercase. Returns the codename, or `*`.
///
/// # Errors
/// [`LtsError::TooFarBack`] when there are not that many codenames, and
/// [`LtsError::NotLowercase`] for a name with uppercase letters.
pub fn normalize_lts(wanted: &str, names: &[String]) -> Result<String, LtsError> {
    if let Some(back) = wanted.strip_prefix('-').and_then(back_count) {
        let index = names.len().saturating_sub(back + 1);
        return match names.get(index) {
            Some(name) if name != "*" => Ok(name.clone()),
            _ => Err(LtsError::TooFarBack),
        };
    }
    if wanted != wanted.to_lowercase() {
        return Err(LtsError::NotLowercase);
    }
    Ok(wanted.to_owned())
}

/// `1`..`9`, optionally followed by more digits.
fn back_count(digits: &str) -> Option<usize> {
    let valid = digits.chars().all(|digit| digit.is_ascii_digit())
        && !digits.starts_with('0')
        && !digits.is_empty();
    valid.then(|| digits.parse().ok()).flatten()
}

#[cfg(test)]
mod tests;

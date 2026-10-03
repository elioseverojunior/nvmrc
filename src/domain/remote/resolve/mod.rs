//! `nvm version-remote`: the one release a description stands for.

use super::{Query, RemoteRow, list, select};
use crate::domain::implicit::derive;
use crate::domain::index::Release;
use crate::domain::version::Version;

/// The newest release `query` stands for, or `None` for `N/A`.
///
/// `node`, `stable` and `unstable` pick the newest release of the newest
/// stable (or unstable) line of the node index, `iojs` the newest io.js
/// release, and anything else the newest release `ls-remote` would list,
/// provided its version contains the pattern. No pattern means `node`.
#[must_use]
pub fn resolve(
    node: Option<&[Release]>,
    iojs: Option<&[Release]>,
    query: &Query,
) -> Option<Version> {
    let lts = query.lts.as_deref();
    match query.pattern.as_deref().filter(|text| !text.is_empty()) {
        Some("iojs") => newest(select(iojs?, None, lts, true)),
        Some("unstable") => newest_of_line(node?, lts, false),
        None | Some("node" | "stable") => newest_of_line(node?, lts, true),
        Some(pattern) => by_pattern(node, iojs, query, pattern),
    }
}

fn newest(rows: Vec<RemoteRow>) -> Option<Version> {
    rows.last().map(|row| row.version)
}

/// The newest release of the highest stable or unstable release line.
fn newest_of_line(node: &[Release], lts: Option<&str>, stable: bool) -> Option<Version> {
    let versions: Vec<Version> = select(node, None, lts, false)
        .iter()
        .map(|row| row.version)
        .collect();
    let implicit = derive(&versions);
    let line = if stable {
        implicit.stable
    } else {
        implicit.unstable
    }?;
    newest(select(node, Some(&line.to_string()), lts, false))
}

fn by_pattern(
    node: Option<&[Release]>,
    iojs: Option<&[Release]>,
    query: &Query,
    pattern: &str,
) -> Option<Version> {
    let listing = list(node, iojs, query).ok()?;
    let version = newest(listing.rows)?;
    version.to_string().contains(pattern).then_some(version)
}

#[cfg(test)]
mod tests;

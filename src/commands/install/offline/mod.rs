//! `--offline`: what is installed or cached is all there is
//! (`nvm_offline_version` and `nvm_ls_cached`).

use crate::context::Context;
use crate::domain::version::Version;
use crate::domain::version_prefix::with_v_prefix;

/// `node-v20.10.0-linux-x64` and `iojs-v3.3.1-linux-x64` hold archives, and
/// `node-v20.10.0` holds a source tree: all versions of the entries of the
/// cache directories `bin` (for this platform) and `src`.
fn cached_versions(context: &Context<'_>) -> Vec<Version> {
    let Ok(cache) = context.cache_dir() else {
        return Vec::new();
    };
    let suffix = context
        .platform()
        .map(|platform| format!("-{}-{}", platform.os.slug(), platform.arch));
    let names = |directory: &str| -> Vec<String> {
        context
            .fs
            .read_dir(&cache.join(directory))
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.is_dir)
            .map(|entry| entry.name)
            .collect()
    };
    let binaries = names("bin")
        .into_iter()
        .filter_map(|name| name.strip_suffix(suffix.as_deref()?).map(str::to_owned));
    binaries
        .chain(names("src"))
        .filter_map(|name| name.strip_prefix("node-").unwrap_or(&name).parse().ok())
        .collect()
}

/// The newest cached version whose name contains `pattern` (with the `v` that
/// a number needs), whether or not it is installed.
#[must_use]
pub fn cached_version(context: &Context<'_>, pattern: &str) -> Option<Version> {
    let wanted = with_v_prefix(pattern);
    cached_versions(context)
        .into_iter()
        .filter(|version| version.to_string().contains(&wanted))
        .max()
}

#[cfg(test)]
mod tests;

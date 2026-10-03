//! `.nvmrc` files: the upward search and the content rules of nvm.sh
//! (`nvm_find_nvmrc` and `nvm_process_nvmrc_content`).

use std::path::{Path, PathBuf};

use crate::ports::FileSystem;

pub const NVMRC_FILE_NAME: &str = ".nvmrc";

/// The hint nvm.sh prints on stderr after an invalid `.nvmrc` message.
pub const PLEASE_SEE: &str =
    "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.";

const INVALID_HEADER: &str = "invalid .nvmrc!
all non-commented content (anything after # is a comment) must be either:
  - a single bare nvm-recognized version-ish
  - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)

additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).

non-commented content parsed:
";

/// The outcome of applying the `.nvmrc` content rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NvmrcContent {
    /// The single bare version-ish line.
    Version(String),
    /// The content breaks the rules; `parsed` is every non-empty line left
    /// after comments and surrounding space were removed.
    Invalid { parsed: Vec<String> },
}

/// `nvm_find_nvmrc`: the nearest `.nvmrc` regular file, looking in `start`
/// (the logical `PWD` string, never canonicalised) and then each parent.
///
/// Like `nvm_find_up`, the walk stops when the path becomes empty, so the
/// filesystem root is not part of it. Nothing found leaves the directory
/// empty, and nvm.sh then retries `/.nvmrc` with `[ -e ]`; here that is
/// `file_info(..).is_ok()`, so a directory named `/.nvmrc` also counts.
#[must_use]
pub fn find_nvmrc(fs: &dyn FileSystem, start: &Path) -> Option<PathBuf> {
    let mut directory = start.to_string_lossy().into_owned();
    while !directory.is_empty() && directory != "." {
        if fs.is_file(&nvmrc_in(&directory)) {
            break;
        }
        directory.truncate(directory.rfind('/').unwrap_or(0));
    }
    let candidate = nvmrc_in(&directory);
    fs.file_info(&candidate).is_ok().then_some(candidate)
}

fn nvmrc_in(directory: &str) -> PathBuf {
    PathBuf::from(format!("{directory}/{NVMRC_FILE_NAME}"))
}

/// `nvm_process_nvmrc_content`: strips `#` comments, trims `[[:space:]]`
/// (CR included), drops empty lines and picks the one bare version.
///
/// A line starting with `=` is bare; any other line with `=` is a `key=value`
/// pair, ignored unless its key is `node` or repeated (both invalid).
#[must_use]
pub fn process_content(contents: &str) -> NvmrcContent {
    let parsed: Vec<String> = contents
        .split('\n')
        .map(|line| trim_space(line.split('#').next().unwrap_or("")))
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    match pick_bare_line(&parsed) {
        Some(version) => NvmrcContent::Version(version),
        None => NvmrcContent::Invalid { parsed },
    }
}

fn pick_bare_line(lines: &[String]) -> Option<String> {
    let mut keys: Vec<&str> = Vec::new();
    let mut bare: Option<&String> = None;
    for line in lines {
        match line.split_once('=').filter(|(key, _)| !key.is_empty()) {
            Some((key, _)) => {
                let key = trim_space(key);
                if key == "node" || keys.contains(&key) {
                    return None;
                }
                keys.push(key);
            }
            None if bare.is_some() => return None,
            None => bare = Some(line),
        }
    }
    bare.cloned()
}

fn trim_space(text: &str) -> &str {
    text.trim_matches(|character| matches!(character, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'))
}

/// The stderr text nvm.sh prints for an invalid `.nvmrc`, ending in a
/// newline, without the closing [`PLEASE_SEE`] line (the caller adds it).
#[must_use]
pub fn invalid_message(parsed: &[String]) -> String {
    let mut message = String::from(INVALID_HEADER);
    for line in parsed {
        message.push_str(line);
        message.push('\n');
    }
    message
}

#[cfg(test)]
mod tests;

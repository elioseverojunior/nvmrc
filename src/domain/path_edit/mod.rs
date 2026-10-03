//! PATH and MANPATH rewriting, as `nvm_change_path` and `nvm_strip_path` do.
//!
//! nvm.sh matches nvm entries with `grep`, `sed` and `awk` regexes into which
//! `NVM_DIR` and the suffix are spliced raw. This module reproduces those
//! patterns by hand (the crate has no regex engine):
//!
//! - flat: `${NVM_DIR}/[^/]*${SUFFIX}`, the legacy `$NVM_DIR/v0.10.48/bin`;
//! - versioned: `${NVM_DIR}/versions/[^/]*/[^/]*${SUFFIX}`.
//!
//! `[^/]*` may cross a `:`, so a match can span two PATH entries; that quirk is
//! kept. In `change_path` a `.` in `NVM_DIR` matches any one character, as the
//! regex does; every other regex metacharacter in `NVM_DIR` is taken literally
//! (nvm.sh would interpret it, or break `sed` on a `#`). Values are assumed to
//! hold no newline (the tools work line by line).

use std::iter;

/// Which of the two nvm entry shapes a pattern recognises.
#[derive(Debug, Clone, Copy)]
enum Layout {
    /// `${NVM_DIR}/[^/]*${SUFFIX}`
    Flat,
    /// `${NVM_DIR}/versions/[^/]*/[^/]*${SUFFIX}`
    Versioned,
}

/// One nvm entry pattern, followed by sed's `[^:]*` so a match is the longest.
#[derive(Debug, Clone, Copy)]
struct EntryPattern<'a> {
    nvm_dir: &'a str,
    suffix: &'a str,
    layout: Layout,
}

impl<'a> EntryPattern<'a> {
    fn both(nvm_dir: &'a str, suffix: &'a str) -> [Self; 2] {
        [Layout::Flat, Layout::Versioned].map(|layout| Self {
            nvm_dir,
            suffix,
            layout,
        })
    }

    /// End of the longest match of the part after `NVM_DIR`, which starts at
    /// `after_dir` in `text`.
    fn tail_end(&self, text: &str, after_dir: usize) -> Option<usize> {
        let rest = &text[after_dir..];
        let segment_start = match self.layout {
            Layout::Flat => rest.starts_with('/').then_some(after_dir + 1)?,
            Layout::Versioned => {
                let version = rest.strip_prefix("/versions/")?;
                let slash = version.find('/')?;
                after_dir + "/versions/".len() + slash + 1
            }
        };
        segment_then_suffix_end(text, segment_start, self.suffix)
    }

    /// End of the longest match starting exactly at `start`.
    fn match_at(&self, text: &str, start: usize) -> Option<usize> {
        let dir_length = regex_dir_length(&text[start..], self.nvm_dir)?;
        self.tail_end(text, start + dir_length)
    }

    /// The leftmost-longest match at or after `from`, as `(start, end)`.
    fn find_from(&self, text: &str, from: usize) -> Option<(usize, usize)> {
        text.char_indices()
            .map(|(index, _)| index)
            .skip_while(|&index| index < from)
            .find_map(|start| self.match_at(text, start).map(|end| (start, end)))
    }

    /// `sed s#PATTERN[^:]*#replacement#`: the first match only.
    fn replace_first(&self, text: &str, replacement: &str) -> String {
        match self.find_from(text, 0) {
            Some((start, end)) => format!("{}{replacement}{}", &text[..start], &text[end..]),
            None => text.to_owned(),
        }
    }
}

/// Bytes of `haystack` matched by `nvm_dir` read as a regex where `.` is any
/// character and everything else is literal.
fn regex_dir_length(haystack: &str, nvm_dir: &str) -> Option<usize> {
    let mut characters = haystack.char_indices();
    for wanted in nvm_dir.chars() {
        let (_, found) = characters.next()?;
        if wanted != '.' && wanted != found {
            return None;
        }
    }
    Some(characters.next().map_or(haystack.len(), |(index, _)| index))
}

/// End of the longest `[^/]*${SUFFIX}[^:]*` starting at `start`.
fn segment_then_suffix_end(text: &str, start: usize, suffix: &str) -> Option<usize> {
    let limit = text[start..]
        .find('/')
        .map_or(text.len(), |offset| start + offset);
    (start..=limit)
        .filter(|&at| text.is_char_boundary(at) && text[at..].starts_with(suffix))
        .map(|at| entry_end(text, at + suffix.len()))
        .max()
}

/// Index of the next `:` at or after `from`, or the end of `text`.
fn entry_end(text: &str, from: usize) -> usize {
    text[from..]
        .find(':')
        .map_or(text.len(), |offset| from + offset)
}

/// Whether `(^|:)(/usr(/local)?)?${SUFFIX}:.*` precedes an nvm entry (rule 3).
fn system_entry_precedes(text: &str, suffix: &str, patterns: &[EntryPattern<'_>]) -> bool {
    let entry_starts = iter::once(0).chain(text.match_indices(':').map(|(index, _)| index + 1));
    let after_system_entry = entry_starts.into_iter().find_map(|start| {
        ["", "/usr", "/usr/local"].into_iter().find_map(|prefix| {
            let lead = format!("{prefix}{suffix}:");
            text[start..].starts_with(&lead).then(|| start + lead.len())
        })
    });
    after_system_entry.is_some_and(|from| {
        patterns
            .iter()
            .any(|pattern| pattern.find_from(text, from).is_some())
    })
}

/// `nvm_change_path PATHVAL SUFFIX VERSION_DIR`: puts `version_dir + suffix`
/// into `path_value` (`suffix` is `/bin` for PATH, `/share/man` for MANPATH).
///
/// The branches, in nvm.sh's order:
/// 1. an empty value becomes the new entry alone, with no colon;
/// 2. a value with no nvm entry gets the new entry prepended;
/// 3. a value where `/bin`, `/usr/bin` or `/usr/local/bin` (with the suffix
///    in place of `/bin`) comes before an nvm entry also gets it prepended,
///    and the old nvm entry is **kept** (nvm issue #1652);
/// 4. otherwise the flat then the versioned pattern each replace their
///    **first** match only, extended by `[^:]*`, so `.../bin/sub` and
///    `.../binx` are replaced whole. The second pass runs on the output of the
///    first, so it can hit the entry the first pass just wrote.
#[must_use]
pub fn change_path(path_value: &str, suffix: &str, version_dir: &str, nvm_dir: &str) -> String {
    let new_entry = format!("{version_dir}{suffix}");
    if path_value.is_empty() {
        return new_entry;
    }
    let patterns = EntryPattern::both(nvm_dir, suffix);
    let has_nvm_entry = patterns
        .iter()
        .any(|pattern| pattern.find_from(path_value, 0).is_some());
    if !has_nvm_entry || system_entry_precedes(path_value, suffix, &patterns) {
        return format!("{new_entry}:{path_value}");
    }
    patterns
        .iter()
        .fold(path_value.to_owned(), |text, pattern| {
            pattern.replace_first(&text, &new_entry)
        })
}

/// `nvm_strip_path VALUE SUFFIX`, used by `nvm deactivate`: drops **every**
/// entry that starts with `nvm_dir` (a literal prefix, as awk's `index` is)
/// and whose remainder matches `^(/versions/[^/]*)?/[^/]*${SUFFIX}.*$`.
/// Other entries, empty ones included, are kept in order, and a trailing `:`
/// survives.
///
/// An empty `nvm_dir` is the caller's problem: nvm.sh refuses it with
/// `${NVM_DIR} not set!` and status 1, while this function would then strip
/// entries such as `/v1/bin`.
#[must_use]
pub fn strip_path(value: &str, suffix: &str, nvm_dir: &str) -> String {
    let patterns = EntryPattern::both(nvm_dir, suffix);
    let is_nvm_entry = |entry: &str| {
        entry.starts_with(nvm_dir)
            && patterns
                .iter()
                .any(|pattern| pattern.tail_end(entry, nvm_dir.len()).is_some())
    };
    let records = value.strip_suffix(':').unwrap_or(value);
    let mut kept = records
        .split(':')
        .filter(|entry| !is_nvm_entry(entry))
        .collect::<Vec<_>>()
        .join(":");
    if value.ends_with(':') {
        kept.push(':');
    }
    kept
}

/// The MANPATH post-step of `nvm use`: appends `:` unless the value matches
/// the sh `case` patterns `:* | *::* | *:`, so `man` still consults its
/// default path through an empty entry. An empty value becomes `:`.
#[must_use]
pub fn manpath_with_trailing_colon(value: &str) -> String {
    if value.starts_with(':') || value.contains("::") || value.ends_with(':') {
        value.to_owned()
    } else {
        format!("{value}:")
    }
}

#[cfg(test)]
mod tests;

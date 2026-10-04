//! Which release each `(Latest: …)` and `(Aliases: …)` annotation lands on,
//! as the `BEGIN` block of the `awk` in `nvm_print_versions` works it out.

use std::collections::HashMap;

use super::FormatInput;
use crate::domain::remote::RemoteRow;

/// The newest release of each kind in the listing.
#[derive(Default)]
pub(super) struct Latest {
    pub(super) stable: Option<String>,
    unstable: Option<String>,
    iojs: Option<String>,
}

/// `^v0\.[0-9]*[13579]\.`: an odd minor of the old 0.x scheme.
pub(super) fn is_old_unstable(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("v0.") else {
        return false;
    };
    let minor: String = rest.chars().take_while(char::is_ascii_digit).collect();
    rest[minor.len()..].starts_with('.')
        && minor
            .chars()
            .last()
            .is_some_and(|digit| "13579".contains(digit))
}

pub(super) fn latest_of_each_kind(rows: &[RemoteRow]) -> Latest {
    let mut latest = Latest::default();
    for row in rows {
        let text = row.version.to_string();
        if text.starts_with("iojs-") {
            latest.iojs = Some(text);
        } else if is_old_unstable(&text) {
            latest.unstable = Some(text);
        } else if text.starts_with('v') {
            latest.stable = Some(text);
        }
    }
    latest
}

/// The release each alias target stands for, and the alias names on each
/// release, in the order the aliases were given.
pub(super) fn aliases_by_version(
    input: &FormatInput<'_>,
    latest: &Latest,
) -> HashMap<String, Vec<String>> {
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    for (target, name) in input.named_aliases {
        let version = match target.as_str() {
            "node" | "stable" => latest.stable.clone(),
            "unstable" => latest.unstable.clone(),
            "iojs" | "iojs-" => latest.iojs.clone(),
            _ => input
                .rows
                .iter()
                .map(|row| row.version.to_string())
                .rfind(|text| text == target || text.starts_with(&format!("{target}."))),
        };
        if let Some(version) = version {
            named.entry(version).or_default().push(name.clone());
        }
    }
    named
}

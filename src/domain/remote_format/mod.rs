//! The rows `nvm ls-remote` prints, as the `awk` of `nvm_print_versions`
//! prints them when stdout is not a terminal (no colors): the version
//! right-aligned in 15 columns, then up to three annotation columns that each
//! start at the same place on every row.

use std::collections::HashMap;

use crate::domain::listing::{RowKind, format_row};
use crate::domain::remote::RemoteRow;
use crate::domain::version::Version;

pub struct FormatInput<'a> {
    pub rows: &'a [RemoteRow],
    /// Everything installed: those rows get a `*`.
    pub installed: &'a [Version],
    /// What `nvm current` prints: that row gets an arrow.
    pub current: &'a str,
    /// `node` for a plain listing: the newest stable release is annotated
    /// `(Latest: node)`.
    pub latest_alias: Option<&'a str>,
    /// `(target, name)` of the aliases to show: `(Aliases: name, ...)` marks
    /// the release each target resolves to.
    pub named_aliases: &'a [(String, String)],
}

#[derive(Default)]
struct Latest {
    stable: Option<String>,
    unstable: Option<String>,
    iojs: Option<String>,
}

/// `^v0\.[0-9]*[13579]\.`: an odd minor of the old 0.x scheme.
fn is_old_unstable(text: &str) -> bool {
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

fn latest_of_each_kind(rows: &[RemoteRow]) -> Latest {
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

/// The release each alias target stands for, and the aliases on each release.
fn aliases_by_version(input: &FormatInput<'_>, latest: &Latest) -> HashMap<String, Vec<String>> {
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

struct Cells {
    version: String,
    padding: &'static str,
    width: usize,
    text: [String; 3],
}

fn cells_for(
    row: &RemoteRow,
    input: &FormatInput<'_>,
    latest: &Latest,
    named: &HashMap<String, Vec<String>>,
) -> Cells {
    let text = row.version.to_string();
    let installed = input.installed.contains(&row.version);
    let kind = if text == input.current {
        RowKind::Current
    } else if installed {
        RowKind::Installed
    } else {
        RowKind::Plain
    };
    let version = format_row(&text, kind);
    let padding = if installed { "" } else { "  " };
    let lts = row.lts.as_deref().map(|name| {
        if row.latest_lts {
            format!(" (Latest LTS: {name})")
        } else {
            format!(" (LTS: {name})")
        }
    });
    let newest = input
        .latest_alias
        .filter(|_| latest.stable.as_deref() == Some(text.as_str()))
        .map(|alias| format!(" (Latest: {alias})"));
    let aliases = named
        .get(&text)
        .map(|names| format!(" (Aliases: {})", names.join(", ")));
    Cells {
        width: version.len() + padding.len(),
        version,
        padding,
        text: [
            lts.unwrap_or_default(),
            newest.unwrap_or_default(),
            aliases.unwrap_or_default(),
        ],
    }
}

fn spaces(count: usize) -> String {
    " ".repeat(count)
}

#[must_use]
pub fn format_remote_rows(input: &FormatInput<'_>) -> Vec<String> {
    let latest = latest_of_each_kind(input.rows);
    let named = aliases_by_version(input, &latest);
    let cells: Vec<Cells> = input
        .rows
        .iter()
        .map(|row| cells_for(row, input, &latest, &named))
        .collect();
    let widest_version = cells.iter().map(|cell| cell.width).max().unwrap_or(0);
    let widths: [usize; 3] = std::array::from_fn(|column| {
        cells
            .iter()
            .map(|cell| cell.text[column].len())
            .max()
            .unwrap_or(0)
    });
    cells
        .iter()
        .map(|cell| assemble(cell, widest_version, &widths))
        .collect()
}

/// One row: the version, then each annotation column that any row has, padded
/// so that the columns line up; nothing is added after the last annotation.
fn assemble(cell: &Cells, widest_version: usize, widths: &[usize; 3]) -> String {
    let Some(last) = cell.text.iter().rposition(|text| !text.is_empty()) else {
        return cell.version.clone();
    };
    let mut row = format!("{}{}", cell.version, cell.padding);
    if widths[1] > 0 || widths[2] > 0 {
        row.push_str(&spaces(widest_version - cell.width));
    }
    let mut gap = String::new();
    for (text, &width) in cell.text.iter().zip(widths).take(last + 1) {
        if width > 0 {
            row.push_str(&gap);
            row.push_str(text);
            gap = format!("{}  ", spaces(width - text.len()));
        }
    }
    row
}

#[cfg(test)]
mod tests;

//! Editing a `.nvmrc` in place: the bare version and the `<tool>=` pins.

use super::pins::key_value;
use super::{content_of, is_bare, trim_space};
use crate::domain::tool::Tool;
use crate::domain::version::Version;

/// What `--save` writes over `contents`: its bare version line replaced by
/// `version` (its trailing comment kept), every other line (pins, other keys,
/// comments) kept in place. Without a bare line the version goes first. With
/// no other line the result is that one line ending in `\n`: exactly
/// `<version>\n`, as nvm.sh writes it, when it has no comment.
#[must_use]
pub fn with_version(contents: &str, version: &str) -> String {
    let (mut lines, default_ending) = split_lines(contents);
    let bare = lines.iter().position(|line| is_bare(content_of(line)));
    let index = bare.unwrap_or(0);
    let line = bare.map_or_else(
        || version.to_owned(),
        |index| replaced(&lines[index], version),
    );
    let alone = (lines.iter().enumerate())
        .all(|(other, text)| bare == Some(other) || trim_space(text).is_empty());
    if alone {
        return format!("{line}\n");
    }
    match bare {
        Some(index) => lines[index] = format!("{line}{}", ending_of(&lines[index])),
        None => lines.insert(index, format!("{line}{default_ending}")),
    }
    lines.concat()
}

/// `contents` with the `<tool>=` line set to `version`: the line replaced
/// (its trailing comment kept) or `<tool>=<version>` added after the last
/// non-blank line. Each line keeps its own ending.
#[must_use]
pub fn with_pin(contents: &str, tool: Tool, version: &Version) -> String {
    let (mut lines, default_ending) = split_lines(contents);
    let pin = format!("{}={}", tool.name(), tool.show(version));
    let existing = lines
        .iter()
        .position(|line| key_value(content_of(line)).is_some_and(|(key, _)| key == tool.name()));
    if let Some(index) = existing {
        let ending = ending_of(&lines[index]);
        lines[index] = format!("{}{ending}", replaced(&lines[index], &pin));
        return lines.concat();
    }
    let anchor = lines.iter().rposition(|line| !trim_space(line).is_empty());
    let ending = anchor.map_or(default_ending, |index| ending_of(&lines[index]));
    lines.insert(
        anchor.map_or(0, |index| index + 1),
        format!("{pin}{ending}"),
    );
    lines.concat()
}

/// The lines of `contents`, each with its ending (`\n` or `\r\n`); a last
/// line without one gets the file's first ending. The second value is that
/// ending, `\n` for a file without any.
fn split_lines(contents: &str) -> (Vec<String>, &'static str) {
    let default_ending = contents
        .split_inclusive('\n')
        .map(ending_of)
        .find(|ending| !ending.is_empty())
        .unwrap_or("\n");
    let lines = contents
        .split_inclusive('\n')
        .map(|line| match ending_of(line) {
            "" => format!("{line}{default_ending}"),
            _ => line.to_owned(),
        })
        .collect();
    (lines, default_ending)
}

fn ending_of(line: &str) -> &'static str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

/// `pin` (a version or `<tool>=<version>`) followed by the comment of
/// `line`, if it has one, and the space that stood before the comment.
fn replaced(line: &str, pin: &str) -> String {
    let body = line.trim_end_matches(['\r', '\n']);
    match body.find('#') {
        Some(hash) => {
            let before = &body[..hash];
            let gap = &before[before.trim_end().len()..];
            format!("{pin}{gap}{}", &body[hash..])
        }
        None => pin.to_owned(),
    }
}

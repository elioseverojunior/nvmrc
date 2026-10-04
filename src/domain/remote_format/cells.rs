//! One row of `nvm ls-remote` before alignment: the version cell and the
//! three annotation columns, each as printed (maybe colored) and as seen.

use std::collections::HashMap;

use super::latest::Latest;
use super::{FormatInput, RemoteColors};
use crate::domain::colors::{ITALIC_OFF, ITALIC_ON, Palette, wrap};
use crate::domain::listing::colored::paint_row;
use crate::domain::listing::{RowKind, format_row};
use crate::domain::remote::RemoteRow;

/// How the cells are painted: the colors (`None` when off) and whether the
/// alias names are italic (`has_italics` of the awk).
pub(super) struct Style<'a> {
    pub(super) colors: Option<RemoteColors<'a>>,
    pub(super) italics: bool,
}

impl Style<'_> {
    /// The `paint` of the awk: the colored span when colors are on and the
    /// role has a color, otherwise the text.
    fn paint(&self, code: impl FnOnce(&Palette) -> Option<String>, text: &str) -> String {
        match self.colors {
            Some(colors) => wrap(code(colors.palette).as_deref(), text),
            None => text.to_owned(),
        }
    }

    fn italicize(&self, name: &str) -> String {
        if self.italics {
            format!("{ITALIC_ON}{name}{ITALIC_OFF}")
        } else {
            name.to_owned()
        }
    }

    fn version(&self, text: &str, kind: RowKind) -> String {
        match self.colors {
            Some(colors) => paint_row(text, kind, colors.palette, true),
            None => format_row(text, kind),
        }
    }
}

/// An annotation column: what is printed and what the terminal shows.
#[derive(Default)]
pub(super) struct Annotation {
    pub(super) shown: String,
    pub(super) printed: String,
}

pub(super) struct Cells {
    pub(super) version: String,
    pub(super) padding: &'static str,
    /// The visible width of the version and its padding.
    pub(super) width: usize,
    pub(super) columns: [Annotation; 3],
}

/// The length the terminal shows: `\e[...m` sequences take no room. Bytes
/// are counted, like the `length` of the awk.
fn visible_length(text: &str) -> usize {
    let mut length = 0;
    let mut rest = text;
    while let Some(start) = rest.find('\x1b') {
        length += start;
        let end = rest[start..]
            .find('m')
            .map_or(rest.len(), |at| start + at + 1);
        rest = &rest[end..];
    }
    length + rest.len()
}

fn row_kind(text: &str, current: &str, installed: bool) -> RowKind {
    if text == current {
        RowKind::Current
    } else if installed {
        RowKind::Installed
    } else {
        RowKind::Plain
    }
}

fn lts_column(row: &RemoteRow, style: &Style<'_>) -> Annotation {
    let Some(name) = row.lts.as_deref() else {
        return Annotation::default();
    };
    if row.latest_lts {
        let shown = format!(" (Latest LTS: {name})");
        let printed = style.paint(Palette::latest_lts, &shown);
        Annotation { shown, printed }
    } else {
        let shown = format!(" (LTS: {name})");
        let printed = style.paint(|palette| palette.old_lts().map(str::to_owned), &shown);
        Annotation { shown, printed }
    }
}

fn latest_column(alias: &str, style: &Style<'_>) -> Annotation {
    let printed = format!(" (Latest: {})", style.italicize(alias));
    Annotation {
        shown: format!(" (Latest: {alias})"),
        printed: style.paint(Palette::annotation, &printed),
    }
}

fn aliases_column(names: &[String], style: &Style<'_>) -> Annotation {
    let italic: Vec<String> = names.iter().map(|name| style.italicize(name)).collect();
    let printed = format!(" (Aliases: {})", italic.join(", "));
    Annotation {
        shown: format!(" (Aliases: {})", names.join(", ")),
        printed: style.paint(Palette::annotation, &printed),
    }
}

/// The three annotation columns of a row: LTS, latest, aliases.
fn columns(
    row: &RemoteRow,
    input: &FormatInput<'_>,
    latest: &Latest,
    named: &HashMap<String, Vec<String>>,
    style: &Style<'_>,
) -> [Annotation; 3] {
    let text = row.version.to_string();
    let newest = input
        .latest_alias
        .filter(|_| latest.stable.as_deref() == Some(text.as_str()));
    [
        lts_column(row, style),
        newest.map_or_else(Annotation::default, |alias| latest_column(alias, style)),
        named
            .get(&text)
            .map_or_else(Annotation::default, |names| aliases_column(names, style)),
    ]
}

/// The cells of a row. The padding is two spaces, except for an installed
/// row with colors off, whose ` *` takes those two columns.
pub(super) fn cells_for(
    row: &RemoteRow,
    input: &FormatInput<'_>,
    latest: &Latest,
    named: &HashMap<String, Vec<String>>,
    style: &Style<'_>,
) -> Cells {
    let text = row.version.to_string();
    let installed = input.installed.contains(&row.version);
    let version = style.version(&text, row_kind(&text, input.current, installed));
    let padding = if installed && style.colors.is_none() {
        ""
    } else {
        "  "
    };
    Cells {
        width: visible_length(&version) + padding.len(),
        version,
        padding,
        columns: columns(row, input, latest, named, style),
    }
}

//! The rows `nvm ls-remote` prints, as the `awk` of `nvm_print_versions`
//! prints them: the version right-aligned in 15 columns, then up to three
//! annotation columns that each start at the same place on every row. With
//! colors on, the rows and annotations are colored and the alignment counts
//! only what the terminal shows, so the layout is the same either way.

mod cells;
mod latest;

use cells::{Cells, Style, cells_for};
use latest::{aliases_by_version, latest_of_each_kind};

use crate::domain::colors::Palette;
use crate::domain::remote::RemoteRow;
use crate::domain::version::Version;

pub struct FormatInput<'a> {
    pub rows: &'a [RemoteRow],
    /// Everything installed: those rows get a `*` (or the installed color).
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

/// The colors of a colored listing: the palette, and whether the terminal
/// shows italics (`nvm_has_italics`).
#[derive(Debug, Clone, Copy)]
pub struct RemoteColors<'a> {
    pub palette: &'a Palette,
    pub italics: bool,
}

/// The rows with colors off, as nvm.sh prints them when stdout is not a
/// terminal.
#[must_use]
pub fn format_remote_rows(input: &FormatInput<'_>) -> Vec<String> {
    paint_remote_rows(input, None)
}

/// The rows with the given colors (`None`: colors off). The alias names and
/// `node` are italic only when the terminal shows italics and the listing
/// has the latest or alias annotations.
#[must_use]
pub fn paint_remote_rows(input: &FormatInput<'_>, colors: Option<RemoteColors<'_>>) -> Vec<String> {
    let annotated = input.latest_alias.is_some() || !input.named_aliases.is_empty();
    let style = Style {
        colors,
        italics: annotated && colors.is_some_and(|colors| colors.italics),
    };
    let latest = latest_of_each_kind(input.rows);
    let named = aliases_by_version(input, &latest);
    let cells: Vec<Cells> = input
        .rows
        .iter()
        .map(|row| cells_for(row, input, &latest, &named, &style))
        .collect();
    let widest_version = cells.iter().map(|cell| cell.width).max().unwrap_or(0);
    let widths: [usize; 3] = std::array::from_fn(|column| {
        cells
            .iter()
            .map(|cell| cell.columns[column].shown.len())
            .max()
            .unwrap_or(0)
    });
    cells
        .iter()
        .map(|cell| assemble(cell, widest_version, &widths))
        .collect()
}

fn spaces(count: usize) -> String {
    " ".repeat(count)
}

/// One row: the version, then each annotation column that any row has, padded
/// so that the columns line up; nothing is added after the last annotation.
fn assemble(cell: &Cells, widest_version: usize, widths: &[usize; 3]) -> String {
    let Some(last) = cell
        .columns
        .iter()
        .rposition(|column| !column.shown.is_empty())
    else {
        return cell.version.clone();
    };
    let mut row = format!("{}{}", cell.version, cell.padding);
    if widths[1] > 0 || widths[2] > 0 {
        row.push_str(&spaces(widest_version - cell.width));
    }
    let mut gap = String::new();
    for (column, &width) in cell.columns.iter().zip(widths).take(last + 1) {
        if width > 0 {
            row.push_str(&gap);
            row.push_str(&column.printed);
            gap = format!("{}  ", spaces(width - column.shown.len()));
        }
    }
    row
}

#[cfg(test)]
mod colored_tests;
#[cfg(test)]
mod tests;

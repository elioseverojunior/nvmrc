//! The rows `nvm ls` prints, as `nvm.sh` prints them when stdout is not a
//! terminal (no colors): the version right-aligned in 15 columns.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// An installed version: `        v18.9.0 *`.
    Installed,
    /// The version in use: `->      v20.1.0 *`.
    Current,
    /// Not installed, so no marker: `            N/A`.
    Plain,
}

/// Right-aligns by bytes, like the `awk` of `nvm.sh`: the 3-byte `∞` is
/// padded with 12 spaces, not 14.
fn pad_left(text: &str, width: usize) -> String {
    format!("{}{text}", " ".repeat(width.saturating_sub(text.len())))
}

#[must_use]
pub fn format_row(text: &str, kind: RowKind) -> String {
    match kind {
        RowKind::Installed => format!("{} *", pad_left(text, 15)),
        RowKind::Current => format!("->{} *", pad_left(text, 13)),
        RowKind::Plain => pad_left(text, 15),
    }
}

/// The `system` row, with the version of the system node when it is known.
#[must_use]
pub fn format_system_row(kind: RowKind, version: Option<&str>) -> String {
    let row = format_row("system", kind);
    match version {
        Some(version) => format!("{row} (-> {version})"),
        None => row,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_versions_are_right_aligned_with_a_star() {
        assert_eq!(
            format_row("v18.9.0", RowKind::Installed),
            "        v18.9.0 *"
        );
        assert_eq!(
            format_row("iojs-v2.5.0", RowKind::Installed),
            "    iojs-v2.5.0 *"
        );
    }

    #[test]
    fn the_current_version_has_an_arrow_and_the_same_width() {
        let row = format_row("v20.1.0", RowKind::Current);
        assert_eq!(row, "->      v20.1.0 *");
        assert_eq!(row.len(), format_row("v20.1.0", RowKind::Installed).len());
    }

    #[test]
    fn a_plain_row_has_no_star() {
        assert_eq!(format_row("N/A", RowKind::Plain), "            N/A");
    }

    #[test]
    fn padding_counts_bytes_so_infinity_gets_twelve_spaces() {
        assert_eq!(format_row("∞", RowKind::Plain), "            ∞");
    }

    #[test]
    fn a_name_longer_than_the_column_is_not_cut() {
        let row = format_row("iojs-v100.100.100", RowKind::Installed);
        assert_eq!(row, "iojs-v100.100.100 *");
    }

    #[test]
    fn the_system_row_shows_the_version_of_the_system_node() {
        let installed = format_system_row(RowKind::Installed, Some("v22.1.0"));
        assert_eq!(installed, "         system * (-> v22.1.0)");
        let current = format_system_row(RowKind::Current, Some("v22.1.0"));
        assert_eq!(current, "->       system * (-> v22.1.0)");
        assert_eq!(
            format_system_row(RowKind::Installed, None),
            "         system *"
        );
    }
}

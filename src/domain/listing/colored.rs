//! The rows of `nvm ls` as `nvm_print_versions` paints them: with colors off
//! they are the plain rows of [`format_row`]; with colors on each row is one
//! colored span and loses its ` *` marker.

use super::{RowKind, format_row, format_system_row, pad_left};
use crate::domain::colors::{Palette, Role, wrap};

/// A version row. `Installed` takes the installed color, `Current` the
/// current color with its `->`, `Plain` (`N/A`, `∞`) is never colored.
#[must_use]
pub fn paint_row(text: &str, kind: RowKind, palette: &Palette, colors: bool) -> String {
    if colors {
        paint(text, kind, Role::Installed, palette)
    } else {
        format_row(text, kind)
    }
}

/// The `fmt_installed` / `fmt_system` / `fmt_current` of the awk: a role
/// without a color falls back to `%15s`, so the current row loses its `->`.
fn paint(text: &str, kind: RowKind, role: Role, palette: &Palette) -> String {
    match kind {
        RowKind::Current => match palette.code(Role::Current) {
            Some(code) => wrap(Some(code), &format!("->{}", pad_left(text, 13))),
            None => pad_left(text, 15),
        },
        RowKind::Installed => wrap(palette.code(role), &pad_left(text, 15)),
        RowKind::Plain => pad_left(text, 15),
    }
}

/// The `system` row: the system color (the current color when it is the
/// version in use), then the target ` (-> v16.0.0)` in the system color.
#[must_use]
pub fn paint_system_row(
    kind: RowKind,
    version: Option<&str>,
    palette: &Palette,
    colors: bool,
) -> String {
    if !colors {
        return format_system_row(kind, version);
    }
    let row = paint("system", kind, Role::System, palette);
    match version {
        Some(version) => {
            let target = wrap(palette.code(Role::System), &format!("-> {version}"));
            format!("{row} ({target})")
        }
        None => row,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(setting: &str) -> Palette {
        Palette::from_setting(Some(setting)).0
    }

    #[test]
    fn colors_off_gives_the_plain_rows() {
        let colors = palette("bygre");
        assert_eq!(
            paint_row("v18.20.4", RowKind::Installed, &colors, false),
            "       v18.20.4 *"
        );
        assert_eq!(
            paint_row("v20.11.1", RowKind::Current, &colors, false),
            "->     v20.11.1 *"
        );
        assert_eq!(
            paint_system_row(RowKind::Current, Some("v16.0.0"), &colors, false),
            "->       system * (-> v16.0.0)"
        );
    }

    #[test]
    fn installed_and_current_rows_are_one_span_without_a_star() {
        let colors = palette("bygre");
        assert_eq!(
            paint_row("iojs-v3.3.1", RowKind::Installed, &colors, true),
            "\x1b[0;34m    iojs-v3.3.1\x1b[0m"
        );
        assert_eq!(
            paint_row("v20.11.1", RowKind::Current, &colors, true),
            "\x1b[0;32m->     v20.11.1\x1b[0m"
        );
    }

    #[test]
    fn a_plain_row_is_never_colored() {
        let colors = palette("bygre");
        assert_eq!(
            paint_row("N/A", RowKind::Plain, &colors, true),
            "            N/A"
        );
    }

    #[test]
    fn the_system_row_and_its_target_take_the_system_color() {
        let colors = palette("bygre");
        assert_eq!(
            paint_system_row(RowKind::Installed, Some("v16.0.0"), &colors, true),
            "\x1b[0;33m         system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)"
        );
        assert_eq!(
            paint_system_row(RowKind::Installed, None, &colors, true),
            "\x1b[0;33m         system\x1b[0m"
        );
    }

    #[test]
    fn a_current_system_row_is_current_but_its_target_stays_system() {
        let colors = palette("bygre");
        assert_eq!(
            paint_system_row(RowKind::Current, Some("v16.0.0"), &colors, true),
            "\x1b[0;32m->       system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)"
        );
    }

    #[test]
    fn a_role_without_color_is_plain_without_star_or_arrow() {
        let short = palette("rg");
        assert_eq!(
            paint_row("v20.11.1", RowKind::Current, &short, true),
            "       v20.11.1"
        );
        let invalid = palette("zzzzz");
        assert_eq!(
            paint_row("v18.20.4", RowKind::Installed, &invalid, true),
            "       v18.20.4"
        );
        assert_eq!(
            paint_system_row(RowKind::Installed, Some("v16.0.0"), &invalid, true),
            "         system (-> v16.0.0)"
        );
    }

    #[test]
    fn the_roles_follow_the_setting() {
        let colors = palette("rgbcm");
        assert_eq!(
            paint_row("v22.3.0", RowKind::Installed, &colors, true),
            "\x1b[0;31m        v22.3.0\x1b[0m"
        );
        assert_eq!(
            paint_system_row(RowKind::Installed, Some("v16.0.0"), &colors, true),
            "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)"
        );
    }
}

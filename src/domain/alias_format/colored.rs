//! The alias rows as `nvm_print_formatted_alias` paints them: with colors off
//! they are the plain lines of [`format_line`]; with colors on the arrows are
//! gray and the alias, its target and its version take the color of what the
//! version is.

use super::format_line;
use crate::domain::colors::{ARROW, Palette, RESET, Role, wrap};

/// What the resolved version of an alias is, in the order nvm.sh checks it:
/// the version in use, an installed version, nothing (`N/A`) or a loop (`∞`),
/// or anything else (`system` when it is not in use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionState {
    Current,
    Installed,
    Missing,
    Other,
}

/// Where an alias comes from: a file of the alias directory, one of the
/// implicit aliases without a file (`node`, `stable`, `unstable`, `iojs`,
/// marked `(default)`), or a file of `alias/lts` (its name in the LTS color).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasKind {
    File,
    Implicit,
    Lts,
}

/// One alias row: `alias -> target (-> version)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AliasRow<'a> {
    pub alias: &'a str,
    pub target: &'a str,
    pub version: &'a str,
    pub state: VersionState,
    pub kind: AliasKind,
}

/// The row with colors on or off. A role without a color (an invalid
/// `NVM_COLORS`) leaves its text plain; nvm.sh splices a malformed `\e[`
/// there instead (deviation on purpose).
#[must_use]
pub fn paint_line(row: &AliasRow<'_>, palette: &Palette, colors: bool) -> String {
    if !colors {
        let available = row.state != VersionState::Missing;
        let implicit = row.kind == AliasKind::Implicit;
        return format_line(row.alias, row.target, row.version, available, implicit);
    }
    let line = colored_line(row, palette);
    if row.kind == AliasKind::Implicit {
        format!("{line} {}", wrap(palette.code(Role::Default), "(default)"))
    } else {
        line
    }
}

/// `ALIAS ARROW VERSION` or `ALIAS ARROW DEST (ARROW VERSION)`, colored.
fn colored_line(row: &AliasRow<'_>, palette: &Palette) -> String {
    let role = role_of(row.state).and_then(|role| palette.code(role));
    let lts = palette.lts();
    let lts_or_role = |is_lts: bool| if is_lts { lts.as_deref() } else { role };
    let arrow = format!("{ARROW}->{RESET}");
    let alias = wrap(lts_or_role(row.kind == AliasKind::Lts), row.alias);
    let version = wrap(role, row.version);
    if row.target == row.version {
        return format!("{alias} {arrow} {version}");
    }
    let target = wrap(lts_or_role(is_lts_target(row.target)), row.target);
    format!("{alias} {arrow} {target} ({arrow} {version})")
}

/// The role of the first matching rule; `Other` is plain.
fn role_of(state: VersionState) -> Option<Role> {
    match state {
        VersionState::Current => Some(Role::Current),
        VersionState::Installed => Some(Role::Installed),
        VersionState::Missing => Some(Role::NotInstalled),
        VersionState::Other => None,
    }
}

/// The `[ "_${DEST%/*}" = "_lts" ]` of nvm.sh: what precedes the last `/` is
/// `lts` (a bare `lts` counts too, `%/*` leaves it as is).
fn is_lts_target(target: &str) -> bool {
    let parent = target.rfind('/').map_or(target, |slash| &target[..slash]);
    parent == "lts"
}

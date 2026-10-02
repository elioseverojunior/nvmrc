//! `nvm ls [pattern]`: the installed versions, one row each.
//!
//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
//! Unlike `nvm.sh` it does not print a blank row when only a system node
//! exists, and it keeps both io.js and Node versions that share a number.

use crate::commands::Output;
use crate::commands::current;
use crate::commands::resolve::{Resolved, resolve_installed, system_node, system_version};
use crate::context::Context;
use crate::domain::alias::AliasStore;
use crate::domain::listing::{RowKind, format_row, format_system_row};
use crate::domain::version::{Flavor, Version, VersionPattern};
use crate::error::{CliError, NvmExitCode};

enum Entry {
    Version(Version),
    /// The system node, with its version when it answers `--version`.
    System(Option<String>),
    Raw(String, RowKind),
}

struct Selection {
    entries: Vec<Entry>,
    /// Nothing matched: the status is 3, like `nvm.sh`.
    missing: bool,
}

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, pattern: Option<&str>) -> Result<Output, CliError> {
    let current = current::detect(context)?.to_string();
    let selection = select(context, pattern.filter(|text| !text.is_empty()), &current)?;
    let rows: Vec<String> = selection
        .entries
        .iter()
        .map(|entry| render(entry, &current))
        .collect();
    let output = Output::stdout(rows.join("\n"));
    Ok(if selection.missing {
        output.with_status(NvmExitCode::InvalidVersion)
    } else {
        output
    })
}

fn select(
    context: &Context<'_>,
    pattern: Option<&str>,
    current: &str,
) -> Result<Selection, CliError> {
    let mut installed = context.installed_versions()?;
    installed.sort();
    match pattern {
        None => everything(context, installed),
        Some("current") => Ok(found(vec![Entry::Raw(
            current.to_owned(),
            RowKind::Current,
        )])),
        Some("system") => system_only(context),
        Some("node") => Ok(of_flavor(installed, Flavor::Node)),
        Some("iojs") => Ok(of_flavor(installed, Flavor::IoJs)),
        Some(name) => by_name(context, installed, name),
    }
}

fn found(entries: Vec<Entry>) -> Selection {
    Selection {
        entries,
        missing: false,
    }
}

fn nothing_found() -> Selection {
    Selection {
        entries: vec![Entry::Raw("N/A".to_owned(), RowKind::Plain)],
        missing: true,
    }
}

fn some_or_nothing(entries: Vec<Entry>) -> Selection {
    if entries.is_empty() {
        nothing_found()
    } else {
        found(entries)
    }
}

fn versions(installed: impl IntoIterator<Item = Version>) -> Vec<Entry> {
    installed.into_iter().map(Entry::Version).collect()
}

fn system_entry(context: &Context<'_>) -> Result<Option<Entry>, CliError> {
    if system_node(context)?.is_none() {
        return Ok(None);
    }
    Ok(Some(Entry::System(system_version(context)?)))
}

/// With nothing installed `nvm.sh` prints `N/A` as if it were an installed
/// version (`            N/A *`), and so does this.
fn everything(context: &Context<'_>, installed: Vec<Version>) -> Result<Selection, CliError> {
    let mut entries = versions(installed);
    entries.extend(system_entry(context)?);
    if entries.is_empty() {
        let raw = Entry::Raw("N/A".to_owned(), RowKind::Installed);
        return Ok(Selection {
            entries: vec![raw],
            missing: true,
        });
    }
    Ok(found(entries))
}

fn system_only(context: &Context<'_>) -> Result<Selection, CliError> {
    Ok(some_or_nothing(
        system_entry(context)?.into_iter().collect(),
    ))
}

fn of_flavor(installed: Vec<Version>, flavor: Flavor) -> Selection {
    some_or_nothing(versions(
        installed
            .into_iter()
            .filter(|version| version.flavor == flavor),
    ))
}

/// An alias (including `stable` and `unstable`) shows what it resolves to;
/// anything else is a version pattern.
fn by_name(
    context: &Context<'_>,
    installed: Vec<Version>,
    name: &str,
) -> Result<Selection, CliError> {
    let is_alias =
        context.alias_store()?.target(name).is_some() || matches!(name, "stable" | "unstable");
    if is_alias {
        return alias_selection(context, name);
    }
    let Ok(pattern) = name.parse::<VersionPattern>() else {
        return Ok(nothing_found());
    };
    Ok(some_or_nothing(versions(
        installed
            .into_iter()
            .filter(|version| pattern.matches(version)),
    )))
}

fn alias_selection(context: &Context<'_>, name: &str) -> Result<Selection, CliError> {
    match resolve_installed(context, name) {
        Ok(Resolved::Installed(version)) => Ok(found(vec![Entry::Version(version)])),
        Ok(Resolved::System) => Ok(found(vec![Entry::System(system_version(context)?)])),
        Ok(Resolved::Missing { .. }) => Ok(nothing_found()),
        Err(CliError::Alias(_)) => Ok(found(vec![Entry::Raw("∞".to_owned(), RowKind::Plain)])),
        Err(error) => Err(error),
    }
}

fn kind_for(text: &str, current: &str) -> RowKind {
    if text == current {
        RowKind::Current
    } else {
        RowKind::Installed
    }
}

fn render(entry: &Entry, current: &str) -> String {
    match entry {
        Entry::Version(version) => {
            let text = version.to_string();
            format_row(&text, kind_for(&text, current))
        }
        Entry::System(version) => {
            format_system_row(kind_for("system", current), version.as_deref())
        }
        Entry::Raw(text, kind) => format_row(text, *kind),
    }
}

#[cfg(test)]
mod tests;

//! `nvm ls [pattern]`: the installed versions, one row each.
//!
//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
//! Unlike `nvm.sh` it does not print a blank row when only a system node
//! exists, and it keeps both io.js and Node versions that share a number.
//! For an alias that resolves to nothing (`ls lts/gallium`, `ls unstable`,
//! `ls sys` with `sys -> system` and no system node) `nvm.sh` prints two
//! `N/A` rows; this prints one.

use crate::commands::Output;
use crate::commands::aliases;
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
    /// Nothing matched: `N/A`.
    NotAvailable,
    Raw(String, RowKind),
}

struct Selection {
    entries: Vec<Entry>,
    /// Nothing matched: the status is 3, like `nvm.sh`.
    missing: bool,
}

/// The command line of `nvm ls`, as `nvm.sh` reads it: the first non-empty
/// word is the pattern, `--no-colors` is accepted (output is always plain).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub no_alias: bool,
}

/// # Errors
/// - [`CliError::Unsupported`] for an unknown `--option`, and for
///   `--no-alias` together with a pattern.
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" | "--no-colors" => {}
            "--no-alias" => options.no_alias = true,
            option if option.starts_with("--") => {
                let message = format!("Unsupported option \"{option}\".");
                return Err(CliError::Unsupported(message));
            }
            word if options.pattern.is_none() && !word.is_empty() => {
                options.pattern = Some(word.to_owned());
            }
            _ => {}
        }
    }
    if options.pattern.is_some() && options.no_alias {
        let message = "`--no-alias` is not supported when a pattern is provided.";
        return Err(CliError::Unsupported(message.to_owned()));
    }
    Ok(options)
}

/// `nvm ls`: the versions, then (without a pattern or `--no-alias`) the
/// aliases, with the exit status of the versions part.
///
/// # Errors
/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run_command(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse_options(args)?;
    let mut output = run(context, options.pattern.as_deref())?;
    if options.pattern.is_none() && !options.no_alias {
        let listed = aliases::list(context, None)?;
        if !listed.stdout.is_empty() {
            output.stdout = format!("{}\n{}", output.stdout, listed.stdout);
        }
    }
    Ok(output)
}

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, pattern: Option<&str>) -> Result<Output, CliError> {
    let current = current::detect(context)?.to_string();
    let mut installed = context.installed_versions()?;
    installed.sort();
    let not_available = not_available_kind(context, &installed)?;
    let pattern = pattern.filter(|text| !text.is_empty());
    let selection = select(context, installed, pattern, &current)?;
    let rows: Vec<String> = selection
        .entries
        .iter()
        .map(|entry| render(entry, &current, not_available))
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
    installed: Vec<Version>,
    pattern: Option<&str>,
    current: &str,
) -> Result<Selection, CliError> {
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
        entries: vec![Entry::NotAvailable],
        missing: true,
    }
}

/// With nothing installed at all (no system node either) `nvm.sh` prints
/// `N/A` as if it were an installed version (`            N/A *`), whatever
/// the pattern; otherwise it is a plain `N/A` row.
fn not_available_kind(context: &Context<'_>, installed: &[Version]) -> Result<RowKind, CliError> {
    let nothing_installed = installed.is_empty() && system_node(context)?.is_none();
    Ok(if nothing_installed {
        RowKind::Installed
    } else {
        RowKind::Plain
    })
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

fn everything(context: &Context<'_>, installed: Vec<Version>) -> Result<Selection, CliError> {
    let mut entries = versions(installed);
    entries.extend(system_entry(context)?);
    Ok(some_or_nothing(entries))
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

fn render(entry: &Entry, current: &str, not_available: RowKind) -> String {
    match entry {
        Entry::Version(version) => {
            let text = version.to_string();
            format_row(&text, kind_for(&text, current))
        }
        Entry::System(version) => {
            format_system_row(kind_for("system", current), version.as_deref())
        }
        Entry::NotAvailable => format_row("N/A", not_available),
        Entry::Raw(text, kind) => format_row(text, *kind),
    }
}

#[cfg(test)]
mod tests;

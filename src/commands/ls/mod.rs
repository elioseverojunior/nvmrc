//! `nvm ls [pattern]`: the installed versions, one row each.
//!
//! The version rows are colored like `nvm_print_versions` when stdout can
//! show colors and `--no-colors` is not given; the alias rows are plain.
//! Unlike `nvm.sh` it does not print a blank row when only a system node
//! exists, and it keeps both io.js and Node versions that share a number.
//! For an alias that resolves to nothing (`ls lts/gallium`, `ls unstable`,
//! `ls sys` with `sys -> system` and no system node) `nvm.sh` prints two
//! `N/A` rows; this prints one.

use crate::commands::Output;
use crate::commands::aliases;
use crate::commands::color_policy;
use crate::commands::current;
use crate::commands::resolve::{Resolved, resolve_installed, system_node, system_version};
use crate::context::Context;
use crate::domain::alias::AliasStore;
use crate::domain::colors::Palette;
use crate::domain::listing::RowKind;
use crate::domain::listing::colored::{paint_row, paint_system_row};
use crate::domain::version::{Flavor, Version, VersionPattern};
use crate::error::{CliError, NvmExitCode};

mod options;
pub use options::{Options, parse_options};

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

/// `nvm ls`: the versions, then (without a pattern or `--no-alias`) the
/// aliases, with the exit status of the versions part.
///
/// # Errors
/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run_command(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse_options(args)?;
    let mut output = run(context, options.pattern.as_deref(), options.no_colors)?;
    if options.pattern.is_none() && !options.no_alias {
        let alias_args = if options.no_colors {
            vec!["--no-colors".to_owned()]
        } else {
            Vec::new()
        };
        let listed = aliases::run(context, &alias_args)?;
        if !listed.stdout.is_empty() {
            output.stdout = format!("{}\n{}", output.stdout, listed.stdout);
        }
    }
    Ok(output)
}

/// `nvm ls [pattern]` without the aliases; `no_colors` is the `--no-colors`
/// flag. An invalid `NVM_COLORS` adds its single warning on stderr.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(
    context: &Context<'_>,
    pattern: Option<&str>,
    no_colors: bool,
) -> Result<Output, CliError> {
    let current = current::detect(context)?.to_string();
    let mut installed = context.installed_versions()?;
    installed.sort();
    let not_available = not_available_kind(context, &installed)?;
    let pattern = pattern.filter(|text| !text.is_empty());
    let selection = select(context, installed, pattern, &current)?;
    let (palette, warning) = color_policy::palette(context);
    let painter = Painter {
        current,
        not_available,
        palette,
        colors: color_policy::detect(context, no_colors).enabled,
    };
    let rows: Vec<String> = selection
        .entries
        .iter()
        .map(|entry| painter.render(entry))
        .collect();
    let mut output = Output::stdout(rows.join("\n"));
    if let Some(warning) = warning {
        output = output.with_stderr(warning);
    }
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

/// What the rows are rendered with: the version in use, how `N/A` shows,
/// and the colors.
struct Painter {
    current: String,
    not_available: RowKind,
    palette: Palette,
    colors: bool,
}

impl Painter {
    fn kind_for(&self, text: &str) -> RowKind {
        if text == self.current {
            RowKind::Current
        } else {
            RowKind::Installed
        }
    }

    fn row(&self, text: &str, kind: RowKind) -> String {
        paint_row(text, kind, &self.palette, self.colors)
    }

    fn render(&self, entry: &Entry) -> String {
        match entry {
            Entry::Version(version) => {
                let text = version.to_string();
                self.row(&text, self.kind_for(&text))
            }
            Entry::System(version) => paint_system_row(
                self.kind_for("system"),
                version.as_deref(),
                &self.palette,
                self.colors,
            ),
            Entry::NotAvailable => self.row("N/A", self.not_available),
            Entry::Raw(text, kind) => self.row(text, *kind),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod colored_tests;

//! `nvm alias` and `nvm alias <prefix>`: list aliases.
//!
//! Three groups, each sorted on the bytes it prints like `command sort`: the
//! alias files, the implicit aliases that have no file (`iojs`, `node`,
//! `stable`, `unstable`), and the `lts/*` files. With colors on, the rows of a
//! group therefore come in the order of their leading color code first
//! (nvm.sh quirk, kept).

use std::path::Path;

use crate::commands::color_policy;
use crate::commands::resolve::shown;
use crate::commands::{Output, alias, unalias};
use crate::context::Context;
use crate::domain::alias::AliasStore;
use crate::domain::alias_format::colored::AliasKind;
use crate::domain::implicit::{IMPLICIT_ALIASES, destination};
use crate::error::{CliError, NvmExitCode};

mod painter;
pub use painter::AliasPainter;

#[derive(Default)]
struct Words {
    name: Option<String>,
    target: Option<String>,
    no_colors: bool,
}

/// `nvm alias [--no-colors] [name [target]]`: the first two words are the
/// name and the target, more are ignored.
fn parse_words(args: &[String]) -> Result<Words, CliError> {
    let mut words = Words::default();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            "--no-colors" => words.no_colors = true,
            option if option.starts_with("--") => {
                let message = format!("Unsupported option \"{option}\".");
                return Err(CliError::Unsupported(message));
            }
            word if words.name.is_none() => words.name = Some(word.to_owned()),
            word if words.target.is_none() => words.target = Some(word.to_owned()),
            _ => {}
        }
    }
    Ok(words)
}

/// `nvm alias`: with no words it lists, with a name it lists the aliases
/// starting with it, with a name and a target it creates the alias, and an
/// explicitly empty target deletes it.
///
/// # Errors
/// - [`CliError::Unsupported`] for an unknown `--option`.
/// - Whatever the listing, creation or deletion fails with.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let Words {
        name,
        target,
        no_colors,
    } = parse_words(args)?;
    match (name, target) {
        (Some(name), Some(target)) if target.is_empty() => unalias::run(context, &[name]),
        (Some(name), _) if name.contains('#') => {
            let message = "Aliases with a comment delimiter (#) are not supported.";
            Err(CliError::InvalidArgument(message.to_owned()))
        }
        (Some(name), Some(target)) => alias::run_with_colors(context, &name, &target, no_colors),
        (Some(name), None) => list(context, Some(&name), no_colors),
        (None, _) => list(context, None, no_colors),
    }
}

/// `nvm alias [prefix]`, colored unless `no_colors` (`--no-colors`) or
/// stdout cannot show colors. An invalid `NVM_COLORS` adds its single
/// warning on stderr when a row is printed.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn list(
    context: &Context<'_>,
    prefix: Option<&str>,
    no_colors: bool,
) -> Result<Output, CliError> {
    let prefix = prefix.unwrap_or_default();
    if prefix.starts_with("lts/") {
        return lts_target(context, prefix);
    }
    let painter = AliasPainter::new(context, color_policy::detect(context, no_colors))?;
    let lines = rows(context, &painter, prefix)?;
    let output = Output::stdout(lines.join("\n"));
    Ok(match painter.warning() {
        Some(warning) if !lines.is_empty() => output.with_stderr(warning),
        _ => output,
    })
}

/// The three groups of rows.
fn rows(
    context: &Context<'_>,
    painter: &AliasPainter,
    prefix: &str,
) -> Result<Vec<String>, CliError> {
    let alias_dir = context.alias_dir()?;
    let mut lines = directory_lines(context, painter, &alias_dir, AliasKind::File, prefix)?;
    lines.extend(implicit_lines(context, painter, &alias_dir, prefix)?);
    lines.extend(directory_lines(
        context,
        painter,
        &alias_dir.join("lts"),
        AliasKind::Lts,
        prefix,
    )?);
    Ok(lines)
}

/// `nvm alias lts/iron` prints the alias file's target as is.
fn lts_target(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
    match context.alias_store()?.target(name) {
        Some(target) => Ok(Output::stdout(target)),
        None => Ok(Output::default()
            .with_stderr("Alias does not exist.")
            .with_status(NvmExitCode::MissingTarget)),
    }
}

fn line(
    context: &Context<'_>,
    painter: &AliasPainter,
    name: &str,
    target: &str,
    kind: AliasKind,
) -> Result<String, CliError> {
    let version = shown(context, target)?;
    Ok(painter.line(name, target, &version, kind))
}

/// The alias files directly inside `directory` whose name starts with
/// `prefix`; those of `alias/lts` ([`AliasKind::Lts`]) are named `lts/` +
/// the file name. Hidden
/// files only match a hidden prefix, like a shell glob.
fn directory_lines(
    context: &Context<'_>,
    painter: &AliasPainter,
    directory: &Path,
    kind: AliasKind,
    prefix: &str,
) -> Result<Vec<String>, CliError> {
    let label = if kind == AliasKind::Lts { "lts/" } else { "" };
    let store = context.alias_store()?;
    let mut lines = Vec::new();
    for entry in context.fs.read_dir(directory).unwrap_or_default() {
        let hidden = entry.name.starts_with('.') && !prefix.starts_with('.');
        if entry.is_dir || hidden || !entry.name.starts_with(prefix) {
            continue;
        }
        let name = format!("{label}{}", entry.name);
        if let Some(target) = store.target(&name) {
            lines.push(line(context, painter, &name, &target, kind)?);
        }
    }
    lines.sort();
    Ok(lines)
}

/// The implicit aliases that have no file of their own, and that match the
/// prefix exactly (a prefix does not select among them).
fn implicit_lines(
    context: &Context<'_>,
    painter: &AliasPainter,
    alias_dir: &Path,
    prefix: &str,
) -> Result<Vec<String>, CliError> {
    let installed = context.installed_versions()?;
    let mut lines = Vec::new();
    for name in IMPLICIT_ALIASES {
        let wanted = prefix.is_empty() || prefix == name;
        if !wanted || context.fs.is_file(&alias_dir.join(name)) {
            continue;
        }
        if let Some(target) = destination(&installed, name) {
            lines.push(line(context, painter, name, &target, AliasKind::Implicit)?);
        }
    }
    lines.sort();
    Ok(lines)
}

#[cfg(test)]
mod colored_tests;
#[cfg(test)]
pub mod fixture;
#[cfg(test)]
mod tests;

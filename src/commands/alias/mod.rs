//! `nvm alias <name> <target>`: create an alias file.
//!
//! Listing aliases (`nvm alias` without a target) lives in
//! [`crate::commands::aliases`], which shares its painter.

use crate::commands::aliases::AliasPainter;
use crate::commands::color_policy::{self, ColorPolicy};
use crate::commands::resolve::{Shown, shown};
use crate::commands::{Output, unalias};
use crate::context::Context;
use crate::domain::alias_format::colored::AliasKind;
use crate::error::CliError;

/// `nvm alias <name> <target>` from the command line: the row is colored
/// unless `no_colors` (`--no-colors`) or stdout cannot show colors, and an
/// exported `NVM_HAS_COLORS=1` colors it whatever the rest says.
///
/// # Errors
/// As [`run`].
pub fn run_with_colors(
    context: &Context<'_>,
    name: &str,
    target: &str,
    no_colors: bool,
) -> Result<Output, CliError> {
    let policy = color_policy::detect_or_forced(context, no_colors);
    create(context, name, target, policy)
}

/// The plain `nvm alias <name> <target>` that `nvm install` runs.
///
/// # Errors
/// - [`CliError::InvalidArgument`] for a name that is empty, `.`, `..`, or
///   contains `#` or `/`.
/// - [`CliError::Io`] when the alias directory cannot be created or the alias
///   file cannot be written.
/// - An empty target deletes the alias instead, with the errors of
///   [`unalias::run`].
///
/// A target that is not installed is only a warning on stderr.
pub fn run(context: &Context<'_>, name: &str, target: &str) -> Result<Output, CliError> {
    create(context, name, target, ColorPolicy::off())
}

/// The version is resolved before the file is written, so `nvm alias a a`
/// shows `N/A`, not a loop. The `! WARNING` comes before the
/// `Invalid color code` line, as nvm.sh prints them.
fn create(
    context: &Context<'_>,
    name: &str,
    target: &str,
    policy: ColorPolicy,
) -> Result<Output, CliError> {
    if target.is_empty() {
        return unalias::run(context, &[name.to_owned()]);
    }
    validate_name(name)?;
    let version = shown(context, target)?;
    let alias_dir = context.alias_dir()?;
    context
        .fs
        .create_dir_all(&alias_dir)
        .map_err(|source| CliError::io(&alias_dir, source))?;
    let alias_file = alias_dir.join(name);
    context
        .fs
        .write_file(&alias_file, &format!("{target}\n"))
        .map_err(|source| CliError::io(&alias_file, source))?;
    let painter = AliasPainter::new(context, policy)?;
    let line = painter.line(name, target, &version, AliasKind::File);
    let mut warnings = Vec::new();
    if version == Shown::NotAvailable {
        warnings.push(format!("! WARNING: Version '{target}' does not exist."));
    }
    warnings.extend(painter.warning().map(str::to_owned));
    Ok(Output::stdout(line).with_stderr(warnings.join("\n")))
}

fn validate_name(name: &str) -> Result<(), CliError> {
    let message = if name.contains('#') {
        "Aliases with a comment delimiter (#) are not supported.".to_owned()
    } else if name.contains('/') {
        "Aliases in subdirectories are not supported.".to_owned()
    } else if matches!(name, "" | "." | "..") {
        format!("invalid alias name: {name}")
    } else {
        return Ok(());
    };
    Err(CliError::InvalidArgument(message))
}

#[cfg(test)]
mod colored_tests;
#[cfg(test)]
mod tests;

//! `nvm ls-remote [pattern]`: the releases a mirror offers, one row each.
//!
//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
//! Every flavor that is listed is downloaded again each time, and the `lts/*`
//! aliases are refreshed from the node index, as in `nvm.sh`.

mod named_aliases;

use crate::commands::Output;
use crate::commands::current;
use crate::commands::remote_index::{Fetched, fetch};
use crate::context::Context;
use crate::domain::listing::{RowKind, format_row};
use crate::domain::remote::{Query, list, normalize_lts, scope};
use crate::domain::remote_format::{FormatInput, format_remote_rows};
use crate::domain::version::Flavor;
use crate::error::{CliError, NvmExitCode};

/// The command line of `nvm ls-remote`, as `nvm.sh` reads it: the first
/// non-empty word is the pattern, `lts/*` and `lts/<name>` mean the LTS
/// filter, and `--no-colors` is accepted (output is always plain).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub lts: Option<String>,
}

/// # Errors
/// [`CliError::Unsupported`] for an unknown `--option`.
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" | "--no-colors" => {}
            "--lts" => options.lts = Some("*".to_owned()),
            option if option.starts_with("--lts=") => {
                options.lts = Some(option["--lts=".len()..].to_owned());
            }
            option if option.starts_with("--") => {
                let message = format!("Unsupported option \"{option}\".");
                return Err(CliError::Unsupported(message));
            }
            word if options.pattern.is_none() && !word.is_empty() => {
                options.pattern = Some(word.to_owned());
                options.take_lts_pattern();
            }
            _ => {}
        }
    }
    Ok(options)
}

impl Options {
    /// `lts/*` and `lts/<name>` given as the pattern are the LTS filter,
    /// unless `--lts` was already given.
    fn take_lts_pattern(&mut self) {
        if self.lts.as_deref().is_some_and(|lts| !lts.is_empty()) {
            return;
        }
        let Some(name) = self.pattern.as_deref().and_then(|p| p.strip_prefix("lts/")) else {
            return;
        };
        self.lts = Some(name.to_owned());
        self.pattern = Some(String::new());
    }
}

/// # Errors
/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse_options(args)?;
    let mut query = Query {
        pattern: options.pattern.clone().filter(|text| !text.is_empty()),
        lts: options.lts.clone().filter(|text| !text.is_empty()),
    };
    let mut warnings = Vec::new();
    let (node_runs, iojs_runs) = match scope(&query) {
        Ok(scope) => (scope.node_runs, scope.iojs_runs),
        Err(error) => return Ok(not_available(vec![error.to_string()])),
    };
    let node = fetch_if(context, node_runs, Flavor::Node, &mut warnings)?;
    if let Some(wanted) = query.lts.as_deref() {
        match lts_filter(context, wanted) {
            Ok(name) => query.lts = Some(name),
            Err(message) => {
                warnings.push(message);
                return Ok(not_available(warnings));
            }
        }
    }
    let iojs = fetch_if(context, iojs_runs, Flavor::IoJs, &mut warnings)?;
    let listing = list(node.as_deref(), iojs.as_deref(), &query)
        .map_err(|error| CliError::InvalidArgument(error.to_string()))?;
    if listing.rows.is_empty() {
        return Ok(not_available(warnings));
    }
    let plain = listing.missing || query.lts.is_some() || options.pattern.is_some();
    let lines = format_rows(context, &listing.rows, plain)?;
    let status = if listing.missing {
        NvmExitCode::InvalidVersion
    } else {
        NvmExitCode::Success
    };
    Ok(Output::stdout(lines.join("\n"))
        .with_stderr(warnings.join("\n"))
        .with_status(status))
}

/// The row `nvm.sh` prints when nothing is listed, with exit status 3.
fn not_available(warnings: Vec<String>) -> Output {
    Output::stdout(format_row("N/A", RowKind::Plain))
        .with_stderr(warnings.join("\n"))
        .with_status(NvmExitCode::InvalidVersion)
}

fn fetch_if(
    context: &Context<'_>,
    wanted: bool,
    flavor: Flavor,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<crate::domain::index::Release>>, CliError> {
    if !wanted {
        return Ok(None);
    }
    let Fetched { releases, warning } = fetch(context, flavor)?;
    warnings.extend(warning);
    Ok(releases)
}

/// The codename a `--lts` argument stands for, once the aliases are fresh.
fn lts_filter(context: &Context<'_>, wanted: &str) -> Result<String, String> {
    let directory = context
        .alias_dir()
        .map_err(|error| error.to_string())?
        .join("lts");
    let mut names: Vec<String> = context
        .fs
        .read_dir(&directory)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.sort();
    normalize_lts(wanted, &names).map_err(|error| error.to_string())
}

fn format_rows(
    context: &Context<'_>,
    rows: &[crate::domain::remote::RemoteRow],
    plain: bool,
) -> Result<Vec<String>, CliError> {
    let installed = context.installed_versions()?;
    let current = current::detect(context)?.to_string();
    let named = if plain {
        Vec::new()
    } else {
        named_aliases::collect(context)?
    };
    Ok(format_remote_rows(&FormatInput {
        rows,
        installed: &installed,
        current: &current,
        latest_alias: (!plain).then_some("node"),
        named_aliases: &named,
    }))
}

#[cfg(test)]
mod tests;

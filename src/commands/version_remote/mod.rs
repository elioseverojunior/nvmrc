//! `nvm version-remote [pattern]`: the one release a description stands for
//! (the newest, with no pattern), downloaded from the mirror.
//!
//! Unlike `nvm.sh`, an invalid `--lts` name (`--lts=Iron`) prints its message
//! once and `N/A` with status 3, whatever the pattern; `nvm.sh` repeats the
//! message and exits 0 after a blank line for `node` and `iojs`.

use crate::commands::Output;
use crate::commands::remote_index::{Fetched, fetch, lts_filter};
use crate::context::Context;
use crate::domain::index::Release;
use crate::domain::remote::resolve::resolve;
use crate::domain::remote::{Query, scope};
use crate::domain::version::Flavor;
use crate::error::{CliError, NvmExitCode};

/// The command line of `nvm version-remote`, as `nvm.sh` reads it: the first
/// non-empty word is the pattern, and `lts/*` or `lts/<name>` as the pattern
/// replace any `--lts`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub lts: Option<String>,
}

/// # Errors
/// [`CliError::Unsupported`] for an unknown `--option` (`--no-colors` too).
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" => {}
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
            }
            _ => {}
        }
    }
    match options.pattern.as_deref() {
        Some("lts/*") => (options.lts, options.pattern) = (Some("*".to_owned()), None),
        Some(text) if text.starts_with("lts/") => {
            options.lts = Some(text["lts/".len()..].to_owned());
            options.pattern = None;
        }
        _ => {}
    }
    Ok(options)
}

/// Which indexes the pattern needs: `iojs` the io.js one, the implicit node
/// aliases the node one, anything else as `ls-remote` does.
fn needs(query: &Query) -> (bool, bool) {
    match query.pattern.as_deref() {
        Some("iojs") => (false, true),
        None | Some("node" | "stable" | "unstable") => (true, false),
        Some(_) => scope(query).map_or((false, false), |scope| (scope.node_runs, scope.iojs_runs)),
    }
}

fn fetch_if(
    context: &Context<'_>,
    wanted: bool,
    flavor: Flavor,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<Release>>, CliError> {
    if !wanted {
        return Ok(None);
    }
    let Fetched { releases, warning } = fetch(context, flavor)?;
    warnings.extend(warning);
    Ok(releases)
}

fn not_available(warnings: &[String]) -> Output {
    Output::stdout("N/A")
        .with_stderr(warnings.join("\n"))
        .with_status(NvmExitCode::InvalidVersion)
}

/// # Errors
/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse_options(args)?;
    let mut query = Query {
        pattern: options.pattern,
        lts: options.lts.filter(|text| !text.is_empty()),
    };
    let (node_runs, iojs_runs) = needs(&query);
    let mut warnings = Vec::new();
    let node = fetch_if(context, node_runs, Flavor::Node, &mut warnings)?;
    if let Some(wanted) = query.lts.as_deref() {
        match lts_filter(context, wanted) {
            Ok(name) => query.lts = Some(name),
            Err(message) => {
                warnings.push(message);
                return Ok(not_available(&warnings));
            }
        }
    }
    let iojs = fetch_if(context, iojs_runs, Flavor::IoJs, &mut warnings)?;
    match resolve(node.as_deref(), iojs.as_deref(), &query) {
        Some(version) => Ok(Output::stdout(version.to_string()).with_stderr(warnings.join("\n"))),
        None => Ok(not_available(&warnings)),
    }
}

#[cfg(test)]
mod tests;

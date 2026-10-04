//! `nvm ls-remote [pattern]`: the releases a mirror offers, one row each.
//!
//! The rows and their annotations are colored like `nvm_print_versions` when
//! stdout can show colors and `--no-colors` is not given; `NVM_NO_COLORS` in
//! the environment is ignored, as `nvm.sh` shadows it.
//! Every flavor that is listed is downloaded again each time, and the `lts/*`
//! aliases are refreshed from the node index, as in `nvm.sh`.

mod named_aliases;
mod options;

pub use options::{Options, parse_options};

use crate::commands::Output;
use crate::commands::color_policy::{self, ColorPolicy};
use crate::commands::current;
use crate::commands::remote_index::{fetch_if, lts_filter};
use crate::context::Context;
use crate::domain::colors::Palette;
use crate::domain::listing::{RowKind, format_row};
use crate::domain::remote::{Query, RemoteRow, list, scope};
use crate::domain::remote_format::{FormatInput, RemoteColors, paint_remote_rows};
use crate::domain::version::Flavor;
use crate::error::{CliError, NvmExitCode};

/// The colors of this run, decided once: the policy, the palette and the
/// single `Invalid color code` warning of an invalid `NVM_COLORS`.
struct Colors {
    policy: ColorPolicy,
    palette: Palette,
    warning: Option<String>,
}

impl Colors {
    fn detect(context: &Context<'_>, no_colors: bool) -> Self {
        let (palette, warning) = color_policy::palette(context);
        Self {
            policy: color_policy::detect(context, no_colors),
            palette,
            warning,
        }
    }

    fn remote(&self) -> Option<RemoteColors<'_>> {
        self.policy.enabled.then_some(RemoteColors {
            palette: &self.palette,
            italics: self.policy.italics,
        })
    }

    /// `nvm_print_versions` reads the palette for every listing, the `N/A`
    /// one included, so the warning follows whatever else went to stderr.
    fn warn(&self, mut output: Output) -> Output {
        if let Some(warning) = &self.warning {
            if !output.stderr.is_empty() {
                output.stderr.push('\n');
            }
            output.stderr.push_str(warning);
        }
        output
    }
}

/// # Errors
/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse_options(args)?;
    let colors = Colors::detect(context, options.no_colors);
    let output = listing(context, &options, &colors)?;
    Ok(colors.warn(output))
}

fn listing(context: &Context<'_>, options: &Options, colors: &Colors) -> Result<Output, CliError> {
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
    if let Err(message) = apply_lts_filter(context, &mut query) {
        warnings.push(message);
        return Ok(not_available(warnings));
    }
    let iojs = fetch_if(context, iojs_runs, Flavor::IoJs, &mut warnings)?;
    let listing = list(node.as_deref(), iojs.as_deref(), &query)
        .map_err(|error| CliError::InvalidArgument(error.to_string()))?;
    if listing.rows.is_empty() {
        return Ok(not_available(warnings));
    }
    let plain = listing.missing || query.lts.is_some() || options.pattern.is_some();
    let lines = format_rows(context, &listing.rows, plain, colors)?;
    Ok(printed(lines, &warnings, listing.missing))
}

/// The listing as `nvm.sh` prints it: exit status 3 when some part found
/// nothing, even if other rows were found.
fn printed(lines: Vec<String>, warnings: &[String], missing: bool) -> Output {
    let status = if missing {
        NvmExitCode::InvalidVersion
    } else {
        NvmExitCode::Success
    };
    Output::stdout(lines.join("\n"))
        .with_stderr(warnings.join("\n"))
        .with_status(status)
}

/// Replaces the LTS filter by the codename it stands for, once the aliases
/// are fresh; the message `nvm.sh` prints when it stands for none.
fn apply_lts_filter(context: &Context<'_>, query: &mut Query) -> Result<(), String> {
    if let Some(wanted) = query.lts.as_deref() {
        query.lts = Some(lts_filter(context, wanted)?);
    }
    Ok(())
}

/// The row `nvm.sh` prints when nothing is listed, with exit status 3.
fn not_available(warnings: Vec<String>) -> Output {
    Output::stdout(format_row("N/A", RowKind::Plain))
        .with_stderr(warnings.join("\n"))
        .with_status(NvmExitCode::InvalidVersion)
}

fn format_rows(
    context: &Context<'_>,
    rows: &[RemoteRow],
    plain: bool,
    colors: &Colors,
) -> Result<Vec<String>, CliError> {
    let installed = context.installed_versions()?;
    let current = current::detect(context)?.to_string();
    let named = if plain {
        Vec::new()
    } else {
        named_aliases::collect(context)?
    };
    let input = FormatInput {
        rows,
        installed: &installed,
        current: &current,
        latest_alias: (!plain).then_some("node"),
        named_aliases: &named,
    };
    Ok(paint_remote_rows(&input, colors.remote()))
}

#[cfg(test)]
mod colored_tests;
#[cfg(test)]
mod tests;

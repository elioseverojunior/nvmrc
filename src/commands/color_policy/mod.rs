//! Whether the colors of nvm.sh are on: `nvm_has_colors` and
//! `nvm_has_italics`, with the `NVM_HAS_COLORS=1` override of the fork.

use std::path::Path;

use crate::context::Context;
use crate::domain::path_search::find_in_path;

#[cfg(test)]
mod tests;

/// Colors are shown from this many terminal colors up (`-ge 8`).
const MINIMUM_COLORS: i64 = 8;
/// What nvm.sh asks terminfo about when `TERM` is unset or empty.
const FALLBACK_TERM: &str = "vt100";

/// What a command may print: SGR colors, and italics on top of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorPolicy {
    pub enabled: bool,
    pub italics: bool,
}

impl ColorPolicy {
    #[must_use]
    pub fn off() -> Self {
        Self {
            enabled: false,
            italics: false,
        }
    }
}

/// `tput -T <term> <capability>` through the `Process` port; `None` when
/// `tput` is not on `PATH` (`nvm_has tput`) or cannot be started.
fn tput(
    context: &Context<'_>,
    term: &str,
    capability: &str,
) -> Option<crate::ports::ProcessOutput> {
    let path_variable = context.env.var_os("PATH").unwrap_or_default();
    let program = find_in_path(context.fs, &path_variable, "tput")?;
    run_tput(context, &program, &["-T", term, capability])
}

fn run_tput(
    context: &Context<'_>,
    program: &Path,
    args: &[&str],
) -> Option<crate::ports::ProcessOutput> {
    context.process().run(program, args).ok()
}

/// The `[ "${NVM_NUM_COLORS:--1}" -ge 8 ]` of nvm.sh: blank, failing or
/// non-integer output counts as -1. Like `[`, surrounding whitespace and a
/// sign are accepted.
fn has_enough_colors(output: Option<crate::ports::ProcessOutput>) -> bool {
    output
        .filter(|output| output.success)
        .and_then(|output| output.stdout.trim().parse::<i64>().ok())
        .is_some_and(|colors| colors >= MINIMUM_COLORS)
}

/// `nvm_has_colors` and `nvm_has_italics`. `no_colors_flag` is whether the
/// command was given `--no-colors`; `NVM_NO_COLORS` and `NO_COLOR` in the
/// environment are ignored, as the commands of nvm.sh shadow the first and
/// nothing reads the second.
#[must_use]
pub fn detect(context: &Context<'_>, no_colors_flag: bool) -> ColorPolicy {
    if no_colors_flag || !context.terminal().stdout_is_terminal() {
        return ColorPolicy::off();
    }
    let term = context
        .env
        .var("TERM")
        .filter(|term| !term.is_empty())
        .unwrap_or_else(|| FALLBACK_TERM.to_owned());
    if !has_enough_colors(tput(context, &term, "colors")) {
        return ColorPolicy::off();
    }
    let italics = tput(context, &term, "sitm").is_some_and(|output| output.success);
    ColorPolicy {
        enabled: true,
        italics,
    }
}

/// The exported `NVM_HAS_COLORS` is exactly `1`: the fork's override that
/// beats a pipe, `TERM=dumb` and `--no-colors`. Any other value is ignored.
#[must_use]
pub fn forced(context: &Context<'_>) -> bool {
    context.env.var("NVM_HAS_COLORS").as_deref() == Some("1")
}

/// [`detect`], except that [`forced`] turns the colors on. Italics stay what
/// detection says: `nvm_has_italics` runs the real `nvm_has_colors`, which the
/// override does not reach.
#[must_use]
pub fn detect_or_forced(context: &Context<'_>, no_colors_flag: bool) -> ColorPolicy {
    let detected = detect(context, no_colors_flag);
    ColorPolicy {
        enabled: detected.enabled || forced(context),
        italics: detected.italics,
    }
}

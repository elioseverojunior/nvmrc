//! `nvm set-colors <five letters>`: `nvm_set_colors` and its case branch.
//!
//! The binary cannot change its parent shell, so the `export NVM_COLORS`
//! of nvm.sh is returned as a [`Script`] for the `nvm` function to eval
//! (the init snippets capture the code of `set-colors`, as of `use`).

use crate::commands::Output;
use crate::commands::color_policy;
use crate::context::Context;
use crate::domain::colors::{is_valid_setting, sgr, wrap};
use crate::error::CliError;
use crate::shell::Script;

/// The variable the setting is exported to.
const VARIABLE: &str = "NVM_COLORS";
/// The only value of `NVM_NO_COLORS` that turns the colors off here.
const NO_COLORS_FLAG: &str = "--no-colors";
/// The confirmation's prefix.
const CONFIRMATION: &str = "Setting colors to: ";
/// The line after the uncolored confirmation.
const NO_COLORS_WARNING: &str =
    "WARNING: Colors may not display because they are not supported in this shell.";
/// The error of an invalid setting, colored whether or not colors are on.
const INVALID_SETTING: &str = "\x1b[1;37mPlease pass in five \x1b[1;31mvalid color codes\
\x1b[1;37m. Choose from: rRgGbBcCyYmMkKeW\x1b[0m";

/// `nvm set-colors [<setting>]`: only the first argument counts, as in
/// nvm.sh (`nvm_set_colors "${1-}"`), so `nvm set-colors rgbcm extra` is
/// valid and `nvm set-colors -- rgbcm` is not.
///
/// A valid setting (exactly five of `rRgGbBcCyYmMkKeW`) is exported and
/// confirmed on stdout, each letter in its own color when the colors are on
/// (`nvm_has_colors`, which reads an `NVM_NO_COLORS` of exactly
/// `--no-colors`, unlike `ls`, `ls-remote` and `alias`; `NVM_HAS_COLORS`
/// does not force them here). An invalid one prints an empty line on stdout
/// and the error on stderr, and the status is 0, as nvm.sh does.
///
/// DELIBERATE DEVIATION: on an invalid setting nvm.sh also prints its whole
/// `nvm --help` on stderr before the error; the port does not (its help is
/// clap's).
///
/// # Errors
/// [`CliError::Shell`] only if the script cannot be built, which the fixed
/// variable name makes a programming error.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let setting = args.first().map_or("", String::as_str);
    if !is_valid_setting(setting) {
        return Ok(Output::default()
            .with_blank_stdout()
            .with_stderr(INVALID_SETTING));
    }
    let no_colors = context.env.var("NVM_NO_COLORS").as_deref() == Some(NO_COLORS_FLAG);
    let colors = color_policy::detect(context, no_colors).enabled;
    let script = Script::new().export(VARIABLE, setting)?;
    Ok(Output::stdout(confirmation(setting, colors)).with_script(script))
}

/// The confirmation of `nvm_set_colors`: the letters each in their own
/// color and unseparated, or spaced and followed by the warning.
fn confirmation(setting: &str, colors: bool) -> String {
    if colors {
        let letters: String = setting
            .chars()
            .map(|letter| wrap(sgr(letter), &letter.to_string()))
            .collect();
        return format!("{CONFIRMATION}{letters}");
    }
    let letters: Vec<String> = setting.chars().map(String::from).collect();
    format!("{CONFIRMATION}{}\n{NO_COLORS_WARNING}", letters.join(" "))
}

#[cfg(test)]
mod tests;

//! The command line of `nvm ls`.

use crate::error::CliError;

/// The command line of `nvm ls`, as `nvm.sh` reads it: the first non-empty
/// word is the pattern, `--no-colors` turns the colors off.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub no_alias: bool,
    pub no_colors: bool,
}

/// # Errors
/// - [`CliError::Unsupported`] for an unknown `--option`, and for
///   `--no-alias` together with a pattern.
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            "--no-colors" => options.no_colors = true,
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

//! The command line of `nvm ls-remote`.

use crate::error::CliError;

/// The command line of `nvm ls-remote`, as `nvm.sh` reads it: the first
/// non-empty word is the pattern, `lts/*` and `lts/<name>` mean the LTS
/// filter, and `--no-colors` (anywhere) turns the colors off.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub lts: Option<String>,
    pub no_colors: bool,
}

/// # Errors
/// [`CliError::Unsupported`] for an unknown `--option`.
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            "--no-colors" => options.no_colors = true,
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

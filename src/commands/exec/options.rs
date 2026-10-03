//! Digest 6.1: options are read up to the version (or the command): a `--`
//! stops them and stays, any other unknown `--x` is an error.

use crate::error::CliError;

/// What `nvm exec` was asked for.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Options {
    pub silent: bool,
    /// `--lts` (`*`) or `--lts=<name>`; `--lts=` is no LTS.
    pub lts: Option<String>,
    /// Everything from the first argument that is not an option: the
    /// version and the command, or only the command.
    pub rest: Vec<String>,
}

pub(super) fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for (index, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "--silent" => options.silent = true,
            "--lts" => options.lts = Some("*".to_owned()),
            "" => {}
            "--" => return Ok(options.with_rest(&args[index..])),
            long if long.starts_with("--") => {
                let Some(name) = long.strip_prefix("--lts=") else {
                    return Err(CliError::Unsupported(format!(
                        "Unsupported option \"{long}\"."
                    )));
                };
                options.lts = Some(name.to_owned()).filter(|name| !name.is_empty());
            }
            _ => return Ok(options.with_rest(&args[index..])),
        }
    }
    Ok(options)
}

impl Options {
    fn with_rest(mut self, rest: &[String]) -> Self {
        self.rest = rest.to_vec();
        self
    }
}

//! The command line of `nvm install`, as `nvm.sh` reads it: options first,
//! then the version (or `lts/*`, `lts/<name>`), and what follows it is not
//! looked at, except for the options that this port does not have.

use crate::error::CliError;

/// Options that need a source build, `npm` or a `.nvmrc`: not in this port yet.
const NOT_YET: [&str; 3] = ["-s", "-j", "--offline"];
const NOT_YET_WITH_VALUE: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];
const NOT_YET_AFTER_VERSION: [&str; 2] = ["--save", "-w"];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// The version, partial version or alias: what follows the options.
    pub version: String,
    /// `*` or a codename (as given): from `--lts[=name]`, `lts/*` or `lts/<name>`.
    pub lts: Option<String>,
    /// From `--default` or `--alias=<name>`.
    pub alias: Option<String>,
    /// There was a version argument (even an empty one).
    pub version_given: bool,
    /// The LTS filter came from `--lts` with no version, which `nvm.sh`
    /// announces ("Installing latest LTS version.").
    pub announce_lts: bool,
    /// `--latest-npm`: upgrade the `npm` of the installed version.
    pub latest_npm: bool,
    /// `--skip-default-packages`: leave `$NVM_DIR/default-packages` alone.
    pub skip_default_packages: bool,
}

fn unsupported(option: &str) -> CliError {
    CliError::Unsupported(format!("The option \"{option}\" is not supported yet."))
}

fn already_given() -> CliError {
    let message =
        "--default and --alias are mutually exclusive, and may not be provided more than once";
    CliError::InvalidOptions(message.to_owned())
}

fn is_not_yet(option: &str, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| option == *name || option.starts_with(&format!("{name}=")))
}

/// # Errors
/// - [`CliError::Unsupported`] for `---x`, and for the options listed above.
/// - [`CliError::InvalidOptions`] for `--default` with `--alias`, or either twice.
pub fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    let mut rest = args.iter().peekable();
    while let Some(option) = rest.next_if(|arg| is_option(arg)) {
        match option.as_str() {
            "-b" | "--no-progress" => {}
            "--latest-npm" => options.latest_npm = true,
            "--skip-default-packages" => options.skip_default_packages = true,
            "--lts" => options.lts = Some("*".to_owned()),
            "--default" => set_alias(&mut options, "default")?,
            other if other.starts_with("--lts=") => {
                options.lts = Some(other["--lts=".len()..].to_owned());
            }
            other if other.starts_with("--alias=") => {
                set_alias(&mut options, &other["--alias=".len()..])?;
            }
            other if other.starts_with("---") => {
                let message = "arguments with `---` are not supported - this is likely a typo";
                return Err(CliError::Unsupported(message.to_owned()));
            }
            other => return Err(unsupported(other)),
        }
    }
    if let Some(version) = rest.next() {
        options.version.clone_from(version);
        options.version_given = true;
    }
    for option in rest {
        if option == "--skip-default-packages" {
            options.skip_default_packages = true;
        } else if is_not_yet(option, &NOT_YET_WITH_VALUE)
            || NOT_YET_AFTER_VERSION.contains(&option.as_str())
        {
            return Err(unsupported(option));
        }
    }
    options.announce_lts = options.lts.is_some() && options.version.is_empty();
    take_lts_version(&mut options);
    Ok(options)
}

/// A word that `nvm.sh` reads as an option rather than as the version.
fn is_option(arg: &str) -> bool {
    matches!(
        arg,
        "-b" | "--no-progress" | "--lts" | "--default" | "--latest-npm" | "--skip-default-packages"
    ) || arg.starts_with("--lts=")
        || arg.starts_with("--alias=")
        || arg.starts_with("---")
        || NOT_YET.contains(&arg)
        || is_not_yet(arg, &NOT_YET_WITH_VALUE)
        || NOT_YET_AFTER_VERSION.contains(&arg)
}

fn set_alias(options: &mut Options, name: &str) -> Result<(), CliError> {
    if options.alias.is_some() {
        return Err(already_given());
    }
    options.alias = Some(name.to_owned());
    Ok(())
}

/// `lts/*` and `lts/<name>` given as the version are the LTS filter, whatever
/// `--lts` said.
fn take_lts_version(options: &mut Options) {
    let Some(name) = options.version.strip_prefix("lts/") else {
        return;
    };
    options.lts = Some(name.to_owned());
    options.version.clear();
}

#[cfg(test)]
mod tests;

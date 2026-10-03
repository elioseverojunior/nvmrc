//! The command line of `nvm install`, as `nvm.sh` reads it: options first,
//! then the version (or `lts/*`, `lts/<name>`). What follows the version is
//! looked at only for `--skip-default-packages` and the two options that name
//! a version to take packages from; every other word there (and `--save` is
//! one) is handed to `./configure` when the version is built from source, as
//! in `nvm.sh`.

use crate::error::CliError;

/// Both options mean the same; `nvm.sh` words its messages after the one used.
const REINSTALL_OPTIONS: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// `--reinstall-packages-from=<version>` (or `--copy-packages-from`): the
    /// version, as given, whose global packages are installed again.
    pub reinstall_from: Option<String>,
    /// `--offline`: use what is installed or cached, and never the network.
    pub offline: bool,
    /// `--save` or `-w`: write the version to `.nvmrc` in the current directory.
    pub save: bool,
    /// `-s`: build from source, whatever there is a binary for.
    pub no_binary: bool,
    /// `-b`: never build from source, not even when the binary fails.
    pub no_source: bool,
    /// `-j <jobs>`: how many `make` jobs, as given.
    pub make_jobs: Option<String>,
    /// The words after the version that no option takes: arguments for
    /// `./configure`.
    pub extra: Vec<String>,
}

pub const BOTH_OFF: &str =
    "-s and -b cannot be set together since they would skip install from both binary and source";

fn already_given() -> CliError {
    let message =
        "--default and --alias are mutually exclusive, and may not be provided more than once";
    CliError::InvalidOptions(message.to_owned())
}

/// `--reinstall-packages-from=18`, `--copy-packages-from=18` or either of
/// them with no value.
fn reinstall_option(option: &str) -> Option<(&'static str, Option<&str>)> {
    REINSTALL_OPTIONS.iter().find_map(|name| {
        if option == *name {
            return Some((*name, None));
        }
        let value = option.strip_prefix(&format!("{name}="))?;
        Some((*name, Some(value)))
    })
}

fn read_reinstall(options: &mut Options, option: &str) -> Result<bool, CliError> {
    let Some((name, value)) = reinstall_option(option) else {
        return Ok(false);
    };
    let message = match value {
        None => format!("If {name} is provided, it must point to an installed version of node using `=`."),
        Some(_) if options.reinstall_from.is_some() && name == "--copy-packages-from" => {
            "--reinstall-packages-from may not be provided more than once, or combined with `--copy-packages-from`".to_owned()
        }
        Some(_) if options.reinstall_from.is_some() => {
            "--reinstall-packages-from may not be provided more than once".to_owned()
        }
        Some("") => format!("If {name} is provided, it must point to an installed version of node."),
        Some(version) => {
            options.reinstall_from = Some(version.to_owned());
            return Ok(true);
        }
    };
    Err(CliError::InvalidOptions(message))
}

/// # Errors
/// - [`CliError::Unsupported`] for `---x`, and for the options listed above.
/// - [`CliError::InvalidOptions`] for `--default` with `--alias`, or either twice.
pub fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    let mut rest = args.iter().peekable();
    while let Some(option) = rest.next_if(|arg| is_option(arg)) {
        read_leading(&mut options, option, &mut rest)?;
    }
    if let Some(version) = rest.next() {
        options.version.clone_from(version);
        options.version_given = true;
    }
    for option in rest {
        read_trailing(&mut options, option)?;
    }
    options.announce_lts = options.lts.is_some() && options.version.is_empty();
    take_lts_version(&mut options);
    Ok(options)
}

/// One option before the version; `-j` takes the next word as its value.
fn read_leading(
    options: &mut Options,
    option: &str,
    rest: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
) -> Result<(), CliError> {
    match option {
        "--no-progress" => {}
        "-s" => set_build_mode(options, true)?,
        "-b" => set_build_mode(options, false)?,
        "-j" => options.make_jobs = Some(rest.next().cloned().unwrap_or_default()),
        "--latest-npm" => options.latest_npm = true,
        "--offline" => options.offline = true,
        "--save" | "-w" => set_save(options)?,
        "--skip-default-packages" => options.skip_default_packages = true,
        "--lts" => options.lts = Some("*".to_owned()),
        "--default" => set_alias(options, "default")?,
        other => read_valued(options, other)?,
    }
    Ok(())
}

/// The options that carry a value (`--lts=`, `--alias=`, the reinstall pair),
/// and `---x`, which is a typo.
fn read_valued(options: &mut Options, option: &str) -> Result<(), CliError> {
    if let Some(name) = option.strip_prefix("--lts=") {
        options.lts = Some(name.to_owned());
    } else if let Some(name) = option.strip_prefix("--alias=") {
        set_alias(options, name)?;
    } else if option.starts_with("---") {
        let message = "arguments with `---` are not supported - this is likely a typo";
        return Err(CliError::Unsupported(message.to_owned()));
    } else {
        read_reinstall(options, option)?;
    }
    Ok(())
}

/// One word after the version: an option that is read there, or an argument
/// for `./configure`.
fn read_trailing(options: &mut Options, option: &str) -> Result<(), CliError> {
    if option == "--skip-default-packages" {
        options.skip_default_packages = true;
    } else if !read_reinstall(options, option)? {
        options.extra.push(option.to_owned());
    }
    Ok(())
}

/// A word that `nvm.sh` reads as an option rather than as the version.
fn is_option(arg: &str) -> bool {
    matches!(
        arg,
        "-b" | "-s"
            | "-j"
            | "--no-progress"
            | "--lts"
            | "--default"
            | "--latest-npm"
            | "--skip-default-packages"
            | "--offline"
            | "--save"
            | "-w"
    ) || arg.starts_with("--lts=")
        || arg.starts_with("--alias=")
        || arg.starts_with("---")
        || reinstall_option(arg).is_some()
}

fn set_build_mode(options: &mut Options, binary_off: bool) -> Result<(), CliError> {
    if (binary_off && options.no_source) || (!binary_off && options.no_binary) {
        return Err(CliError::InvalidOptions(BOTH_OFF.to_owned()));
    }
    if binary_off {
        options.no_binary = true;
    } else {
        options.no_source = true;
    }
    Ok(())
}

fn set_save(options: &mut Options) -> Result<(), CliError> {
    if options.save {
        let message = "--save and -w may only be provided once";
        return Err(CliError::InvalidOptions(message.to_owned()));
    }
    options.save = true;
    Ok(())
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

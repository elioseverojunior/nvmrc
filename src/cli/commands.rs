//! The subcommands and what each one runs.

use clap::Subcommand;

use crate::commands::{self, Output};
use crate::context::Context;
use crate::error::CliError;

#[derive(Subcommand)]
pub(super) enum Command {
    /// Print the highest installed version matching a version or alias
    /// (the current version when no argument is given).
    Version { pattern: Option<String> },
    /// Print the version of the node that is active in this shell.
    Current,
    /// List the installed versions and the aliases (`--no-alias` omits them;
    /// a pattern lists only the versions matching it).
    #[command(visible_alias = "list")]
    Ls {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// List the versions a mirror offers (`--lts[=name]` keeps the LTS
    /// releases; a pattern keeps the versions matching it).
    #[command(name = "ls-remote", visible_alias = "list-remote")]
    LsRemote {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Print the cache directory (`dir`) or empty it (`clear`).
    Cache {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Download and install a version (a partial version, an alias, `--lts`,
    /// `lts/<name>`); `--default` or `--alias=<name>` also make an alias.
    #[command(visible_alias = "i")]
    Install {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Upgrade the `npm` of the node in use to the newest one that works on it.
    #[command(name = "install-latest-npm")]
    InstallLatestNpm {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Install, in the node in use, the global packages of another version.
    #[command(name = "reinstall-packages")]
    ReinstallPackages {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// The same as `reinstall-packages`.
    #[command(name = "copy-packages")]
    CopyPackages {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Remove an installed version (`--lts` and `--lts=<name>` pick one by
    /// its LTS alias) and the aliases that name it.
    Uninstall {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Print the newest release a version, alias or `--lts[=name]` stands for
    /// on the mirror (`N/A` when there is none).
    #[command(name = "version-remote")]
    VersionRemote {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Print the path to the node binary of a version or alias (or of the
    /// `.nvmrc` version when none is given).
    Which {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// List aliases, show those starting with a name, or create an alias for
    /// a version (an empty target deletes the alias).
    Alias {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Delete an alias.
    Unalias { names: Vec<String> },
    /// Switch this shell to a version, alias, `--lts[=name]` or the `.nvmrc`
    /// version (`--silent`, `--delete-prefix`, `--save`).
    Use {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Undo `nvm use` in this shell.
    Deactivate {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    // DELIBERATE DEVIATION: unlike nvm.sh, `-h`, `help` and `--help` after
    // `exec` and `run` are not taken for a request for nvm's help: every
    // argument after the subcommand is passed through to the command.
    /// Run a command with a version, alias, `--lts[=name]` or the `.nvmrc`
    /// version on `PATH` (`--silent`), and exit with its status.
    Exec {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Run `node` (or `iojs`) of a version, alias, `--lts[=name]` or the
    /// `.nvmrc` version with the given arguments (`--silent`).
    Run {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Print the shell code that defines the `nvm` function, for
    /// `eval "$(nvm init bash)"` in the shell's startup file (`--no-use`
    /// skips switching to the default version; `--install` installs it).
    Init {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// What the `nvm` function runs at shell start: `use`, `install` or
    /// `none` (nvm.sh's `nvm_auto`).
    #[command(name = "__auto", hide = true)]
    Auto {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
}

pub(super) fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
    match command {
        Command::Version { pattern } => {
            commands::version::run(context, pattern.as_deref().unwrap_or("current"))
        }
        Command::Current => commands::current::run(context),
        Command::Ls { args } => commands::ls::run_command(context, args),
        Command::LsRemote { args } => commands::ls_remote::run(context, args),
        Command::VersionRemote { args } => commands::version_remote::run(context, args),
        Command::Cache { args } => commands::cache::run(context, args),
        Command::Install { args } => commands::install::run(context, args),
        Command::InstallLatestNpm { args } => commands::install_latest_npm::run(context, args),
        Command::ReinstallPackages { args } => packages(context, "reinstall-packages", args),
        Command::CopyPackages { args } => packages(context, "copy-packages", args),
        Command::Uninstall { args } => commands::uninstall::run(context, args),
        Command::Which { args } => commands::which::run_command(context, args),
        Command::Alias { args } => commands::aliases::run(context, args),
        Command::Unalias { names } => commands::unalias::run(context, names),
        Command::Use { args } => commands::use_version::run(context, args),
        Command::Deactivate { args } => commands::deactivate::run(context, args),
        Command::Exec { args } => commands::exec::run(context, args),
        Command::Run { args } => commands::run::run(context, args),
        Command::Init { args } => commands::init::run(args),
        Command::Auto { args } => commands::auto::run(context, args),
    }
}

/// `reinstall-packages` and `copy-packages`, which differ only by name.
fn packages(context: &Context<'_>, name: &str, args: &[String]) -> Result<Output, CliError> {
    commands::reinstall_packages::run(context, name, args)
}

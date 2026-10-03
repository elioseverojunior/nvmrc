//! Argument parsing and the translation of results into streams and exit codes.

use std::ffi::OsString;
use std::io::Write;

use clap::{Parser, Subcommand};

use crate::adapters::retrying_http::RetryingHttp;
use crate::adapters::std_env::StdEnv;
use crate::adapters::std_fs::StdFileSystem;
use crate::adapters::std_process::StdProcess;
use crate::adapters::std_sleeper::StdSleeper;
use crate::adapters::ureq_http::UreqHttp;
use crate::commands::{self, Output};
use crate::context::Context;
use crate::domain::http_header::sanitize_auth_header;
use crate::error::{CliError, NvmExitCode};
use crate::ports::Env;

#[derive(Parser)]
#[command(name = "nvm", version, about = "Node Version Manager, in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
    /// Print the newest release a version, alias or `--lts[=name]` stands for
    /// on the mirror (`N/A` when there is none).
    #[command(name = "version-remote")]
    VersionRemote {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Print the path to the node binary of a version or alias.
    Which { version: Option<String> },
    /// List aliases, show those starting with a name, or create an alias for
    /// a version (an empty target deletes the alias).
    Alias {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Delete an alias.
    Unalias { names: Vec<String> },
}

fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
    match command {
        Command::Version { pattern } => {
            commands::version::run(context, pattern.as_deref().unwrap_or("current"))
        }
        Command::Current => commands::current::run(context),
        Command::Ls { args } => commands::ls::run_command(context, args),
        Command::LsRemote { args } => commands::ls_remote::run(context, args),
        Command::VersionRemote { args } => commands::version_remote::run(context, args),
        Command::Cache { args } => commands::cache::run(context, args),
        Command::Which { version } => commands::which::run(context, version.as_deref()),
        Command::Alias { args } => commands::aliases::run(context, args),
        Command::Unalias { names } => commands::unalias::run(context, names),
    }
}

/// Writes `text` and flushes, so a closed or full stream is noticed.
fn emit(stream: &mut dyn Write, text: &str) -> std::io::Result<()> {
    stream.write_all(text.as_bytes())?;
    stream.flush()
}

fn exit_code_for_write(result: std::io::Result<()>, success: NvmExitCode) -> u8 {
    match result {
        Ok(()) => success.code(),
        Err(_) => NvmExitCode::Failure.code(),
    }
}

/// Runs the CLI and returns the process exit code. Output goes to the given
/// writers so tests can capture it.
pub fn run<I, T>(args: I, context: &Context<'_>, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) if error.use_stderr() => {
            // Nowhere left to report a failed stderr write.
            let _ = emit(err, &error.to_string());
            // nvm.sh: unknown commands and usage errors are 127.
            return NvmExitCode::NotFound.code();
        }
        Err(error) => {
            return exit_code_for_write(emit(out, &error.to_string()), NvmExitCode::Success);
        }
    };
    match dispatch(&cli.command, context) {
        Ok(output) => finish(&output, out, err),
        // nvm.sh prints N/A on stdout, not stderr.
        Err(error @ CliError::NotInstalled) => {
            let output = Output::stdout(error.to_string()).with_status(error.exit_code());
            finish(&output, out, err)
        }
        Err(error) => {
            let output = Output::default()
                .with_stderr(error.to_string())
                .with_status(error.exit_code());
            finish(&output, out, err)
        }
    }
}

/// Prints diagnostics first, then the result, and finishes with the status the
/// command asked for, unless writing the result failed.
fn finish(output: &Output, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if !output.stderr.is_empty() {
        // Nowhere left to report a failed stderr write.
        let _ = emit(err, &format!("{}\n", output.stderr));
    }
    if output.stdout.is_empty() {
        return output.status.code();
    }
    exit_code_for_write(emit(out, &format!("{}\n", output.stdout)), output.status)
}

/// Entry point shared by the `nvmrc` and `nvm` binaries.
#[must_use]
pub fn run_from_env() -> u8 {
    let process = StdProcess::default();
    let auth_header = StdEnv
        .var("NVM_AUTH_HEADER")
        .filter(|value| !value.is_empty())
        .map(|value| sanitize_auth_header(&value));
    let network = UreqHttp::new(auth_header);
    let http = RetryingHttp::new(&network, &StdSleeper);
    let context = Context::new(&StdFileSystem, &StdEnv)
        .with_process(&process)
        .with_http(&http);
    run(
        std::env::args_os(),
        &context,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

#[cfg(test)]
mod tests;

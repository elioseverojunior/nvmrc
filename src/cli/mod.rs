//! Argument parsing and the translation of results into streams and exit codes.

use std::ffi::OsString;
use std::io::Write;

use clap::{Parser, Subcommand};

use crate::adapters::std_env::StdEnv;
use crate::adapters::std_fs::StdFileSystem;
use crate::adapters::std_process::StdProcess;
use crate::commands::{self, Output};
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};

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
    /// List the installed versions, optionally only those matching a pattern.
    #[command(visible_alias = "list")]
    Ls { pattern: Option<String> },
    /// Print the path to the node binary of a version or alias.
    Which { version: Option<String> },
    /// Create an alias for a version (an empty target deletes the alias).
    Alias { name: String, target: String },
    /// Delete an alias.
    Unalias { names: Vec<String> },
}

fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
    match command {
        Command::Version { pattern } => {
            commands::version::run(context, pattern.as_deref().unwrap_or("current"))
        }
        Command::Current => commands::current::run(context),
        Command::Ls { pattern } => commands::ls::run(context, pattern.as_deref()),
        Command::Which { version } => commands::which::run(context, version.as_deref()),
        Command::Alias { name, target } => commands::alias::run(context, name, target),
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
    let context = Context::new(&StdFileSystem, &StdEnv).with_process(&StdProcess);
    run(
        std::env::args_os(),
        &context,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

#[cfg(test)]
mod tests;

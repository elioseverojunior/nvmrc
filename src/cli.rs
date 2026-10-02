//! Argument parsing and the translation of results into streams and exit codes.

use std::ffi::OsString;
use std::io::Write;

use clap::{Parser, Subcommand};

use crate::adapters::std_env::StdEnv;
use crate::adapters::std_fs::StdFileSystem;
use crate::commands;
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
    /// Print the highest installed version matching a version or alias.
    Version { pattern: String },
}

fn dispatch(command: &Command, context: &Context<'_>) -> Result<String, CliError> {
    match command {
        Command::Version { pattern } => commands::version::run(context, pattern),
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
        Err(error) => {
            if error.use_stderr() {
                let _ = write!(err, "{error}");
                return NvmExitCode::Failure.code();
            }
            let _ = write!(out, "{error}");
            return NvmExitCode::Success.code();
        }
    };
    match dispatch(&cli.command, context) {
        Ok(text) => {
            let _ = writeln!(out, "{text}");
            NvmExitCode::Success.code()
        }
        Err(error) => {
            let _ = writeln!(err, "{error}");
            error.exit_code().code()
        }
    }
}

/// Entry point shared by the `nvmrc` and `nvm` binaries.
#[must_use]
pub fn run_from_env() -> u8 {
    let context = Context {
        fs: &StdFileSystem,
        env: &StdEnv,
    };
    run(
        std::env::args_os(),
        &context,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn run_cli(args: &[&str]) -> (u8, String, String) {
        let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn version_prints_the_match_and_exits_zero() {
        assert_eq!(
            run_cli(&["nvm", "version", "20"]),
            (0, "v20.1.0\n".into(), String::new())
        );
    }

    #[test]
    fn version_reports_not_installed_on_stderr_with_exit_3() {
        assert_eq!(
            run_cli(&["nvm", "version", "16"]),
            (3, String::new(), "N/A\n".into())
        );
    }

    #[test]
    fn a_usage_error_exits_non_zero_without_stdout() {
        let (code, out, err) = run_cli(&["nvm", "bogus"]);
        assert_ne!(code, 0);
        assert!(out.is_empty() && !err.is_empty());
    }
}

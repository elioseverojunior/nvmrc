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
            return NvmExitCode::Failure.code();
        }
        Err(error) => {
            return exit_code_for_write(emit(out, &error.to_string()), NvmExitCode::Success);
        }
    };
    match dispatch(&cli.command, context) {
        Ok(text) => exit_code_for_write(emit(out, &format!("{text}\n")), NvmExitCode::Success),
        // nvm.sh prints N/A on stdout, not stderr.
        Err(error @ CliError::NotInstalled) => {
            exit_code_for_write(emit(out, &format!("{error}\n")), error.exit_code())
        }
        Err(error) => {
            let _ = emit(err, &format!("{error}\n"));
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
    fn version_reports_not_installed_on_stdout_with_exit_3() {
        assert_eq!(
            run_cli(&["nvm", "version", "16"]),
            (3, "N/A\n".into(), String::new())
        );
    }

    #[test]
    fn version_of_a_non_version_name_is_na_on_stdout_with_exit_3() {
        assert_eq!(
            run_cli(&["nvm", "version", "foo"]),
            (3, "N/A\n".into(), String::new())
        );
    }

    #[test]
    fn an_alias_loop_is_reported_on_stderr_with_exit_8() {
        let fs = FakeFileSystem::default()
            .with_file("/n/alias/a", "b")
            .with_file("/n/alias/b", "a");
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let args = ["nvm", "version", "a"];
        let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
        assert_eq!(code, 8);
        assert!(out.is_empty() && !err.is_empty());
    }

    struct BrokenPipe;

    impl Write for BrokenPipe {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }

    #[test]
    fn a_failed_stdout_write_exits_1() {
        let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let mut err = Vec::new();
        let args = ["nvm", "version", "20"];
        let code = run(
            args,
            &Context { fs: &fs, env: &env },
            &mut BrokenPipe,
            &mut err,
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn a_failed_stderr_write_does_not_panic() {
        let fs = FakeFileSystem::default();
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let mut out = Vec::new();
        let args = ["nvm", "version", "a=b"];
        let code = run(
            args,
            &Context { fs: &fs, env: &env },
            &mut out,
            &mut BrokenPipe,
        );
        assert_ne!(code, 0);
    }

    #[test]
    fn a_usage_error_exits_non_zero_without_stdout() {
        let (code, out, err) = run_cli(&["nvm", "bogus"]);
        assert_ne!(code, 0);
        assert!(out.is_empty() && !err.is_empty());
    }
}

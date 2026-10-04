//! Argument parsing and the translation of results into streams and exit codes.

mod channel;
mod child;
mod commands;

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use clap::Parser;

use self::commands::{Command, dispatch};

use crate::adapters::fd_channel::FdChannel;
use crate::adapters::retrying_http::RetryingHttp;
use crate::adapters::sha256_digest::Sha256Digest;
use crate::adapters::std_clock::StdClock;
use crate::adapters::std_cpu::StdCpu;
use crate::adapters::std_env::StdEnv;
use crate::adapters::std_fs::StdFileSystem;
use crate::adapters::std_process::StdProcess;
use crate::adapters::std_prompt::StdPrompt;
use crate::adapters::std_sleeper::StdSleeper;
use crate::adapters::std_terminal::StdTerminal;
use crate::adapters::tar_archive::TarArchive;
use crate::adapters::ureq_http::UreqHttp;
use crate::commands::Output;
use crate::context::Context;
use crate::domain::http_header::sanitize_auth_header;
use crate::domain::platform::Platform;
use crate::error::{CliError, NvmExitCode};
use crate::ports::{Env, FileSystem};

#[derive(Parser)]
#[command(name = "nvm", version, about = "Node Version Manager, in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    let args = arguments(args);
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
    let output = dispatch(&cli.command, context).unwrap_or_else(|error| failure(&error));
    finish("nvm", &output, context, out, err)
}

/// The commands that see a `--` right after their name.
const KEEP_DOUBLE_DASH: [&str; 3] = ["exec", "run", "set-colors"];

/// clap takes a `--` right after a subcommand for the end of the options
/// and drops it, but nvm.sh's `exec` stops its own options there, `run`
/// hands it to the script and `set-colors` takes it for the (invalid)
/// setting; doubling it makes clap pass the user's one on.
fn keep_leading_double_dash(mut args: Vec<OsString>) -> Vec<OsString> {
    let passes_it_on = args
        .get(1)
        .is_some_and(|command| KEEP_DOUBLE_DASH.iter().any(|name| command == name));
    if passes_it_on && args.get(2).is_some_and(|argument| argument == "--") {
        args.insert(2, OsString::from("--"));
    }
    args
}

/// What clap is given: a bare `nvm` is `nvm --help`, as nvm.sh's function
/// does, and a `--` after `exec`, `run` or `set-colors` is kept.
fn arguments<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if args.len() == 1 {
        args.push(OsString::from("--help"));
    }
    keep_leading_double_dash(args)
}

/// What a failed command prints: its message, on stderr except for `N/A`,
/// which nvm.sh prints on stdout.
fn failure(error: &CliError) -> Output {
    let output = match error {
        CliError::NotInstalled => Output::stdout(error.to_string()),
        _ => Output::default().with_stderr(error.to_string()),
    };
    output.with_status(error.exit_code())
}

/// Prints the streams, then runs the program the command left to run, if
/// any, whose status wins.
fn finish(
    tool: &str,
    output: &Output,
    context: &Context<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let printed = print(output, context, out, err);
    match &output.spawn {
        Some(invocation) => child::run_child(tool, context, invocation, err),
        None => printed,
    }
}

/// Prints diagnostics first, then the result, and finishes with the status the
/// command asked for, unless writing the result failed. The shell code of the
/// command travels as `channel` says.
fn print(output: &Output, context: &Context<'_>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let delivery = channel::deliver(output, context);
    if !delivery.stderr.is_empty() {
        // Nowhere left to report a failed stderr write.
        let _ = emit(err, &delivery.stderr);
    }
    if delivery.stdout.is_empty() {
        return delivery.status.code();
    }
    exit_code_for_write(emit(out, &delivery.stdout), delivery.status)
}

/// Calls `body` with the context of the real machine: the standard file
/// system, environment, process runner, network and clock, and the channel
/// of the `nvm` function when it opened one.
fn with_real_context<R>(body: impl FnOnce(&Context<'_>) -> R) -> R {
    // First: the descriptor must be taken over before anything else opens one.
    let channel = FdChannel::from_env(&StdEnv);
    let process = StdProcess::default();
    let auth_header = StdEnv
        .var("NVM_AUTH_HEADER")
        .filter(|value| !value.is_empty())
        .map(|value| sanitize_auth_header(&value));
    let network = UreqHttp::new(auth_header);
    let http = RetryingHttp::new(&network, &StdSleeper);
    let musl = StdFileSystem.is_file(Path::new("/etc/alpine-release"));
    let platform = Platform::from_host(std::env::consts::OS, std::env::consts::ARCH, musl);
    let context = Context::new(&StdFileSystem, &StdEnv)
        .with_process(&process)
        .with_http(&http)
        .with_digest(&Sha256Digest)
        .with_archive(&TarArchive)
        .with_sleeper(&StdSleeper)
        .with_cpu(&StdCpu)
        .with_terminal(&StdTerminal)
        .with_clock(&StdClock)
        .with_prompt(&StdPrompt)
        .with_platform(platform);
    match &channel {
        Some(channel) => body(&context.with_script_channel(channel)),
        None => body(&context),
    }
}

/// Entry point shared by the `nvmrc` and `nvm` binaries.
#[must_use]
pub fn run_from_env() -> u8 {
    with_real_context(|context| {
        run(
            std::env::args_os(),
            context,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        )
    })
}

/// Runs the command a script gave `nvm-exec` (the arguments after the
/// program name, verbatim) and returns the process exit code.
#[must_use]
pub fn run_nvm_exec<I, T>(
    command: I,
    context: &Context<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let command: Vec<String> = command
        .into_iter()
        .map(|argument| argument.into().to_string_lossy().into_owned())
        .collect();
    let output = crate::commands::nvm_exec::run(context, &command);
    finish("nvm-exec", &output, context, out, err)
}

/// Entry point of the `nvm-exec` binary.
#[must_use]
pub fn nvm_exec_from_env() -> u8 {
    with_real_context(|context| {
        run_nvm_exec(
            std::env::args_os().skip(1),
            context,
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        )
    })
}

#[cfg(test)]
mod tests;

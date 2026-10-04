use super::args::{Options, parse};
use super::fixtures::Setup;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakePrompt};
use crate::shell::init::Shell;

fn parsed(args: &[&str]) -> Result<Options, CliError> {
    let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
    parse(&args)
}

#[test]
fn every_flag_is_read() {
    let options = parsed(&["--dry-run", "-y", "--undo", "--shell", "zsh"]).unwrap();
    let expected = Options {
        dry_run: true,
        yes: true,
        undo: true,
        shell: Some(Shell::Zsh),
    };
    assert_eq!(options, expected);
    assert_eq!(
        parsed(&["--yes", "--shell=fish"]).unwrap().shell,
        Some(Shell::Fish)
    );
    assert_eq!(parsed(&[]).unwrap(), Options::default());
}

#[test]
fn an_unknown_shell_lists_the_supported_ones() {
    let Err(CliError::Usage(message)) = parsed(&["--shell", "tcsh"]) else {
        panic!("expected a usage error");
    };
    assert!(
        message.contains("bash, zsh, sh, dash, ksh, fish"),
        "{message}"
    );
}

#[test]
fn a_missing_shell_name_is_a_usage_error() {
    let Err(CliError::Usage(message)) = parsed(&["--shell"]) else {
        panic!("expected a usage error");
    };
    assert!(message.starts_with("--shell needs a name."), "{message}");
}

#[test]
fn another_option_is_unsupported() {
    let error = parsed(&["--force"]).unwrap_err();
    assert_eq!(error.to_string(), "Unsupported option \"--force\".");
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
    assert_eq!(error.exit_code().code(), 55);
}

#[test]
fn a_positional_word_is_a_usage_error() {
    let Err(CliError::Usage(message)) = parsed(&["now"]) else {
        panic!("expected a usage error");
    };
    assert!(
        message.contains("Unexpected argument \"now\"."),
        "{message}"
    );
}

#[test]
fn run_rejects_bad_arguments_before_reading_anything() {
    let fs = FakeFileSystem::default();
    let setup = Setup {
        fs: &fs,
        env: &FakeEnv::default(),
        process: &FakeProcess::default(),
        prompt: &FakePrompt::unavailable(),
    };
    assert!(matches!(
        setup.migrate(&["--nope"]),
        Err(CliError::Unsupported(_))
    ));
}

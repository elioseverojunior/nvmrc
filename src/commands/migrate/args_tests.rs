use super::args::{Options, parse};
use super::fixtures::Setup;
use super::shell_of;
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

#[test]
fn the_shell_of_a_file_comes_from_its_name() {
    let env = FakeEnv::default();
    let cases = [
        ("/h/.zshrc", Shell::Zsh),
        ("/h/.zprofile", Shell::Zsh),
        ("/h/.zlogin", Shell::Zsh),
        ("/h/.zshenv", Shell::Zsh),
        ("/h/.bashrc", Shell::Bash),
        ("/h/.bash_profile", Shell::Bash),
        ("/h/.bash_login", Shell::Bash),
        ("/h/.kshrc", Shell::Ksh),
        ("/h/.config/fish/config.fish", Shell::Fish),
        ("/h/.config/fish/conf.d/x.fish", Shell::Fish),
        ("/h/.profile", Shell::Bash),
    ];
    for (path, shell) in cases {
        assert_eq!(shell_of(path, &env), shell, "{path}");
    }
}

#[test]
fn the_profile_follows_a_supported_login_shell() {
    let cases = [
        ("/bin/zsh", Shell::Zsh),
        ("/bin/ksh", Shell::Ksh),
        ("/bin/dash", Shell::Dash),
        ("/usr/bin/tcsh", Shell::Bash),
    ];
    for (login, shell) in cases {
        let env = FakeEnv::default().with_var("SHELL", login);
        assert_eq!(shell_of("/h/.profile", &env), shell, "{login}");
    }
}

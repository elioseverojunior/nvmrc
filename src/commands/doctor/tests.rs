use super::{USAGE, run};
use crate::commands::Output;
use crate::commands::conflict::fixtures::{HOME, home_env, link, users_dotfiles, users_env};
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeEnv, FakeFileSystem};

fn doctor(fs: &FakeFileSystem, env: &FakeEnv, args: &[&str]) -> Result<Output, CliError> {
    let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
    run(&Context::new(fs, env), &args)
}

fn report(fs: &FakeFileSystem, env: &FakeEnv, args: &[&str]) -> Output {
    doctor(fs, env, args).unwrap()
}

const INSTALL_SH: &str = "export NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

#[test]
fn no_profile_file_is_not_a_failure() {
    let output = report(&FakeFileSystem::default(), &home_env(), &[]);
    assert_eq!(output.stdout, "nvm doctor: no shell profile files found");
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn a_clean_setup_exits_zero() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", "export PATH=$PATH:/x\n");
    let output = report(&fs, &home_env(), &[]);
    assert_eq!(
        output.stdout,
        "nvm doctor: scanned 1 file(s)\n\nResult: no conflicts"
    );
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn the_install_script_lines_are_a_loader_and_a_completion_migrate_fixes() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", INSTALL_SH);
    let env = home_env().with_var("NVM_DIR", "/Users/u/.nvm");
    let output = report(&fs, &env, &[]);
    let fix = "      fix: run `nvm migrate` (comments the line out and adds the nvmrc init line)";
    let expected = [
        "nvm doctor: scanned 1 file(s)",
        "",
        "/Users/u/.bashrc",
        "  2: nvm.sh loader  [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm",
        fix,
        "  3: nvm bash_completion  [ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion",
        fix,
        "",
        "Info:",
        "  /Users/u/.bashrc:1: NVM_DIR export: kept by `nvm migrate`",
        "  /Users/u/.bashrc:2: file not found: /Users/u/.nvm/nvm.sh",
        "  /Users/u/.bashrc:3: file not found: /Users/u/.nvm/bash_completion",
        "",
        "Result: 2 conflict(s), 2 can be fixed with `nvm migrate`",
    ];
    assert_eq!(output.stdout, expected.join("\n"));
    assert_eq!(output.status, NvmExitCode::Failure);
}

const STUB_FIX: &str = "      fix: the stub loads nvm.sh on first use, and an `nvm` stub after \
the init line hides nvmrc: delete it";

/// The block of the user's `.zshrc`, then the one of `lazy-functions.zsh`.
const USERS_FILE_BLOCKS: [&str; 16] = [
    "/Users/u/.zshrc -> /Users/u/dotfiles/zsh/.zshrc",
    "  3: oh-my-zsh nvm plugin  nvm",
    "      fix: remove `nvm` from `plugins=(...)`; it does nothing while the nvmrc binary is \
on PATH but depends on it",
    "",
    "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh",
    "  1: lazy stub  nvm() {",
    STUB_FIX,
    "  2: nvm unset  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
    "      fix: it removes a lazy stub once nvm.sh is loaded: delete it with the stub",
    "  3: lazy loader  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
    "      fix: remove or replace this loader by hand: nvmrc has its own function (see \
`eval \"$(nvmrc init <shell>)\"`)",
    "      patch: replace the line with: eval \"$(nvmrc init zsh)\"",
    "  6: lazy stub  npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
    STUB_FIX,
    "",
    "Info:",
];

#[test]
fn the_users_tree_shows_the_stubs_the_plugin_hint_and_the_links() {
    let output = report(&users_dotfiles(), &users_env(), &["--shell", "zsh"]);
    let mut expected = vec!["nvm doctor: scanned 7 file(s)", ""];
    expected.extend(USERS_FILE_BLOCKS);
    expected.extend([
        "  /Users/u/.zshenv:3: NVM_DIR export: kept by `nvm migrate`",
        "  /Users/u/.oh-my-zsh/oh-my-zsh.sh:2: not followed: unset $plugin",
        "  /Users/u/dotfiles/zsh/scripts/lazy-functions.zsh:3: not followed: unset $nvm_prefix",
        "",
        "Result: 4 conflict(s)",
    ]);
    assert_eq!(output.stdout, expected.join("\n"));
    assert_eq!(output.status, NvmExitCode::Failure);
}

#[test]
fn a_symlinked_zshrc_shows_its_canonical_path() {
    let fs = FakeFileSystem::default().with_file("/Users/u/dots/zshrc", INSTALL_SH);
    link(&fs, "dots/zshrc", &format!("{HOME}/.zshrc"));
    let text = report(&fs, &home_env(), &[]).stdout;
    assert!(
        text.contains("\n/Users/u/.zshrc -> /Users/u/dots/zshrc\n"),
        "{text}"
    );
}

#[test]
fn shell_limits_the_roots_in_both_spellings() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", INSTALL_SH)
        .with_file("/Users/u/.zshrc", "export A=1\n");
    for args in [["--shell", "zsh"], ["--shell=zsh", "--shell=zsh"]] {
        let output = report(&fs, &home_env(), &args);
        assert_eq!(output.status, NvmExitCode::Success, "{args:?}");
        assert!(output.stdout.starts_with("nvm doctor: scanned 1 file(s)"));
    }
    assert_eq!(report(&fs, &home_env(), &[]).status, NvmExitCode::Failure);
}

#[test]
fn a_shell_without_files_says_so() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", INSTALL_SH);
    let output = report(&fs, &home_env(), &["--shell", "fish"]);
    assert_eq!(output.stdout, "nvm doctor: no shell profile files found");
}

#[test]
fn an_unknown_shell_lists_the_shells() {
    let fs = FakeFileSystem::default();
    let Err(CliError::Usage(message)) = doctor(&fs, &home_env(), &["--shell", "tcsh"]) else {
        panic!("expected a usage error");
    };
    assert!(
        message.contains("bash, zsh, sh, dash, ksh, fish"),
        "{message}"
    );
    assert_eq!(CliError::Usage(message).exit_code(), NvmExitCode::NotFound);
}

#[test]
fn a_missing_shell_name_and_positional_words_are_usage_errors() {
    let fs = FakeFileSystem::default();
    for args in [&["--shell"][..], &["zsh"][..]] {
        let result = doctor(&fs, &home_env(), args);
        assert!(matches!(&result, Err(CliError::Usage(text)) if text.contains(USAGE)));
    }
}

#[test]
fn another_option_is_unsupported() {
    let fs = FakeFileSystem::default();
    let result = doctor(&fs, &home_env(), &["--fix"]);
    assert!(
        matches!(&result, Err(CliError::Unsupported(text)) if text == "Unsupported option \"--fix\".")
    );
}

#[test]
fn a_missing_sourced_file_is_info_not_an_error() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", ". ~/gone.sh\n");
    let output = report(&fs, &home_env(), &[]);
    assert!(
        output
            .stdout
            .contains("Info:\n  /Users/u/.bashrc:1: file not found: /Users/u/gone.sh")
    );
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn nvm_sh_and_its_completion_are_reported_as_loaders_and_never_scanned() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", INSTALL_SH)
        .with_file("/Users/u/.nvm/nvm.sh", "nvm() {\n  nvm_echo hi\n}\n")
        .with_file("/Users/u/.nvm/bash_completion", "nvm_echo x\n");
    let env = home_env().with_var("NVM_DIR", "/Users/u/.nvm");
    let output = report(&fs, &env, &[]);
    assert!(output.stdout.starts_with("nvm doctor: scanned 1 file(s)"));
    assert!(!output.stdout.contains("lazy stub"));
    assert!(!output.stdout.contains("helper call"));
}

#[test]
fn control_bytes_of_a_finding_are_shown_in_caret_notation() {
    let fs = FakeFileSystem::default().with_file(
        "/Users/u/.bashrc",
        "printf '\x1b]0;x\x07'; source ~/.nvm/nvm.sh\n",
    );
    let output = report(&fs, &home_env(), &[]);
    assert!(
        output.stdout.contains("printf '^[]0;x^G'; source"),
        "{}",
        output.stdout
    );
    assert!(!output.stdout.contains(['\x1b', '\x07']));
}

#[test]
fn a_profile_that_is_not_utf8_is_a_warning_and_a_failure() {
    let fs = FakeFileSystem::default();
    let bashrc = std::path::Path::new("/Users/u/.bashrc");
    crate::ports::FileSystem::write_bytes(&fs, bashrc, b"# caf\xe9\n. ~/.nvm/nvm.sh\n").unwrap();
    let output = report(&fs, &home_env(), &[]);
    assert_eq!(
        output.stdout,
        "nvm doctor: scanned 0 file(s)\n\n\
Warning: not scanned, so they may still load nvm.sh:\n  \
/Users/u/.bashrc: not read: stream did not contain valid UTF-8\n\n\
Result: no conflicts found; 1 file(s) not read"
    );
    assert_eq!(output.status, NvmExitCode::Failure);
}

#[test]
fn a_looping_profile_link_is_reported_not_skipped() {
    let fs = FakeFileSystem::default();
    link(&fs, "/Users/u/.bashrc", "/Users/u/.bashrc");
    let output = report(&fs, &home_env(), &[]);
    assert!(
        output.stdout.contains("/Users/u/.bashrc: not read:"),
        "{}",
        output.stdout
    );
    assert_eq!(output.status, NvmExitCode::Failure);
}

use super::*;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem};

fn installed() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/node/v18.9.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
}

fn which_with(fs: &FakeFileSystem, path: &str, name: Option<&str>) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    run(&Context::new(fs, &env), name, false)
}

fn which(name: &str) -> Result<Output, CliError> {
    which_with(&installed(), "/usr/bin", Some(name))
}

#[test]
fn prints_the_node_binary_of_the_highest_match() {
    let output = which("20").unwrap();
    assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
}

#[test]
fn node_is_the_latest_installed_node() {
    let output = which("node").unwrap();
    assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
}

#[test]
fn follows_aliases() {
    let fs = installed().with_file("/n/alias/default", "v18");
    let output = which_with(&fs, "/usr/bin", Some("default")).unwrap();
    assert_eq!(output, Output::stdout("/n/versions/node/v18.9.0/bin/node"));
}

#[test]
fn iojs_binaries_live_under_the_io_js_directory() {
    let output = which("iojs-v3.0.0").unwrap();
    assert_eq!(output, Output::stdout("/n/versions/io.js/v3.0.0/bin/node"));
}

#[test]
fn current_means_the_active_version() {
    let path = "/n/versions/node/v18.9.0/bin";
    let output = which_with(&installed(), path, Some("current")).unwrap();
    assert_eq!(output, Output::stdout("/n/versions/node/v18.9.0/bin/node"));
}

#[test]
fn a_missing_version_gets_the_install_hint_and_exit_1() {
    let error = which("16").unwrap_err();
    let expected = "N/A: version \"v16\" is not yet installed.\n\n\
                    You need to run `nvm install 16` to install and use it.";
    assert_eq!(error.to_string(), expected);
    assert_eq!(error.exit_code(), NvmExitCode::Failure);
}

#[test]
fn a_missing_alias_target_shows_the_chain() {
    let fs = installed().with_file("/n/alias/old", "16");
    let error = which_with(&fs, "/usr/bin", Some("old")).unwrap_err();
    let expected = "N/A: version \"old -> v16\" is not yet installed.\n\n\
                    You need to run `nvm install old` to install and use it.";
    assert_eq!(error.to_string(), expected);
}

#[test]
fn an_alias_loop_has_exit_code_8_and_names_the_alias() {
    let fs = installed().with_file("/n/alias/loop", "loop");
    let error = which_with(&fs, "/usr/bin", Some("loop")).unwrap_err();
    assert_eq!(
        error.to_string(),
        "The alias \"loop\" leads to an infinite loop. Aborting."
    );
    assert_eq!(error.exit_code(), NvmExitCode::AliasLoop);
}

#[test]
fn system_is_the_first_node_outside_nvm_dir() {
    let fs = installed().with_file("/usr/bin/node", "");
    let path = "/n/versions/node/v20.1.0/bin:/usr/bin";
    let output = which_with(&fs, path, Some("system")).unwrap();
    assert_eq!(output, Output::stdout("/usr/bin/node"));
}

#[test]
fn system_without_a_system_node_is_exit_127() {
    let path = "/n/versions/node/v20.1.0/bin";
    let error = which_with(&installed(), path, Some("system")).unwrap_err();
    assert_eq!(error.to_string(), "System version of node not found.");
    assert_eq!(error.exit_code(), NvmExitCode::NotFound);
}

#[test]
fn an_alias_to_system_prints_the_system_node() {
    let fs = installed()
        .with_file("/usr/bin/node", "")
        .with_file("/n/alias/default", "system");
    let output = which_with(&fs, "/usr/bin", Some("default")).unwrap();
    assert_eq!(output, Output::stdout("/usr/bin/node"));
}

#[test]
fn an_alias_to_system_without_a_system_node_is_not_installed() {
    let fs = installed().with_file("/n/alias/default", "system");
    let error = which_with(&fs, "/usr/bin", Some("default")).unwrap_err();
    let expected = "N/A: version \"default -> system\" is not yet installed.\n\n\
                    You need to run `nvm install default` to install and use it.";
    assert_eq!(error.to_string(), expected);
    assert_eq!(error.exit_code(), NvmExitCode::Failure);
}

#[test]
fn an_empty_version_directory_is_not_installed() {
    let fs = FakeFileSystem::default().with_dir("/n/versions/node/v18.0.0");
    let error = which_with(&fs, "/usr/bin", Some("18")).unwrap_err();
    let expected = "N/A: version \"v18\" is not yet installed.\n\n\
                    You need to run `nvm install 18` to install and use it.";
    assert_eq!(error.to_string(), expected);
    assert_eq!(error.exit_code(), NvmExitCode::Failure);
}

#[test]
fn current_without_an_active_node_keeps_the_typed_name() {
    let error = which_with(&installed(), "/usr/bin", Some("current")).unwrap_err();
    let expected = "N/A: version \"current\" is not yet installed.\n\n\
                    You need to run `nvm install current` to install and use it.";
    assert_eq!(error.to_string(), expected);
}

fn which_in(fs: &FakeFileSystem, args: &[&str]) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin")
        .with_var("PWD", "/proj/sub");
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    run_command(&Context::new(fs, &env), &args)
}

fn with_nvmrc(content: &str) -> FakeFileSystem {
    installed()
        .with_dir("/proj/sub")
        .with_file("/proj/.nvmrc", content)
}

#[test]
fn without_a_version_the_nvmrc_is_read_and_announced() {
    let output = which_in(&with_nvmrc("18\n"), &[]).unwrap();
    assert_eq!(
        output.stdout,
        "Found '/proj/.nvmrc' with version <18>\n/n/versions/node/v18.9.0/bin/node"
    );
}

#[test]
fn silent_hides_the_found_line() {
    let output = which_in(&with_nvmrc("18\n"), &["--silent"]).unwrap();
    assert_eq!(output, Output::stdout("/n/versions/node/v18.9.0/bin/node"));
}

#[test]
fn an_explicit_version_ignores_the_nvmrc() {
    let output = which_in(&with_nvmrc("18\n"), &["20"]).unwrap();
    assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
}

#[test]
fn the_last_positional_wins_and_double_dash_is_ignored() {
    let output = which_in(&installed(), &["--", "18", "20"]).unwrap();
    assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
}

#[test]
fn a_missing_nvmrc_gives_the_message_then_the_usage_and_127() {
    let error = which_in(&installed().with_dir("/proj/sub"), &[]).unwrap_err();
    let text = error.to_string();
    assert!(text.starts_with("No version provided and no .nvmrc file found\nUsage: nvm which"));
    assert_eq!(error.exit_code(), NvmExitCode::NotFound);
}

#[test]
fn a_missing_nvmrc_with_silent_gives_only_the_usage() {
    let error = which_in(&installed().with_dir("/proj/sub"), &["--silent"]).unwrap_err();
    assert!(error.to_string().starts_with("Usage: nvm which"));
}

#[test]
fn an_invalid_nvmrc_gives_its_message_then_the_usage_even_when_silent() {
    let error = which_in(&with_nvmrc("18\n20\n"), &["--silent"]).unwrap_err();
    let text = error.to_string();
    assert!(text.starts_with("invalid .nvmrc!\n"));
    assert!(text.contains("non-commented content parsed:\n18\n20\nUsage: nvm which"));
    assert!(!text.contains("Please see"));
}

#[test]
fn a_found_version_that_is_not_installed_keeps_the_found_line_on_stdout() {
    let output = which_in(&with_nvmrc("16\n"), &[]).unwrap();
    assert_eq!(output.stdout, "Found '/proj/.nvmrc' with version <16>");
    assert!(
        output
            .stderr
            .starts_with("N/A: version \"v16\" is not yet installed.")
    );
    assert_eq!(output.status, NvmExitCode::Failure);
}

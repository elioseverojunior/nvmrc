//! `--save` / `-w` (`nvm_write_nvmrc`): written to `$PWD/.nvmrc` after the
//! version is resolved and before it is checked; it never stops `use`.

use std::path::Path;

use super::*;
use crate::ports::FileSystem;

fn saved(fs: &FakeFileSystem) -> Option<String> {
    fs.read_to_string(Path::new("/proj/.nvmrc")).ok()
}

fn wrote(version: &str) -> String {
    format!("Wrote version number ({version}) to .nvmrc")
}

fn save_with(args: &[&str]) -> (FakeFileSystem, Result<Target, Halt>, Output) {
    let fs = lab();
    let (target, output) = run_in(&fs, &env_on(BASE_PATH), args);
    (fs, target, output)
}

#[test]
fn save_writes_the_resolved_version_and_says_so() {
    for (args, version) in [(["--save", "18"], "v18.20.4"), (["-w", "20"], "v20.11.1")] {
        let (fs, target, output) = save_with(&args);
        assert!(matches!(target, Ok(Target::Installed { .. })), "{args:?}");
        assert_eq!(saved(&fs), Some(format!("{version}\n")));
        assert_eq!(
            (output.stdout, output.stderr),
            (wrote(version), String::new())
        );
    }
}

#[test]
fn silent_saves_without_a_word() {
    let (fs, target, output) = save_with(&["18", "--save", "--silent"]);
    assert!(target.is_ok());
    assert_eq!(saved(&fs).as_deref(), Some("v18.20.4\n"));
    assert_eq!(output, Output::default());
}

#[test]
fn save_with_an_lts_writes_the_version_it_leads_to() {
    for args in [["--save", "--lts"], ["--save", "lts/*"]] {
        let (fs, _, output) = save_with(&args);
        assert_eq!(saved(&fs).as_deref(), Some("v20.11.1\n"), "{args:?}");
        assert_eq!(output.stdout, wrote("v20.11.1"));
    }
}

#[test]
fn nothing_is_written_for_what_does_not_resolve_and_the_failure_follows() {
    let rows = [
        (&["--save", "99"][..], NvmExitCode::InvalidVersion),
        (&["--save", "system"], NvmExitCode::NotFound),
        (&["--save", "loopa"], NvmExitCode::AliasLoop),
        (&["--save"], NvmExitCode::NotFound),
        (&["--save", "current"], NvmExitCode::InvalidVersion),
        (&["--save", "-w", "18"], NvmExitCode::InvalidOptions),
    ];
    for (args, status) in rows {
        let (fs, target, output) = save_with(args);
        assert_eq!(target, Err(Halt { status }), "{args:?}");
        assert_eq!(saved(&fs), None, "{args:?}");
        assert_eq!(output.stdout, "", "{args:?}");
    }
}

#[test]
fn a_version_directory_without_its_node_is_not_saved() {
    let fs = lab().with_dir("/n/versions/node/v24.0.0");
    let (target, output) = run_in(&fs, &env_on(BASE_PATH), &["--save", "24"]);
    assert_eq!(
        target,
        Err(Halt {
            status: NvmExitCode::InvalidVersion
        })
    );
    assert_eq!((saved(&fs), output.stdout), (None, String::new()));
}

#[test]
fn system_is_saved_as_system_when_there_is_one() {
    let fs = lab().with_executable("/sys/bin/node", "");
    let (target, output) = run_in(&fs, &env_on("/sys/bin:/usr/bin"), &["--save", "system"]);
    assert!(matches!(target, Ok(Target::System(_))));
    assert_eq!(saved(&fs).as_deref(), Some("system\n"));
    assert_eq!(output.stdout, wrote("system"));
}

#[test]
fn the_found_line_comes_before_the_wrote_line_and_the_file_is_in_pwd() {
    let fs = lab().with_file("/proj/.nvmrc", "18\n");
    let env = env_on(BASE_PATH).with_var("PWD", "/proj/sub");
    let (target, output) = run_in(&fs, &env, &["--save"]);
    assert!(target.is_ok());
    let expected = format!(
        "Found '/proj/.nvmrc' with version <18>\n{}",
        wrote("v18.20.4")
    );
    assert_eq!(output.stdout, expected);
    let written = fs.read_to_string(Path::new("/proj/sub/.nvmrc")).ok();
    assert_eq!(written.as_deref(), Some("v18.20.4\n"));
}

#[test]
fn a_failed_write_warns_and_use_goes_on() {
    let fs = lab();
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", BASE_PATH);
    let (target, output) = run_in(&fs, &env, &["--save", "18"]);
    assert!(matches!(target, Ok(Target::Installed { .. })));
    let warning = "Warning: Unable to write version number (v18.20.4) to .nvmrc";
    assert_eq!(
        (output.stdout.as_str(), output.stderr.as_str()),
        ("", warning)
    );
}

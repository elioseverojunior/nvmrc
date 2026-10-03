use super::*;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem};

const N18: &str = "/n/versions/node/v18.9.0";

fn deactivate_with(vars: &[(&str, &str)], args: &[&str]) -> Output {
    let fs = FakeFileSystem::default();
    let mut env = FakeEnv::default().with_var("NVM_DIR", "/n");
    for (name, value) in vars {
        env = env.with_var(name, value);
    }
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    run(&Context::new(&fs, &env), &args).unwrap()
}

fn active_path() -> String {
    format!("{N18}/bin:/usr/bin")
}

const UNSET_BINS: &str = "unset NVM_BIN\nunset NVM_INC\n";
const HASH: &str = "hash -r 2>/dev/null || true\n";

#[test]
fn deactivating_an_active_version_removes_path_and_manpath_entries() {
    let manpath = format!("{N18}/share/man:");
    let output = deactivate_with(&[("PATH", &active_path()), ("MANPATH", &manpath)], &[]);
    assert_eq!(
        output.stdout,
        "/n/*/bin removed from ${PATH}\n/n/*/share/man removed from ${MANPATH}"
    );
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
    let expected = format!("export PATH='/usr/bin'\n{HASH}unset MANPATH\n{UNSET_BINS}");
    assert_eq!(output.script.render(), expected);
}

#[test]
fn nothing_active_reports_the_missing_path_entry() {
    let output = deactivate_with(&[("PATH", "/usr/bin")], &[]);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "Could not find /n/*/bin in ${PATH}");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.script.render(), UNSET_BINS);
}

#[test]
fn silent_prints_nothing_and_still_unsets() {
    let output = deactivate_with(&[("PATH", "/usr/bin"), ("MANPATH", "/x")], &["--silent"]);
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
    assert_eq!(output.script.render(), UNSET_BINS);
    let active = deactivate_with(&[("PATH", &active_path())], &["--silent"]);
    assert_eq!((active.stdout.as_str(), active.stderr.as_str()), ("", ""));
    assert_eq!(
        active.script.render(),
        format!("export PATH='/usr/bin'\n{HASH}{UNSET_BINS}")
    );
}

#[test]
fn an_unrelated_manpath_is_reported_and_left_alone() {
    let output = deactivate_with(&[("PATH", "/usr/bin"), ("MANPATH", "/x")], &[]);
    assert_eq!(output.stdout, "");
    assert_eq!(
        output.stderr,
        "Could not find /n/*/bin in ${PATH}\nCould not find /n/*/share/man in ${MANPATH}"
    );
    assert_eq!(output.script.render(), UNSET_BINS);
}

#[test]
fn a_manpath_that_keeps_entries_is_exported() {
    let manpath = format!("/x:{N18}/share/man:");
    let output = deactivate_with(&[("PATH", &active_path()), ("MANPATH", &manpath)], &[]);
    assert_eq!(output.stderr, "");
    let expected = format!("export PATH='/usr/bin'\n{HASH}export MANPATH='/x:'\n{UNSET_BINS}");
    assert_eq!(output.script.render(), expected);
}

#[test]
fn a_manpath_that_becomes_empty_is_unset() {
    let manpath = format!("{N18}/share/man");
    let output = deactivate_with(&[("PATH", "/usr/bin"), ("MANPATH", &manpath)], &[]);
    assert_eq!(output.stdout, "/n/*/share/man removed from ${MANPATH}");
    assert_eq!(
        output.script.render(),
        format!("unset MANPATH\n{UNSET_BINS}")
    );
}

#[test]
fn node_path_is_stripped_and_exported_even_when_empty() {
    let node_path = format!("/q:{N18}/lib/node_modules");
    let path = format!("{N18}/bin:/usr/bin:/n/versions/node/v20.0.0/bin:");
    let output = deactivate_with(&[("PATH", &path), ("NODE_PATH", &node_path)], &[]);
    assert_eq!(
        output.stdout,
        "/n/*/bin removed from ${PATH}\n/n/*/lib/node_modules removed from ${NODE_PATH}"
    );
    assert_eq!(output.stderr, "");
    let expected = format!("export PATH='/usr/bin:'\n{HASH}export NODE_PATH='/q'\n{UNSET_BINS}");
    assert_eq!(output.script.render(), expected);
    let only = format!("{N18}/lib/node_modules");
    let emptied = deactivate_with(&[("PATH", "/usr/bin"), ("NODE_PATH", &only)], &[]);
    assert!(emptied.script.render().contains("export NODE_PATH=''\n"));
}

#[test]
fn an_unchanged_node_path_prints_and_exports_nothing() {
    let output = deactivate_with(&[("PATH", "/usr/bin"), ("NODE_PATH", "/q")], &[]);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "Could not find /n/*/bin in ${PATH}");
    assert_eq!(output.script.render(), UNSET_BINS);
}

#[test]
fn other_arguments_are_ignored() {
    let output = deactivate_with(&[("PATH", "/usr/bin")], &["--bogus", "foo", "--"]);
    assert_eq!(output.stderr, "Could not find /n/*/bin in ${PATH}");
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn the_script_text_quotes_values_with_a_single_quote() {
    let path = format!("{N18}/bin:/it's");
    let output = deactivate_with(&[("PATH", &path)], &[]);
    assert!(
        output
            .script
            .render()
            .starts_with("export PATH='/it'\\''s'\n")
    );
}

#[test]
fn an_unresolvable_nvm_dir_fails_without_a_script() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default()
        .with_var("PATH", "/usr/bin")
        .with_var("MANPATH", "/a");
    let context = Context::new(&fs, &env);
    let error = run(&context, &[]).unwrap_err();
    assert!(matches!(error, CliError::NvmDirUnresolved));
    assert_eq!(error.exit_code(), NvmExitCode::Failure);
    let mut transcript = Transcript::default();
    assert!(deactivate(&context, false, &mut transcript).is_err());
    let output = transcript.finish(NvmExitCode::Failure);
    assert_eq!(output.stderr, "${NVM_DIR} not set!");
    assert!(output.script.is_empty());
}

#[test]
fn the_reusable_function_appends_to_the_callers_transcript() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin");
    let mut transcript = Transcript::default();
    transcript.out("before");
    let script = deactivate(&Context::new(&fs, &env), true, &mut transcript).unwrap();
    assert_eq!(script.render(), UNSET_BINS);
    assert_eq!(transcript.finish(NvmExitCode::Success).stdout, "before");
}

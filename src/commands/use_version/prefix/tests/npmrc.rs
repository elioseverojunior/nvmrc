//! The four npmrc files: their order, labels, sanitised paths, the project
//! directory search, and the `npm config` calls of `--delete-prefix`.

use super::*;
use crate::ports::Completed;

const BAD: &str = "color=true\nprefix = /x\n";
const BUILTIN: &str = "/nvm/versions/node/v18.20.4/lib/node_modules/npm/npmrc";
const GLOBAL: &str = "/nvm/versions/node/v18.20.4/etc/npmrc";
const NPM: &str = "/nvm/versions/node/v18.20.4/bin/npm";

fn has_setting(label: &str, shown: &str) -> String {
    format!(
        "Your {label} ({shown})\n\
has a `globalconfig` and/or a `prefix` setting, which are incompatible with nvm.\n\
Run `{COMMAND}` to unset it."
    )
}

fn refused_for(fs: &FakeFileSystem, label: &str, shown: &str) {
    let (result, output) = checked(fs, &environment());
    assert_eq!(result, Err(Refusal::KeepSwitch));
    assert_eq!(output.stderr, has_setting(label, shown));
    assert_eq!(output.stdout, "");
}

#[test]
fn a_builtin_npmrc_with_a_prefix_is_refused() {
    let fs = lab().with_file(BUILTIN, BAD);
    let shown = "${NVM_DIR}/versions/node/v18.20.4/lib/node_modules/npm/npmrc";
    refused_for(&fs, "builtin npmrc file", shown);
}

#[test]
fn a_global_npmrc_with_a_globalconfig_is_refused() {
    let fs = lab().with_file(GLOBAL, "globalconfig=/y\n");
    let shown = "${NVM_DIR}/versions/node/v18.20.4/etc/npmrc";
    refused_for(&fs, "global npmrc file", shown);
}

#[test]
fn a_user_npmrc_is_named_with_a_typographic_apostrophe() {
    let fs = lab().with_file("/h/.npmrc", BAD);
    refused_for(&fs, "user\u{2019}s .npmrc file", "${HOME}/.npmrc");
}

#[test]
fn the_project_is_the_nearest_ancestor_with_a_package_json() {
    let fs = lab()
        .with_file("/proj/package.json", "{}")
        .with_file("/proj/.npmrc", BAD)
        .with_file("/.npmrc", "");
    refused_for(&fs, "project npmrc file", "/proj/.npmrc");
}

#[test]
fn a_node_modules_directory_also_marks_the_project() {
    let fs = lab()
        .with_dir("/proj/sub/node_modules")
        .with_file("/proj/package.json", "{}")
        .with_file("/proj/sub/.npmrc", BAD);
    refused_for(&fs, "project npmrc file", "/proj/sub/.npmrc");
}

#[test]
fn a_package_json_directory_or_a_node_modules_file_does_not() {
    let fs = lab()
        .with_dir("/proj/sub/package.json")
        .with_file("/proj/sub/node_modules", "")
        .with_file("/proj/sub/.npmrc", BAD);
    assert_eq!(checked(&fs, &environment()).0, Ok(()));
}

#[test]
fn without_a_project_the_file_is_the_root_npmrc() {
    let fs = lab().with_file("/.npmrc", BAD);
    refused_for(&fs, "project npmrc file", "/.npmrc");
}

#[test]
fn files_without_a_setting_or_that_are_directories_pass() {
    let fs = lab()
        .with_file(BUILTIN, "; prefix=/x\nprefixes=/y\n")
        .with_dir(GLOBAL)
        .with_file("/h/.npmrc", " prefix=/x\n");
    assert_eq!(checked(&fs, &environment()), (Ok(()), Output::default()));
}

#[test]
fn the_first_bad_file_in_order_is_the_only_one_reported() {
    let fs = lab()
        .with_file("/.npmrc", BAD)
        .with_file("/h/.npmrc", BAD)
        .with_file(GLOBAL, BAD);
    let shown = "${NVM_DIR}/versions/node/v18.20.4/etc/npmrc";
    refused_for(&fs, "global npmrc file", shown);
}

fn npm_calls(process: &FakeProcess) -> Vec<String> {
    process
        .executed()
        .iter()
        .map(|run| {
            assert_eq!(run.program, std::path::Path::new(NPM));
            run.args.join(" ")
        })
        .collect()
}

#[test]
fn delete_prefix_runs_npm_config_for_every_bad_file_in_order_and_passes() {
    let fs = lab()
        .with_file(BUILTIN, BAD)
        .with_file(GLOBAL, BAD)
        .with_file("/h/.npmrc", BAD)
        .with_file("/.npmrc", BAD);
    let process = FakeProcess::default();
    let (result, _) = checked_with(&fs, &environment(), &process, true);
    assert_eq!(result, Ok(()));
    let userconfig =
        |file: &str, key: &str| format!("config --loglevel=warn delete {key} --userconfig={file}");
    assert_eq!(
        npm_calls(&process),
        [
            userconfig(BUILTIN, "prefix"),
            userconfig(BUILTIN, "globalconfig"),
            "config --global --loglevel=warn delete prefix".to_owned(),
            "config --global --loglevel=warn delete globalconfig".to_owned(),
            userconfig("/h/.npmrc", "prefix"),
            userconfig("/h/.npmrc", "globalconfig"),
            "config --loglevel=warn delete prefix".to_owned(),
            "config --loglevel=warn delete globalconfig".to_owned(),
        ]
    );
}

#[test]
fn delete_prefix_passes_npm_output_on_and_ignores_its_status() {
    let fs = lab().with_file("/h/.npmrc", BAD);
    let failed = Completed {
        success: false,
        code: Some(1),
        stdout: "said\n".to_owned(),
        stderr: "npm warn\n".to_owned(),
    };
    let args = "config --loglevel=warn delete prefix --userconfig=/h/.npmrc";
    let globalconfig = "config --loglevel=warn delete globalconfig --userconfig=/h/.npmrc";
    let process = FakeProcess::default()
        .with_execution(NPM, args, failed)
        .with_success(NPM, globalconfig, "");
    let (result, output) = checked_with(&fs, &environment(), &process, true);
    assert_eq!(result, Ok(()));
    assert_eq!(
        (output.stdout.as_str(), output.stderr.as_str()),
        ("said", "npm warn")
    );
    assert_eq!(npm_calls(&process).len(), 2);
}

#[test]
fn delete_prefix_runs_nothing_when_every_file_is_clean() {
    let process = FakeProcess::default();
    let (result, _) = checked_with(&lab(), &environment(), &process, true);
    assert_eq!(result, Ok(()));
    assert!(process.executed().is_empty());
}

//! End-to-end: `nvm install` with no version reads `.nvmrc`, and in the `nvm`
//! function (`NVMRC_SCRIPT_FD=3`) a real `sh` that evals the code written on
//! fd 3 ends up with the installed version first on `PATH`.

mod common;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Output;

use common::{Mirror, command_for, nvm_with, stderr, stdout};

const USAGE: &str = "No version provided and no .nvmrc file found\n\
Usage: nvm install [<version>]\n  \
Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
Run `nvm --help` for full help.\n";

/// What the `nvm` function does: the code on fd 3 is captured and evaluated,
/// stdout and stderr pass through. Then the shell says where `node` is.
const FUNCTION: &str = r#"{ code=$(NVMRC_SCRIPT_FD=3 "$NVMRC_BINARY" install "$@" 3>&1 1>&4 4>&-); } 4>&1
status=$?
eval "$code"
echo "status=$status"
echo "PATH=$PATH"
node
"#;

fn mirror() -> Option<String> {
    Some(Mirror::new().binary("v18.19.0", &[], None)?.serve())
}

/// A temporary `$NVM_DIR` and, inside it, a project holding `.nvmrc`.
fn project(nvmrc: Option<&str>) -> (tempfile::TempDir, String) {
    let home = tempfile::tempdir().unwrap();
    let project = home.path().join("proj");
    fs::create_dir(&project).unwrap();
    if let Some(text) = nvmrc {
        fs::write(project.join(".nvmrc"), text).unwrap();
    }
    let shown = project.display().to_string();
    (home, shown)
}

fn install_in(home: &Path, mirror: &str, project: &str, args: &[&str]) -> Output {
    let args: Vec<&str> = std::iter::once("install")
        .chain(args.iter().copied())
        .collect();
    nvm_with(home, mirror, &args, "/nonexistent", &[("PWD", project)])
}

fn in_function(home: &Path, mirror: &str, project: &str, args: &[&str]) -> Output {
    let mut command = command_for(OsStr::new("sh"), home, mirror, "/usr/bin:/bin");
    command
        .env("PWD", project)
        .current_dir(project)
        .env("NVMRC_BINARY", env!("CARGO_BIN_EXE_nvm"))
        .args(["-c", FUNCTION, "sh"])
        .args(args);
    command.output().expect("run sh")
}

#[test]
fn standalone_the_nvmrc_version_is_installed_and_not_activated() {
    let Some(mirror) = mirror() else { return };
    let (home, project) = project(Some("18\n"));
    let output = install_in(home.path(), &mirror, &project, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "Found '{project}/.nvmrc' with version <18>\n\
             Downloading and installing node v18.19.0...\n\
             Creating default alias: default -> 18 (-> v18.19.0 *)\n"
        )
    );
    assert!(
        home.path()
            .join("versions/node/v18.19.0/bin/node")
            .is_file()
    );
}

#[test]
fn without_a_version_or_an_nvmrc_it_is_the_usage_and_127() {
    let Some(mirror) = mirror() else { return };
    let (home, project) = project(None);
    let output = install_in(home.path(), &mirror, &project, &[]);
    assert_eq!(output.status.code(), Some(127));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), USAGE);
}

fn assert_activated(output: &Output, home: &Path) {
    let text = stdout(output);
    let bin = home.join("versions/node/v18.19.0/bin");
    assert!(text.contains("Now using node v18.19.0\n"), "{text}");
    assert!(text.contains("status=0\n"), "{text}\n{}", stderr(output));
    assert!(
        text.contains(&format!("PATH={}:/usr/bin:/bin\n", bin.display())),
        "{text}"
    );
    assert!(text.ends_with("v18.19.0\n"), "node is the new one: {text}");
}

#[cfg(unix)]
#[test]
fn in_the_nvm_function_install_18_puts_it_first_on_path() {
    let Some(mirror) = mirror() else { return };
    let (home, project) = project(None);
    let output = in_function(home.path(), &mirror, &project, &["18"]);
    assert_activated(&output, home.path());
    let again = in_function(home.path(), &mirror, &project, &["18"]);
    assert!(stderr(&again).contains("v18.19.0 is already installed.\n"));
    assert_activated(&again, home.path());
}

#[cfg(unix)]
#[test]
fn in_the_nvm_function_the_nvmrc_version_is_installed_and_put_first_on_path() {
    let Some(mirror) = mirror() else { return };
    let (home, project) = project(Some("18\n"));
    let output = in_function(home.path(), &mirror, &project, &[]);
    let text = stdout(&output);
    assert!(text.starts_with(&format!("Found '{project}/.nvmrc' with version <18>\n")));
    assert_activated(&output, home.path());
}

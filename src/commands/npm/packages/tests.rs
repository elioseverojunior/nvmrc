use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
use crate::ports::Completed;

const OLD_NPM: &str = "/n/versions/node/v18.19.0/bin/npm";
const NEW_NPM: &str = "/n/versions/node/v20.10.0/bin/npm";
const LISTING: &str = "/x/lib\n├── corepack@0.20.0\n├── npm@10.2.3\n├── yarn@1.22.19\n├── mylink@1.0.0 -> /abs/link/dir\n├── rel@1.0.0 -> ../../rellink\n└── @scope/tool@2.0.0\n";

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

fn fs() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable(OLD_NPM, "")
        .with_executable(NEW_NPM, "")
}

fn run_reinstall(source: &Source, process: &FakeProcess) -> (NvmExitCode, String, String) {
    let fs = fs();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(&fs, &env).with_process(process);
    let destination = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    let mut transcript = Transcript::default();
    let status = reinstall(&context, source, &destination, &mut transcript);
    let output = transcript.finish(status);
    (status, output.stdout, output.stderr)
}

fn calls(process: &FakeProcess) -> Vec<String> {
    process
        .executed()
        .iter()
        .map(|i| {
            format!(
                "{} {} [{}]",
                i.program.display(),
                i.args.join(" "),
                i.dir
                    .as_ref()
                    .map_or(String::new(), |d| d.display().to_string())
            )
        })
        .collect()
}

/// The expectations are what `nvm install --reinstall-packages-from` of the
/// real `nvm.sh` printed and ran, with a fake `npm`.
#[test]
fn the_packages_are_installed_in_one_command_and_the_links_are_linked() {
    let process = FakeProcess::default()
        .with_success(OLD_NPM, "list -g --depth=0", LISTING)
        .with_success(
            NEW_NPM,
            "install -g --quiet yarn@1.22.19 @scope/tool@2.0.0",
            "added\n",
        )
        .with_success(
            NEW_NPM,
            "root -g",
            "/n/versions/node/v20.10.0/lib/node_modules\n",
        )
        .with_success(NEW_NPM, "link", "linked\n");
    let (status, stdout, stderr) = run_reinstall(&Source::Version(version("v18.19.0")), &process);
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Reinstalling global packages from v18.19.0...\nadded\nLinking global packages from v18.19.0...\nlinked\nlinked"
    );
    assert_eq!(stderr, "");
    assert_eq!(
        calls(&process),
        [
            format!("{OLD_NPM} list -g --depth=0 []"),
            format!("{NEW_NPM} install -g --quiet yarn@1.22.19 @scope/tool@2.0.0 []"),
            format!("{NEW_NPM} root -g []"),
            format!("{NEW_NPM} link [/abs/link/dir]"),
            format!("{NEW_NPM} link [/n/versions/node/v20.10.0/lib/node_modules/../../../rellink]"),
        ]
    );
}

#[test]
fn nothing_to_install_or_link_says_so() {
    let process =
        FakeProcess::default().with_success(OLD_NPM, "list -g --depth=0", "/x/lib\n└── (empty)\n");
    let (status, stdout, _) = run_reinstall(&Source::Version(version("v18.19.0")), &process);
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Reinstalling global packages from v18.19.0...\nNo installed global packages found...\nLinking global packages from v18.19.0...\nNo linked global packages found..."
    );
}

#[test]
fn a_missing_version_prints_the_use_message_and_finds_nothing() {
    let (status, stdout, stderr) = run_reinstall(&Source::Missing, &FakeProcess::default());
    assert_eq!(status, NvmExitCode::Success);
    assert!(stdout.starts_with(
        "Reinstalling global packages from N/A...\nNo installed global packages found..."
    ));
    assert_eq!(
        stderr,
        "N/A: version \"N/A\" is not yet installed.\n\nYou need to run `nvm install N/A` to install and use it."
    );
}

#[test]
fn the_status_is_that_of_the_last_link() {
    let failed = Completed {
        success: false,
        stdout: String::new(),
        stderr: String::new(),
        ..Completed::default()
    };
    let process = FakeProcess::default()
        .with_success(
            OLD_NPM,
            "list -g --depth=0",
            "/x\n├── a@1.0.0 -> /one\n├── b@1.0.0 -> /two\n",
        )
        .with_success(NEW_NPM, "install -g --quiet a@1.0.0 b@1.0.0", "")
        .with_success(NEW_NPM, "root -g", "/r\n")
        .with_execution(NEW_NPM, "link", failed);
    let (status, _, _) = run_reinstall(&Source::Version(version("v18.19.0")), &process);
    assert_eq!(status, NvmExitCode::Failure);
}

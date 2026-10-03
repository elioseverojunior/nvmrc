//! The checks of `nvm_die_on_prefix` against fakes: the environment
//! variables here, the npmrc files in `npmrc`.

mod npmrc;

use super::{Check, Refusal, check, command};
use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::version::Version;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

const DIRECTORY: &str = "/nvm/versions/node/v18.20.4";
const NEW_PATH: &str = "/nvm/versions/node/v18.20.4/bin:/usr/bin";
const COMMAND: &str = "nvm use --delete-prefix v18.20.4";

fn version(text: &str) -> Version {
    text.parse().expect("a version")
}

fn lab() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable(&format!("{DIRECTORY}/bin/node"), "")
        .with_executable(&format!("{DIRECTORY}/bin/npm"), "")
        .with_dir("/proj/sub/deep")
}

fn environment() -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/nvm")
        .with_var("HOME", "/h")
        .with_var("PWD", "/proj/sub/deep")
        .with_var("PATH", "/usr/bin")
}

fn checked_with(
    fs: &FakeFileSystem,
    env: &FakeEnv,
    process: &FakeProcess,
    delete: bool,
) -> (Result<(), Refusal>, Output) {
    let context = Context::new(fs, env).with_process(process);
    let request = Check {
        nvm_dir: "/nvm",
        directory: DIRECTORY,
        path: NEW_PATH,
        command: COMMAND,
        delete,
    };
    let mut transcript = Transcript::default();
    let result = check(&context, &request, &mut transcript);
    (result, transcript.finish(NvmExitCode::Success))
}

fn checked(fs: &FakeFileSystem, env: &FakeEnv) -> (Result<(), Refusal>, Output) {
    checked_with(fs, env, &FakeProcess::default(), false)
}

fn incompatible(name: &str, value: &str) -> String {
    format!(
        "nvm is not compatible with the \"{name}\" environment variable: currently set to \
\"{value}\"\nRun `unset {name}` to unset it."
    )
}

#[test]
fn the_command_names_the_resolved_version_only_when_one_was_given() {
    let v18 = version("v18.20.4");
    assert_eq!(command(&v18, true, false), COMMAND);
    assert_eq!(command(&v18, false, false), "nvm use --delete-prefix");
    assert_eq!(
        command(&v18, true, true),
        "nvm use --delete-prefix v18.20.4 --silent"
    );
    assert_eq!(
        command(&v18, false, true),
        "nvm use --delete-prefix --silent"
    );
}

#[test]
fn the_command_names_io_js_with_its_prefix_like_version_does() {
    let iojs = version("iojs-v3.3.1");
    assert_eq!(
        command(&iojs, true, false),
        "nvm use --delete-prefix iojs-v3.3.1"
    );
}

#[test]
fn nothing_set_passes_in_silence() {
    let (result, output) = checked(&lab(), &environment());
    assert_eq!(result, Ok(()));
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
}

#[test]
fn a_prefix_variable_elsewhere_is_refused_with_a_deactivate() {
    let env = environment().with_var("PREFIX", "/zz");
    let (result, output) = checked(&lab(), &env);
    assert_eq!(result, Err(Refusal::Deactivate));
    assert_eq!(output.stderr, incompatible("PREFIX", "/zz"));
    assert_eq!(output.stdout, "");
}

#[test]
fn a_prefix_variable_naming_the_version_directory_passes() {
    let env = environment().with_var("PREFIX", DIRECTORY);
    assert_eq!(checked(&lab(), &env).0, Ok(()));
    let empty = environment().with_var("PREFIX", "");
    assert_eq!(checked(&lab(), &empty).0, Ok(()));
}

#[test]
fn npm_config_prefix_outside_nvm_dir_is_refused_by_its_own_name() {
    let upper = environment().with_var("NPM_CONFIG_PREFIX", "/foo");
    let (result, output) = checked(&lab(), &upper);
    assert_eq!(result, Err(Refusal::Deactivate));
    assert_eq!(output.stderr, incompatible("NPM_CONFIG_PREFIX", "/foo"));
    let lower = environment().with_var("npm_config_prefix", "/foo");
    let (result, output) = checked(&lab(), &lower);
    assert_eq!(result, Err(Refusal::Deactivate));
    assert_eq!(output.stderr, incompatible("npm_config_prefix", "/foo"));
    let mixed = environment().with_var("Npm_Config_Prefix", "/foo");
    assert_eq!(checked(&lab(), &mixed).0, Err(Refusal::Deactivate));
}

#[test]
fn npm_config_prefix_empty_or_inside_nvm_dir_passes() {
    let empty = environment().with_var("NPM_CONFIG_PREFIX", "");
    assert_eq!(checked(&lab(), &empty).0, Ok(()));
    let inside = environment().with_var("npm_config_prefix", DIRECTORY);
    assert_eq!(checked(&lab(), &inside).0, Ok(()));
    let nvm_dir = environment().with_var("NPM_CONFIG_PREFIX", "/nvm/");
    assert_eq!(checked(&lab(), &nvm_dir).0, Ok(()));
}

#[test]
fn npm_config_prefix_with_a_longer_name_is_not_it() {
    let env = environment().with_var("NPM_CONFIG_PREFIX_X", "/foo");
    assert_eq!(checked(&lab(), &env).0, Ok(()));
}

#[test]
fn prefix_is_checked_first_and_only_its_message_shows() {
    let env = environment()
        .with_var("PREFIX", "/zz")
        .with_var("NPM_CONFIG_PREFIX", "/foo");
    let fs = lab().with_file("/h/.npmrc", "prefix=/x\n");
    let (result, output) = checked(&fs, &env);
    assert_eq!(result, Err(Refusal::Deactivate));
    assert_eq!(output.stderr, incompatible("PREFIX", "/zz"));
}

#[test]
fn npm_config_prefix_comes_before_the_npmrc_files() {
    let env = environment().with_var("NPM_CONFIG_PREFIX", "/foo");
    let fs = lab().with_file("/h/.npmrc", "prefix=/x\n");
    let (result, output) = checked(&fs, &env);
    assert_eq!(result, Err(Refusal::Deactivate));
    assert_eq!(output.stderr, incompatible("NPM_CONFIG_PREFIX", "/foo"));
}

#[test]
fn delete_prefix_does_not_excuse_the_variables() {
    let env = environment().with_var("PREFIX", "/zz");
    let (result, _) = checked_with(&lab(), &env, &FakeProcess::default(), true);
    assert_eq!(result, Err(Refusal::Deactivate));
}

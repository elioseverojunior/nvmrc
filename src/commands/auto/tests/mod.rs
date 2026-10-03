//! `nvm __auto use` over every row of the oracle's table (digest 10), with
//! fakes: three installed versions with their `npm`, and a system `node` in
//! `/sys/bin` for the rows that put it on `PATH`.

mod install;

use super::run;
use crate::commands::Output;
use crate::context::Context;
use crate::domain::fixtures::index_text;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp, FakeProcess};

const N18: &str = "/n/versions/node/v18.20.4";
const N20: &str = "/n/versions/node/v20.11.1";
const N22: &str = "/n/versions/node/v22.3.0";
const BASE_PATH: &str = "/usr/bin:/bin";
const SYSTEM_PATH: &str = "/sys/bin:/usr/bin:/bin";
const VERSIONS: [(&str, &str); 3] = [(N18, "10.7.0"), (N20, "10.2.4"), (N22, "10.8.1")];
const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
const IOJS_INDEX: &str = "https://iojs.org/dist/index.tab";

/// The world one `__auto` runs in, built row by row.
struct Lab {
    fs: FakeFileSystem,
    env: FakeEnv,
}

impl Lab {
    fn new() -> Self {
        let fs = VERSIONS
            .iter()
            .fold(FakeFileSystem::default(), |fs, (directory, _)| {
                fs.with_executable(&format!("{directory}/bin/node"), "#!/bin/sh")
                    .with_executable(&format!("{directory}/bin/npm"), "#!/bin/sh")
            })
            .with_executable("/sys/bin/node", "#!/bin/sh")
            .with_dir("/proj");
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("HOME", "/h")
            .with_var("PWD", "/proj");
        Self { fs, env }.path(BASE_PATH)
    }

    fn alias(mut self, name: &str, target: &str) -> Self {
        self.fs = self
            .fs
            .with_file(&format!("/n/alias/{name}"), &format!("{target}\n"));
        self
    }

    fn file(mut self, path: &str, text: &str) -> Self {
        self.fs = self.fs.with_file(path, text);
        self
    }

    fn nvmrc(self, text: &str) -> Self {
        self.file("/proj/.nvmrc", text)
    }

    fn var(mut self, name: &str, value: &str) -> Self {
        self.env = self.env.with_var(name, value);
        self
    }

    fn path(self, path: &str) -> Self {
        self.var("PATH", path)
    }

    fn auto(&self, mode: &str) -> Result<Output, CliError> {
        let process = VERSIONS
            .iter()
            .fold(FakeProcess::default(), |process, (directory, npm)| {
                let npm_path = format!("{directory}/bin/npm");
                process.with_success(&npm_path, "--version", &format!("{npm}\n"))
            });
        let http = FakeHttp::default()
            .with_body(NODE_INDEX, &index_text(&[("v18.20.4", "Hydrogen")]))
            .with_body(IOJS_INDEX, &index_text(&[]));
        let context = Context::new(&self.fs, &self.env)
            .with_process(&process)
            .with_http(&http);
        run(&context, &[mode.to_owned()])
    }

    fn auto_use(&self) -> Output {
        self.auto("use").expect("__auto use runs")
    }
}

fn assert_unchanged(output: &Output) {
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
    assert!(output.script.is_empty(), "{}", output.script.render());
}

/// A silent switch to `directory`: no output, status 0, `PATH` as given.
fn assert_using(output: &Output, directory: &str, path: &str) {
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
    let code = output.script.render();
    assert!(code.contains(&format!("export PATH='{path}'\n")), "{code}");
    let bin = format!("export NVM_BIN='{directory}/bin'\n");
    assert!(code.contains(&bin), "{code}");
}

fn using(directory: &str) -> String {
    format!("{directory}/bin:{BASE_PATH}")
}

#[test]
fn the_default_alias_is_used() {
    let output = Lab::new().alias("default", "18").auto_use();
    assert_using(&output, N18, &using(N18));
}

#[test]
fn the_default_alias_wins_over_nvmrc() {
    let output = Lab::new().alias("default", "18").nvmrc("20\n").auto_use();
    assert_using(&output, N18, &using(N18));
}

#[test]
fn without_a_default_or_an_nvmrc_nothing_changes() {
    assert_unchanged(&Lab::new().auto_use());
}

#[test]
fn without_a_default_the_nvmrc_version_is_used() {
    let output = Lab::new().nvmrc("20\n").auto_use();
    assert_using(&output, N20, &using(N20));
}

#[test]
fn an_nvmrc_version_that_is_not_installed_is_status_3_and_silent() {
    let output = Lab::new().nvmrc("99\n").auto_use();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(output.stdout, "");
    assert!(output.script.is_empty());
}

#[test]
fn an_invalid_nvmrc_is_ignored_without_a_word() {
    assert_unchanged(&Lab::new().nvmrc("18\n20\n").auto_use());
}

#[test]
fn a_broken_default_does_nothing_and_never_reads_the_nvmrc() {
    assert_unchanged(&Lab::new().alias("default", "99").auto_use());
    let with_nvmrc = Lab::new().alias("default", "99").nvmrc("20\n");
    assert_unchanged(&with_nvmrc.auto_use());
    let garbage = Lab::new().alias("default", "garbage!").nvmrc("20\n");
    assert_unchanged(&garbage.auto_use());
}

#[test]
fn a_default_that_loops_does_nothing() {
    let looped = Lab::new()
        .alias("default", "loopa")
        .alias("loopa", "default");
    assert_unchanged(&looped.nvmrc("20\n").auto_use());
}

#[test]
fn a_default_through_aliases_lts_or_node_is_resolved() {
    for (target, directory) in [("myalias", N20), ("lts/*", N20), ("node", N22)] {
        let lab = Lab::new()
            .alias("myalias", "20")
            .alias("lts/*", "lts/iron")
            .alias("lts/iron", "v20.11.1")
            .alias("default", target);
        assert_using(&lab.auto_use(), directory, &using(directory));
    }
}

#[test]
fn a_system_default_with_a_system_node_changes_nothing() {
    let lab = Lab::new().alias("default", "system").path(SYSTEM_PATH);
    assert_unchanged(&lab.auto_use());
}

#[test]
fn the_default_is_put_before_a_system_node() {
    let lab = Lab::new().alias("default", "18").path(SYSTEM_PATH);
    let output = lab.auto_use();
    assert_using(&output, N18, &format!("{N18}/bin:{SYSTEM_PATH}"));
}

#[test]
fn a_system_node_without_a_default_or_an_nvmrc_stays() {
    assert_unchanged(&Lab::new().path(SYSTEM_PATH).auto_use());
}

#[test]
fn the_nvmrc_version_is_put_before_a_system_node() {
    let lab = Lab::new().nvmrc("20\n").path(SYSTEM_PATH);
    let output = lab.auto_use();
    assert_using(&output, N20, &format!("{N20}/bin:{SYSTEM_PATH}"));
}

#[test]
fn an_nvm_node_first_on_path_is_used_again_over_the_default() {
    let path = format!("{N20}/bin:{BASE_PATH}");
    let lab = Lab::new().alias("default", "18").path(&path);
    assert_using(&lab.auto_use(), N20, &path);
}

#[test]
fn a_prefix_in_the_user_npmrc_is_status_11_and_still_switches() {
    let lab = Lab::new()
        .alias("default", "18")
        .file("/h/.npmrc", "prefix=/x\n");
    let output = lab.auto_use();
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert_eq!(output.stdout, "");
    let hint = "Run `nvm use --delete-prefix v18.20.4 --silent` to unset it.";
    assert!(output.stderr.contains(hint), "{}", output.stderr);
    let code = output.script.render();
    assert!(
        code.contains(&format!("export NVM_BIN='{N18}/bin'")),
        "{code}"
    );
}

#[test]
fn an_exported_npm_config_prefix_is_status_11_and_switches_nothing() {
    let lab = Lab::new()
        .alias("default", "18")
        .var("NPM_CONFIG_PREFIX", "/x");
    let output = lab.auto_use();
    assert_eq!(output.status, NvmExitCode::IncompatiblePrefix);
    assert!(
        output.stderr.contains("NPM_CONFIG_PREFIX"),
        "{}",
        output.stderr
    );
    assert!(!output.script.render().contains("export NVM_BIN"));
}

#[test]
fn mode_none_does_nothing() {
    let lab = Lab::new().alias("default", "18");
    assert_unchanged(&lab.auto("none").expect("__auto none runs"));
}

#[test]
fn any_other_mode_is_an_error_with_status_1() {
    for mode in ["bogus", "", "USE"] {
        let error = Lab::new().auto(mode).expect_err("an invalid mode fails");
        assert_eq!(error.to_string(), "Invalid auto mode supplied.");
        assert_eq!(error.exit_code(), NvmExitCode::Failure);
    }
}

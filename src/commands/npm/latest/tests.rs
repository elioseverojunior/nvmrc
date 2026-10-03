use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

struct Setup {
    fs: FakeFileSystem,
    env: FakeEnv,
    process: FakeProcess,
}

impl Setup {
    fn new(npm_prints: &str) -> Self {
        Self {
            fs: FakeFileSystem::default().with_executable(NPM, ""),
            env: FakeEnv::default(),
            process: FakeProcess::default()
                .with_success(NPM, "--version", &format!("{npm_prints}\n"))
                .with_success(NPM, "install -g npm@10", "added 1 package\n"),
        }
    }

    fn run(&self, node: &Node<'_>, with_npm: bool) -> (NvmExitCode, String, String) {
        let context = Context::new(&self.fs, &self.env).with_process(&self.process);
        let npm = Npm::in_version(&context, std::path::Path::new("/n/versions/node/v20.10.0"));
        let mut transcript = Transcript::default();
        let status = install_latest(
            &context,
            npm.as_ref().filter(|_| with_npm),
            node,
            &mut transcript,
        );
        let output = transcript.finish(status);
        (status, output.stdout, output.stderr)
    }
}

/// The expectations are what `nvm install-latest-npm` of the real `nvm.sh`
/// printed.
#[test]
fn it_says_why_installs_and_reports_the_new_version() {
    let setup = Setup::new("10.2.3");
    let (status, stdout, stderr) = setup.run(&Node::Version("v20.10.0"), true);
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm...\n\
         * `npm` `v10.x` is the last version that works on `node` `< v20.17`, `v21`, or `v22.0` - `v22.8`\n\
         added 1 package\n\
         * npm upgraded to: v10.2.3"
    );
    assert_eq!(stderr, "");
    let ran: Vec<String> = setup
        .process
        .executed()
        .iter()
        .map(|i| i.args.join(" "))
        .collect();
    assert_eq!(ran, ["--version", "install -g npm@10", "--version"]);
}

#[test]
fn debug_prints_the_commands_instead_of_running_them() {
    let mut setup = Setup::new("3.10.0");
    setup.env = FakeEnv::default().with_var("NVM_DEBUG", "1");
    let (_, stdout, _) = setup.run(&Node::Version("v8.17.0"), true);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm...\n\
         Detected node version v8.17.0, npm version v3.10.0\n\
         * `npm` `v4.4.4` or later is required to install npm v6.14.18\n\
         npm install -g npm@4\n\
         * `npm` `v6.x` is the last version that works on `node` below `v10.0.0`\n\
         npm install -g npm@6\n\
         * npm upgraded to: v3.10.0"
    );
    assert_eq!(setup.process.executed().len(), 2);
}

#[test]
fn node_0_9_is_told_it_cannot_go_further() {
    let mut setup = Setup::new("1.4.0");
    setup.env = FakeEnv::default().with_var("NVM_DEBUG", "1");
    let (_, stdout, _) = setup.run(&Node::Version("v0.9.12"), true);
    assert!(stdout.contains("unable to upgrade further"), "{stdout}");
}

#[test]
fn an_iojs_version_is_read_without_its_prefix() {
    let mut setup = Setup::new("2.14.7");
    setup.env = FakeEnv::default().with_var("NVM_DEBUG", "1");
    let (_, stdout, _) = setup.run(&Node::Version("iojs-v3.3.1"), true);
    assert!(stdout.contains("Detected node version v3.3.1, npm version v2.14.7"));
}

#[test]
fn no_active_node_is_status_1() {
    let setup = Setup::new("10.2.3");
    let (status, stdout, stderr) = setup.run(&Node::None, true);
    assert_eq!(status, NvmExitCode::Failure);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm...\n\
         Detected node version none, npm version v10.2.3"
    );
    assert_eq!(stderr, "Unable to obtain node version.");
}

#[test]
fn no_npm_is_status_2() {
    let setup = Setup::new("10.2.3");
    let (status, stdout, stderr) = setup.run(&Node::Version("v20.10.0"), false);
    assert_eq!(status, NvmExitCode::MissingTarget);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm..."
    );
    assert_eq!(stderr, "Unable to obtain npm version.");
}

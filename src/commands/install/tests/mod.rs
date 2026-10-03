use std::path::Path;

use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper};
use crate::ports::FileSystem;

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
const IOJS_INDEX: &str = "https://iojs.org/dist/index.tab";
const SUMS: &str = "https://nodejs.org/dist/v20.10.0/SHASUMS256.txt";
const TARBALL_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0-linux-x64.tar.gz";
const TARBALL: &str = "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz";
const NODE: &str = "/n/versions/node/v20.10.0/bin/node";
const GOOD: &str = "aa11";

/// Everything a successful install of v20.10.0 touches, as one small world.
struct World {
    fs: FakeFileSystem,
    http: FakeHttp,
    digest: FakeDigest,
    sleeper: FakeSleeper,
    env: FakeEnv,
}

impl World {
    fn new() -> Self {
        let node = index_text(&[("v20.10.0", "Iron"), ("v18.19.0", "Hydrogen")]);
        let iojs = index_text(&[]);
        Self {
            fs: FakeFileSystem::default(),
            http: FakeHttp::default()
                .with_body(NODE_INDEX, &node)
                .with_body(IOJS_INDEX, &iojs)
                .with_body(SUMS, &format!("{GOOD}  node-v20.10.0-linux-x64.tar.gz\n"))
                .with_bytes(TARBALL_URL, b"tarball"),
            digest: FakeDigest::default().with_digest(TARBALL, GOOD),
            sleeper: FakeSleeper::default(),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
        }
    }

    fn run(&self, line: &str) -> Result<Output, CliError> {
        let archive = FakeArchive::new(&self.fs).with_archive(
            TARBALL,
            &[("node-v20.10.0-linux-x64/bin/node", "binary", true)],
        );
        let context = Context::new(&self.fs, &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_sleeper(&self.sleeper);
        let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        super::run(&context, &words)
    }

    fn installed(&self) -> bool {
        self.fs.file_info(Path::new(NODE)).is_ok()
    }

    fn text(&self, path: &str) -> Option<String> {
        self.fs.read_to_string(Path::new(path)).ok()
    }
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

mod failures;

#[test]
fn a_fresh_install_downloads_unpacks_and_makes_the_default_alias() {
    let world = World::new();
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert_eq!(
        lines(&output.stderr),
        [
            format!("Downloading {TARBALL_URL}..."),
            "Checksums matched!".to_owned()
        ]
    );
    assert!(world.installed());
    assert_eq!(world.text("/n/alias/default").as_deref(), Some("20\n"));
}

#[test]
fn an_installed_version_is_not_downloaded_again() {
    let world = World::new();
    world.run("20").unwrap();
    let requests = world.http.requests().len();
    let output = world.run("v20.10.0").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.stderr, "v20.10.0 is already installed.");
    assert_eq!(output.stdout, "");
    assert_eq!(
        world.http.requests().len(),
        requests + 2,
        "only the indexes"
    );
}

#[test]
fn an_existing_default_is_not_replaced_by_a_second_install() {
    let world = World {
        fs: FakeFileSystem::default().with_file("/n/alias/default", "18\n"),
        ..World::new()
    };
    let output = world.run("20").unwrap();
    assert!(!output.stdout.contains("Creating default alias"));
    assert_eq!(world.text("/n/alias/default").as_deref(), Some("18\n"));
}

#[test]
fn lts_installs_the_newest_lts_and_makes_default_point_at_lts_star() {
    let world = World::new();
    let output = world.run("--lts").unwrap();
    assert_eq!(
        lines(&output.stdout),
        [
            "Installing latest LTS version.",
            "Downloading and installing node v20.10.0...",
            "Creating default alias: default -> lts/* (-> v20.10.0 *)",
        ]
    );
}

#[test]
fn a_named_lts_is_announced_and_lowercased_in_the_default_alias() {
    let world = World::new();
    let output = world.run("--lts=Iron").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stdout,
        "Installing with latest version of LTS line: Iron"
    );
    assert_eq!(
        lines(&output.stderr),
        [
            "LTS names must be lowercase",
            "Version with LTS filter 'Iron' not found - try `nvm ls-remote --lts=Iron` to browse available versions.",
        ]
    );
}

#[test]
fn default_and_alias_make_the_alias_before_the_default_is_ensured() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/alias/default"), "18\n")
        .unwrap();
    let output = world.run("--alias=work 20").unwrap();
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "work -> 20 (-> v20.10.0 *)"
        ]
    );
    let again = world.run("--default 20").unwrap();
    assert_eq!(again.stdout, "default -> 20 (-> v20.10.0 *)");
    assert_eq!(world.text("/n/alias/default").as_deref(), Some("20\n"));
}

#[test]
fn the_lock_is_released_after_an_install() {
    let world = World::new();
    world.run("20").unwrap();
    assert!(
        world
            .fs
            .file_info(Path::new("/n/.cache/locks/v20.10.0"))
            .is_err()
    );
}

#[test]
fn a_broken_install_without_a_working_node_is_installed_again() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/versions/node/v20.10.0/bin/node"), "")
        .unwrap();
    let output = world.run("20").unwrap();
    assert!(
        output
            .stdout
            .starts_with("Downloading and installing node v20.10.0...")
    );
    assert!(world.installed());
}

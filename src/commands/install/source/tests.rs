use std::path::PathBuf;

use super::*;
use crate::domain::platform::Platform;
use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeProcess};
use crate::ports::{Completed, FileSystem};

const SUMS: &str = "https://nodejs.org/dist/v20.10.0/SHASUMS256.txt";
const TARBALL_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0.tar.xz";
const TARBALL: &str = "/n/.cache/src/node-v20.10.0/node-v20.10.0.tar.xz";
const TOP: &str = "/n/.cache/src/node-v20.10.0/files";
const PREFIX: &str = "--prefix=/n/versions/node/v20.10.0";

struct World {
    fs: FakeFileSystem,
    http: FakeHttp,
    digest: FakeDigest,
    env: FakeEnv,
    process: FakeProcess,
}

impl World {
    fn new() -> Self {
        Self {
            fs: FakeFileSystem::default(),
            http: FakeHttp::default()
                .with_body(SUMS, "aa11  node-v20.10.0.tar.xz\n")
                .with_bytes(TARBALL_URL, b"source"),
            digest: FakeDigest::default().with_digest(TARBALL, "aa11"),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
            process: FakeProcess::default()
                .with_success(&format!("{TOP}/configure"), PREFIX, "configured\n")
                .with_success("make", "-j 7", "built\n")
                .with_success("make", "-j 7 install", "installed\n"),
        }
    }

    fn build(
        &self,
        extra: &[&str],
        platform: Option<Platform>,
    ) -> (Result<(), BuildFailed>, String, String) {
        let archive = FakeArchive::new(&self.fs)
            .with_archive(TARBALL, &[("node-v20.10.0/configure", "#!/bin/sh", true)]);
        let context = Context::new(&self.fs, &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_process(&self.process)
            .with_platform(platform);
        let version: Version = "v20.10.0".parse().unwrap();
        let path = PathBuf::from("/n/versions/node/v20.10.0");
        let extra: Vec<String> = extra.iter().map(|word| (*word).to_owned()).collect();
        let job = Build {
            version: &version,
            version_path: &path,
            jobs: 7,
            extra: &extra,
            offline: false,
        };
        let mut transcript = Transcript::default();
        let result = build(&context, &job, &mut transcript);
        let output = transcript.finish(crate::error::NvmExitCode::Success);
        (result, output.stdout, output.stderr)
    }

    fn ran(&self) -> Vec<String> {
        self.process
            .executed()
            .iter()
            .map(|i| format!("{} {}", i.program.display(), i.args.join(" ")))
            .collect()
    }
}

fn linux() -> Option<Platform> {
    Platform::from_host("linux", "x86_64", false)
}

/// The expectations are what `nvm install -s` of the real `nvm.sh` printed
/// and ran, with a fake `./configure` and `make`.
#[test]
fn it_downloads_unpacks_configures_makes_and_installs() {
    let world = World::new();
    let (result, stdout, stderr) = world.build(&[], linux());
    assert_eq!(result, Ok(()));
    assert_eq!(
        stdout,
        format!("$>./configure {PREFIX} <\nconfigured\nbuilt\ninstalled")
    );
    assert_eq!(
        stderr,
        format!("Downloading {TARBALL_URL}...\nChecksums matched!")
    );
    assert_eq!(
        world.ran(),
        [
            format!("{TOP}/configure {PREFIX}"),
            "make -j 7".to_owned(),
            "make -j 7 install".to_owned()
        ]
    );
    let first = &world.process.executed()[0];
    assert_eq!(first.dir.as_deref(), Some(std::path::Path::new(TOP)));
}

#[test]
fn the_words_after_the_version_go_to_configure_and_are_announced_with_a_leading_space() {
    let world = World {
        process: FakeProcess::default()
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --with-intl=full-icu --ninja"),
                "",
            )
            .with_success("make", "-j 7", "")
            .with_success("make", "-j 7 install", ""),
        ..World::new()
    };
    let (result, stdout, _) = world.build(&["--with-intl=full-icu", "--ninja"], linux());
    assert_eq!(result, Ok(()));
    assert!(stdout.starts_with(
        "Additional options while compiling:  --with-intl=full-icu --ninja\n$>./configure --prefix=/n/versions/node/v20.10.0 --with-intl=full-icu --ninja<"
    ));
}

#[test]
fn a_32_bit_arm_gets_without_snapshot() {
    let arm = Platform::from_host("linux", "arm", false);
    let world = World {
        process: FakeProcess::default()
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --without-snapshot"),
                "",
            )
            .with_success("make", "-j 7", "")
            .with_success("make", "-j 7 install", ""),
        ..World::new()
    };
    let (_, stdout, _) = world.build(&[], arm.clone());
    assert!(stdout.starts_with("Additional options while compiling: --without-snapshot\n"));
    let world = World {
        process: FakeProcess::default().with_success(
            &format!("{TOP}/configure"),
            &format!("{PREFIX} --without-snapshot --a"),
            "",
        ),
        ..World::new()
    };
    let (_, stdout, _) = world.build(&["--a"], arm);
    assert!(stdout.starts_with("Additional options while compiling: --without-snapshot  --a\n"));
}

#[test]
fn macos_names_the_compilers_and_clang_is_announced() {
    let mac = Platform::from_host("macos", "aarch64", false);
    let fs = FakeFileSystem::default()
        .with_executable("/usr/bin/clang", "")
        .with_executable("/usr/bin/clang++", "");
    let process = FakeProcess::default()
        .with_output(
            "/usr/bin/clang",
            "Apple clang version 15.0.0 (clang-1500.1.0.2.5)\n",
        )
        .with_success(&format!("{TOP}/configure"), PREFIX, "")
        .with_success("make", "-j 7 CC=cc CXX=c++", "")
        .with_success("make", "-j 7 CC=cc CXX=c++ install", "");
    let world = World {
        fs,
        env: FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", "/usr/bin"),
        process,
        ..World::new()
    };
    let (result, stdout, _) = world.build(&[], mac);
    assert_eq!(result, Ok(()));
    assert!(stdout.starts_with("Clang v3.5+ detected! CC or CXX not specified, will use Clang as C/C++ compiler!\n$>./configure"));
    assert!(
        world
            .ran()
            .contains(&"make -j 7 CC=cc CXX=c++ install".to_owned())
    );
}

#[test]
fn a_failing_configure_is_reported_and_the_unpacked_tree_is_removed() {
    let failed = Completed {
        success: false,
        stdout: String::new(),
        stderr: "no compiler\n".to_owned(),
        ..Completed::default()
    };
    let world = World {
        process: FakeProcess::default().with_execution(&format!("{TOP}/configure"), PREFIX, failed),
        ..World::new()
    };
    let (result, stdout, stderr) = world.build(&[], linux());
    assert_eq!(result, Err(BuildFailed));
    assert_eq!(stdout, format!("$>./configure {PREFIX} <"));
    assert!(stderr.ends_with("no compiler\nnvm: install v20.10.0 failed!"));
    assert!(
        world
            .fs
            .file_info(std::path::Path::new("/n/.cache/src/node-v20.10.0/files"))
            .is_err()
    );
    assert_eq!(world.ran().len(), 1);
}

#[test]
fn a_failing_make_install_is_a_failure_too() {
    let failed = Completed::default();
    let world = World {
        process: FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "")
            .with_success("make", "-j 7", "")
            .with_execution("make", "-j 7 install", failed),
        ..World::new()
    };
    let (result, _, stderr) = world.build(&[], linux());
    assert_eq!(result, Err(BuildFailed));
    assert!(stderr.ends_with("nvm: install v20.10.0 failed!"));
}

#[test]
fn a_failed_download_fails_without_saying_the_install_failed() {
    let world = World {
        http: FakeHttp::default().with_status(TARBALL_URL, 404),
        ..World::new()
    };
    let (result, _, stderr) = world.build(&[], linux());
    assert_eq!(result, Err(BuildFailed));
    assert!(stderr.ends_with(&format!("download from {TARBALL_URL} failed")));
    assert!(world.ran().is_empty());
}

#[test]
fn old_node_gets_a_shell_for_make_and_the_bsds_use_gmake() {
    let freebsd = Platform::from_host("freebsd", "x86_64", false);
    let world = World {
        process: FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "")
            .with_success("gmake", "-j 7 CC=cc CXX=c++", "")
            .with_success("gmake", "-j 7 CC=cc CXX=c++ install", ""),
        ..World::new()
    };
    let (result, _, _) = world.build(&[], freebsd);
    assert_eq!(result, Ok(()));
}

#[test]
fn a_platform_without_source_builds_says_so() {
    let world = World::new();
    let (result, _, stderr) = world.build(&[], None);
    assert_eq!(result, Err(BuildFailed));
    assert_eq!(
        stderr,
        "Installing from source is not supported on this platform."
    );
    assert!(world.ran().is_empty());
}

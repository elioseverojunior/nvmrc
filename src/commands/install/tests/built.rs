use std::rc::Rc;

use super::*;
use crate::fakes::FakeCpu;

pub(super) const SRC_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0.tar.xz";
pub(super) const SRC_TARBALL: &str = "/n/.cache/src/node-v20.10.0/node-v20.10.0.tar.xz";
pub(super) const TOP: &str = "/n/.cache/src/node-v20.10.0/files";
pub(super) const PREFIX: &str = "--prefix=/n/versions/node/v20.10.0";

/// A world where the binary cannot be had and the source can.
pub(super) struct Built {
    pub(super) fs: Rc<FakeFileSystem>,
    pub(super) http: FakeHttp,
    pub(super) digest: FakeDigest,
    pub(super) env: FakeEnv,
    pub(super) cpu: FakeCpu,
}

impl Built {
    pub(super) fn new() -> Self {
        let node = index_text(&[("v20.10.0", "Iron")]);
        let sums =
            format!("{GOOD}  node-v20.10.0-linux-x64.tar.xz\n{GOOD}  node-v20.10.0.tar.xz\n");
        Self {
            fs: Rc::new(FakeFileSystem::default()),
            http: FakeHttp::default()
                .with_body(NODE_INDEX, &node)
                .with_body(IOJS_INDEX, &index_text(&[]))
                .with_body(SUMS, &sums)
                .with_status(TARBALL_URL, 404)
                .with_bytes(SRC_URL, b"source"),
            digest: FakeDigest::default().with_digest(SRC_TARBALL, GOOD),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
            cpu: FakeCpu::with_cores(8),
        }
    }

    pub(super) fn with_env(self, name: &str, value: &str) -> Self {
        let mut vars = vec![("NVM_DIR", "/n")];
        vars.push((name, value));
        let env = vars
            .iter()
            .fold(FakeEnv::default(), |env, (k, v)| env.with_var(k, v));
        Self { env, ..self }
    }

    /// What a successful build or install hook leaves: an executable `node`.
    pub(super) fn node_maker(&self) -> impl Fn() + 'static {
        let fs = Rc::clone(&self.fs);
        move || {
            fs.write_file(Path::new(NODE), "binary").unwrap();
            fs.set_executable(Path::new(NODE));
        }
    }

    pub(super) fn run(&self, line: &str) -> Output {
        let process = FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "configured\n")
            .with_success("make", "-j 7", "built\n")
            .with_success("make", "-j 7 install", "installed\n")
            .with_effect("make", "-j 7 install", self.node_maker())
            .with_success("make", "-j 4", "")
            .with_success("make", "-j 4 install", "")
            .with_effect("make", "-j 4 install", self.node_maker())
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --with-intl=full-icu"),
                "",
            );
        let archive = FakeArchive::new(&self.fs).with_archive(
            SRC_TARBALL,
            &[("node-v20.10.0/configure", "#!/bin/sh", true)],
        );
        let context = Context::new(self.fs.as_ref(), &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_process(&process)
            .with_cpu(&self.cpu);
        let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        super::super::run(&context, &words).unwrap()
    }
}

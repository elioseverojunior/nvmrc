//! The temporary tree a scenario runs in, and the shell command that runs it.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common::{IOJS_INDEX, NODE_INDEX, serve};

const LOAD: &str = "@load@";
const LOAD_NO_USE: &str = "@load-no-use@";
/// A port nothing listens on: every download is refused.
const REFUSED: &str = "http://127.0.0.1:9";

/// What a scenario needs before it runs.
#[derive(Debug, Clone, Copy)]
pub enum Fixture {
    /// `$NVM_DIR/versions/node/<v>/bin/node`, printing `<v>`.
    Node(&'static str),
    /// `$NVM_DIR/<v>/bin/node`: where nvm's tests put v0.x versions.
    Legacy(&'static str),
    /// `$NVM_DIR/versions/io.js/<v>/bin/{node,iojs}`, printing `<v>`.
    Iojs(&'static str),
    /// `$NVM_DIR/alias/<name>`, holding the target and a newline.
    Alias(&'static str, &'static str),
    /// A file under the root (`proj/.nvmrc`, `home/.npmrc`), as given.
    File(&'static str, &'static str),
    /// `<W>/sys/node`, printing the version.
    SystemNode(&'static str),
    /// Both mirrors are served locally.
    Mirror,
}

/// Which `nvm` runs a scenario.
pub enum Implementation<'a> {
    Rust,
    /// The `nvm.sh` at this path, sourced.
    Oracle(&'a Path),
}

pub struct World {
    _directory: tempfile::TempDir,
    root: PathBuf,
    /// The node and the io.js mirror.
    mirrors: (String, String),
}

impl World {
    pub fn new(fixtures: &[Fixture], implementation: &Implementation<'_>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        for name in ["bin", "home", "proj", "sys", "nvm/alias"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        let refused = (REFUSED.to_owned(), REFUSED.to_owned());
        let mut world = Self {
            _directory: directory,
            root,
            mirrors: refused,
        };
        world.link_programs(implementation);
        fixtures.iter().for_each(|fixture| world.add(*fixture));
        world
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn add(&mut self, fixture: Fixture) {
        match fixture {
            Fixture::Node(version) => {
                self.programs(
                    &format!("nvm/versions/node/{version}/bin"),
                    version,
                    &["node"],
                );
            }
            Fixture::Legacy(version) => {
                self.programs(&format!("nvm/{version}/bin"), version, &["node"])
            }
            Fixture::Iojs(version) => {
                let bin = format!("nvm/versions/io.js/{version}/bin");
                self.programs(&bin, version, &["node", "iojs"]);
            }
            Fixture::SystemNode(version) => self.programs("sys", version, &["node"]),
            Fixture::Alias(name, target) => {
                self.file(&format!("nvm/alias/{name}"), &format!("{target}\n"))
            }
            Fixture::File(relative, text) => self.file(relative, text),
            Fixture::Mirror => self.mirrors = (index(NODE_INDEX), index(IOJS_INDEX)),
        }
    }

    /// Executables `names` in `relative`, each printing `version`.
    fn programs(&self, relative: &str, version: &str, names: &[&str]) {
        let directory = self.root.join(relative);
        fs::create_dir_all(&directory).unwrap();
        for name in names {
            let path = directory.join(name);
            fs::write(&path, format!("#!/bin/sh\necho {version}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn file(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn link_programs(&self, implementation: &Implementation<'_>) {
        match implementation {
            Implementation::Rust => {
                let binaries = [
                    ("nvm", env!("CARGO_BIN_EXE_nvm")),
                    ("nvmrc", env!("CARGO_BIN_EXE_nvmrc")),
                    ("nvm-exec", env!("CARGO_BIN_EXE_nvm-exec")),
                ];
                for (name, binary) in binaries {
                    symlink(binary, self.root.join("bin").join(name)).unwrap();
                }
            }
            Implementation::Oracle(nvm_sh) => {
                let checkout = nvm_sh.parent().unwrap();
                for name in ["nvm.sh", "nvm-exec"] {
                    symlink(checkout.join(name), self.root.join("nvm").join(name)).unwrap();
                }
            }
        }
    }

    fn nvm_exec(&self, implementation: &Implementation<'_>) -> PathBuf {
        match implementation {
            Implementation::Rust => self.root.join("bin/nvm-exec"),
            Implementation::Oracle(_) => self.root.join("nvm/nvm-exec"),
        }
    }

    /// `program -c <script>` (`zsh -f -c`) in `<W>/proj` with the scenario's
    /// environment and nothing else.
    pub fn command(
        &self,
        program: &Path,
        shell: &str,
        script: &str,
        implementation: &Implementation<'_>,
    ) -> Command {
        let at = |relative: &str| self.root.join(relative);
        let mut command = Command::new(program);
        if shell == "zsh" {
            command.arg("-f");
        }
        command
            .arg("-c")
            .arg(full_script(shell, script, implementation))
            .env_clear()
            .env("HOME", at("home"))
            .env("TERM", "dumb")
            .env("PATH", format!("{}:/usr/bin:/bin", at("bin").display()))
            .env("PWD", at("proj"))
            .env("NVM_DIR", at("nvm"))
            .env("D", at("nvm"))
            .env("SYS", at("sys"))
            .env("NVM_EXEC", self.nvm_exec(implementation))
            .env("NVM_NODEJS_ORG_MIRROR", &self.mirrors.0)
            .env("NVM_IOJS_ORG_MIRROR", &self.mirrors.1)
            .current_dir(at("proj"));
        command
    }
}

/// A local mirror serving `text` as its `index.tab`.
fn index(text: &str) -> String {
    serve(BTreeMap::from([(
        "/index.tab".to_owned(),
        text.as_bytes().to_vec(),
    )]))
}

/// `script` with its load markers replaced; a script without one starts
/// with `@load-no-use@`.
fn full_script(shell: &str, script: &str, implementation: &Implementation<'_>) -> String {
    let script = if script.contains("@load") {
        script.to_owned()
    } else {
        format!("{LOAD_NO_USE}\n{script}")
    };
    let (load, load_no_use) = match implementation {
        Implementation::Rust => (
            format!("eval \"$(nvm init {shell})\""),
            format!("eval \"$(nvm init {shell} --no-use)\""),
        ),
        Implementation::Oracle(nvm_sh) => {
            let source = format!(". '{}'", nvm_sh.display());
            (source.clone(), format!("{source} --no-use"))
        }
    };
    script
        .replace(LOAD_NO_USE, &load_no_use)
        .replace(LOAD, &load)
}

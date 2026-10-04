//! Test-only: the fixture of the golden captures of the digest (section 4):
//! node v18.20.4, v20.11.1 (in use), v22.3.0, io.js v3.3.1, a system node
//! v16.0.0, aliases `default -> 18`, `myalias -> 20`, `lts/* -> lts/iron`,
//! `lts/hydrogen -> v18.20.4`, `lts/iron -> v20.11.1`, and a `tput` that
//! knows `xterm-256color`.

use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
pub const IN_USE: &str = "/n/versions/node/v20.11.1/bin";

pub fn fixture() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v18.20.4/bin/node", "")
        .with_file("/n/versions/node/v20.11.1/bin/node", "")
        .with_file("/n/versions/node/v22.3.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.3.1/bin/node", "")
        .with_file("/n/alias/default", "18\n")
        .with_file("/n/alias/myalias", "20\n")
        .with_file("/n/alias/lts/*", "lts/iron\n")
        .with_file("/n/alias/lts/hydrogen", "v18.20.4\n")
        .with_file("/n/alias/lts/iron", "v20.11.1\n")
        .with_file("/sys/node", "")
        .with_file(TPUT, "")
}

/// Where stdout goes, which `node` is in use and the extra variables.
pub struct Setup {
    pub in_use: &'static str,
    pub terminal: bool,
    pub variables: Vec<(&'static str, &'static str)>,
}

impl Setup {
    pub fn tty() -> Self {
        Self {
            in_use: IN_USE,
            terminal: true,
            variables: Vec::new(),
        }
    }

    pub fn pipe() -> Self {
        Self {
            terminal: false,
            ..Self::tty()
        }
    }

    pub fn in_use(self, in_use: &'static str) -> Self {
        Self { in_use, ..self }
    }

    pub fn var(mut self, key: &'static str, value: &'static str) -> Self {
        self.variables.push((key, value));
        self
    }

    pub fn run(
        &self,
        fs: &FakeFileSystem,
        command: impl FnOnce(&Context<'_>) -> Result<Output, CliError>,
    ) -> Result<Output, CliError> {
        let process = FakeProcess::default()
            .with_output("/sys/node", "v16.0.0\n")
            .with_run(TPUT, "-T xterm-256color colors", true, "256\n")
            .with_run(TPUT, "-T xterm-256color sitm", true, "");
        let path = format!("{}:/sys:/usr/bin", self.in_use);
        let mut env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", &path)
            .with_var("TERM", "xterm-256color");
        for (key, value) in &self.variables {
            env = env.with_var(key, value);
        }
        let terminal = if self.terminal {
            FakeTerminal::terminal()
        } else {
            FakeTerminal::pipe()
        };
        let context = Context::new(fs, &env)
            .with_process(&process)
            .with_terminal(&terminal);
        command(&context)
    }

    /// `nvm alias <args>` on the fixture.
    pub fn alias(&self, args: &[&str]) -> Output {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        self.run(&fixture(), |context| super::run(context, &args))
            .unwrap()
    }
}

/// The alias block of 4.1, byte for byte.
pub const GOLDEN: [&str; 9] = [
    "\x1b[0;32mmyalias\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32m20\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m)",
    "\x1b[0;34mdefault\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34m18\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv18.20.4\x1b[0m)",
    "\x1b[0;31munstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mN/A\x1b[0m \x1b[0;37m(default)\x1b[0m",
    "\x1b[0;34miojs\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34miojs-v3.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34miojs-v3.3.1\x1b[0m) \x1b[0;37m(default)\x1b[0m",
    "\x1b[0;34mnode\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34mstable\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv22.3.0\x1b[0m) \x1b[0;37m(default)\x1b[0m",
    "\x1b[0;34mstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34m22.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv22.3.0\x1b[0m) \x1b[0;37m(default)\x1b[0m",
    "\x1b[1;33mlts/*\x1b[0m \x1b[0;90m->\x1b[0m \x1b[1;33mlts/iron\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m)",
    "\x1b[1;33mlts/hydrogen\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34mv18.20.4\x1b[0m",
    "\x1b[1;33mlts/iron\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m",
];

/// The alias block of 4.2 (pipe, `--no-colors`), byte for byte.
pub const PLAIN: [&str; 9] = [
    "default -> 18 (-> v18.20.4 *)",
    "myalias -> 20 (-> v20.11.1 *)",
    "iojs -> iojs-v3.3 (-> iojs-v3.3.1 *) (default)",
    "node -> stable (-> v22.3.0 *) (default)",
    "stable -> 22.3 (-> v22.3.0 *) (default)",
    "unstable -> N/A (default)",
    "lts/* -> lts/iron (-> v20.11.1 *)",
    "lts/hydrogen -> v18.20.4 *",
    "lts/iron -> v20.11.1 *",
];

/// The alias block of 4.3 (`NVM_COLORS=rgbcm`), byte for byte.
pub const RGBCM: [&str; 9] = [
    "\x1b[0;31mdefault\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m18\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mv18.20.4\x1b[0m)",
    "\x1b[0;34mmyalias\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34m20\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv20.11.1\x1b[0m)",
    "\x1b[0;31miojs\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31miojs-v3.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31miojs-v3.3.1\x1b[0m) \x1b[0;35m(default)\x1b[0m",
    "\x1b[0;31mnode\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mstable\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mv22.3.0\x1b[0m) \x1b[0;35m(default)\x1b[0m",
    "\x1b[0;31mstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m22.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mv22.3.0\x1b[0m) \x1b[0;35m(default)\x1b[0m",
    "\x1b[0;36munstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;36mN/A\x1b[0m \x1b[0;35m(default)\x1b[0m",
    "\x1b[1;32mlts/*\x1b[0m \x1b[0;90m->\x1b[0m \x1b[1;32mlts/iron\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv20.11.1\x1b[0m)",
    "\x1b[1;32mlts/hydrogen\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mv18.20.4\x1b[0m",
    "\x1b[1;32mlts/iron\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34mv20.11.1\x1b[0m",
];

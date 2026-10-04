//! What the tests of `nvm migrate` share.

use std::path::Path;

use super::run;
use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;
use crate::fakes::{FakeClock, FakeEnv, FakeFileSystem, FakeProcess, FakePrompt};
use crate::ports::FileSystem;

pub use crate::commands::conflict::fixtures::{HOME, home_env, link};

/// The fake clock's time: 2026-10-04 10:34:00 UTC.
pub const NOW: i64 = 1_791_110_040;
/// The suffix of a backup taken at [`NOW`].
pub const STAMP: &str = ".nvmrc-backup-20261004T103400Z";
/// The question asked before writing.
pub const QUESTION: &str = "Apply these changes? [y/N] ";

/// The lines install.sh appends to `.bashrc`.
pub const INSTALL_SH: &str = "export NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

/// [`INSTALL_SH`] after `migrate` for bash.
pub const INSTALL_SH_MIGRATED: &str = "export NVM_DIR=\"$HOME/.nvm\"\n\
# >>> nvmrc init >>>\n\
eval \"$(nvmrc init bash)\"\n\
# <<< nvmrc init <<<\n\
# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
# [nvmrc-migrated] [ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

/// Every syntax checker present and happy.
pub fn checkers() -> FakeProcess {
    FakeProcess::default()
        .with_output("bash", "")
        .with_output("zsh", "")
        .with_output("sh", "")
        .with_output("ksh", "")
        .with_output("fish", "")
}

/// The parts of the context a test varies.
pub struct Setup<'a> {
    pub fs: &'a FakeFileSystem,
    pub env: &'a FakeEnv,
    pub process: &'a FakeProcess,
    pub prompt: &'a FakePrompt,
}

impl Setup<'_> {
    /// `nvm migrate <args>` at [`NOW`].
    pub fn migrate(&self, args: &[&str]) -> Result<Output, CliError> {
        let clock = FakeClock::at(NOW);
        let context = Context::new(self.fs, self.env)
            .with_clock(&clock)
            .with_process(self.process)
            .with_prompt(self.prompt);
        let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
        run(&context, &args)
    }

    pub fn output(&self, args: &[&str]) -> Output {
        self.migrate(args).unwrap()
    }
}

/// `nvm migrate <args>` with `fs`, `$HOME` set, every checker and nobody at
/// the terminal.
pub fn migrate(fs: &FakeFileSystem, args: &[&str]) -> Output {
    let process = checkers();
    let prompt = FakePrompt::unavailable();
    let setup = Setup {
        fs,
        env: &home_env(),
        process: &process,
        prompt: &prompt,
    };
    setup.output(args)
}

pub fn read(fs: &FakeFileSystem, path: &str) -> String {
    fs.read_to_string(Path::new(path)).unwrap()
}

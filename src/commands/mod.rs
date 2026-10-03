pub mod alias;
pub mod aliases;
pub mod cache;
pub mod current;
pub mod deactivate;
pub mod exec;
pub mod install;
pub mod install_latest_npm;
pub mod ls;
pub mod ls_remote;
pub mod npm;
pub mod nvm_exec;
pub mod nvmrc_file;
pub mod rc_version;
pub mod reinstall_packages;
pub mod remote_index;
pub mod resolve;
pub mod run;
pub mod sanitize;
pub mod transcript;
pub mod unalias;
pub mod uninstall;
pub mod use_version;
pub mod version;
pub mod version_remote;
pub mod which;

use crate::error::NvmExitCode;
use crate::ports::Invocation;
use crate::shell::Script;

/// What a command wants printed, and the exit status to finish with. Text
/// carries no trailing newline: the CLI adds one to each non-empty stream.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub status: NvmExitCode,
    /// Shell code the generated `nvm` function must `eval`; empty for a
    /// command that changes nothing in the calling shell.
    pub script: Script,
    /// A program to run once the streams are printed (`nvm exec`, `nvm
    /// run`): its exit status replaces [`Self::status`].
    pub spawn: Option<Invocation>,
}

impl Output {
    #[must_use]
    pub fn stdout(text: impl Into<String>) -> Self {
        Self {
            stdout: text.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_stderr(mut self, text: impl Into<String>) -> Self {
        self.stderr = text.into();
        self
    }

    /// Shell code for the calling shell to evaluate.
    #[must_use]
    pub fn with_script(mut self, script: Script) -> Self {
        self.script = script;
        self
    }

    /// A program the CLI runs after printing the streams, whose status it
    /// finishes with.
    #[must_use]
    pub fn with_spawn(mut self, invocation: Invocation) -> Self {
        self.spawn = Some(invocation);
        self
    }

    /// For a result that is printed and still not a success, like `nvm ls`
    /// finding nothing.
    #[must_use]
    pub fn with_status(mut self, status: NvmExitCode) -> Self {
        self.status = status;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Invocation;

    #[test]
    fn the_default_status_is_success_and_can_be_changed() {
        assert_eq!(Output::stdout("x").status, NvmExitCode::Success);
        let output = Output::stdout("x").with_status(NvmExitCode::InvalidVersion);
        assert_eq!(output.status, NvmExitCode::InvalidVersion);
    }

    #[test]
    fn the_default_script_is_empty_and_can_be_set() {
        assert!(Output::default().script.is_empty());
        let script = Script::new().unset("NVM_BIN").unwrap();
        let output = Output::stdout("x").with_script(script);
        assert_eq!(output.script.render(), "unset NVM_BIN\n");
    }

    #[test]
    fn the_default_spawn_is_none_and_can_be_set() {
        assert_eq!(Output::default().spawn, None);
        let invocation = Invocation::new("node").args(&["a"]);
        let output = Output::stdout("Running").with_spawn(invocation.clone());
        assert_eq!(output.spawn, Some(invocation));
        assert_eq!(output.stdout, "Running");
    }

    #[test]
    fn stdout_output_has_an_empty_stderr() {
        let output = Output::stdout("v20.1.0");
        assert_eq!(output.stdout, "v20.1.0");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn with_stderr_keeps_stdout() {
        let output = Output::stdout("out").with_stderr("warning");
        assert_eq!(
            (output.stdout.as_str(), output.stderr.as_str()),
            ("out", "warning")
        );
    }
}

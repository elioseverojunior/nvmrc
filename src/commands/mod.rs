pub mod alias;
pub mod aliases;
pub mod current;
pub mod ls;
pub mod ls_remote;
pub mod remote_index;
pub mod resolve;
pub mod unalias;
pub mod version;
pub mod which;

use crate::error::NvmExitCode;

/// What a command wants printed, and the exit status to finish with. Text
/// carries no trailing newline: the CLI adds one to each non-empty stream.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub status: NvmExitCode,
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

    #[test]
    fn the_default_status_is_success_and_can_be_changed() {
        assert_eq!(Output::stdout("x").status, NvmExitCode::Success);
        let output = Output::stdout("x").with_status(NvmExitCode::InvalidVersion);
        assert_eq!(output.status, NvmExitCode::InvalidVersion);
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

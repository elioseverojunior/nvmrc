pub mod current;
pub mod resolve;
pub mod version;
pub mod which;

/// What a command wants printed. Text carries no trailing newline: the CLI
/// adds one to each non-empty stream.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    #[must_use]
    pub fn stdout(text: impl Into<String>) -> Self {
        Self {
            stdout: text.into(),
            stderr: String::new(),
        }
    }

    #[must_use]
    pub fn with_stderr(mut self, text: impl Into<String>) -> Self {
        self.stderr = text.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

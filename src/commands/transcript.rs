//! What a command prints while it works, kept apart so a failure can still
//! show everything that came before it.

use crate::commands::Output;
use crate::error::NvmExitCode;
use crate::shell::Script;

#[derive(Debug, Default)]
pub struct Transcript {
    stdout: Vec<String>,
    stderr: Vec<String>,
    script: Script,
}

impl Transcript {
    pub fn out(&mut self, line: impl Into<String>) {
        self.stdout.push(line.into());
    }

    pub fn err(&mut self, line: impl Into<String>) {
        self.stderr.push(line.into());
    }

    /// Adds the stdout, stderr and shell code of an [`Output`] a command
    /// returned.
    pub fn absorb(&mut self, output: Output) {
        self.script = std::mem::take(&mut self.script).append(output.script);
        self.stdout
            .extend(Some(output.stdout).filter(|text| !text.is_empty()));
        self.stderr
            .extend(Some(output.stderr).filter(|text| !text.is_empty()));
    }

    #[must_use]
    pub fn finish(self, status: NvmExitCode) -> Output {
        self.finish_with(status, Script::new())
    }

    /// Like [`Self::finish`], with `script` after the shell code absorbed so
    /// far.
    #[must_use]
    pub fn finish_with(self, status: NvmExitCode, script: Script) -> Output {
        Output::stdout(self.stdout.join("\n"))
            .with_stderr(self.stderr.join("\n"))
            .with_status(status)
            .with_script(self.script.append(script))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_keeps_the_lines_of_each_stream_in_order_and_ends_with_a_status() {
        let mut transcript = Transcript::default();
        transcript.out("one");
        transcript.err("warning");
        transcript.out("two");
        let output = transcript.finish(NvmExitCode::MissingTarget);
        assert_eq!(output.stdout, "one\ntwo");
        assert_eq!(output.stderr, "warning");
        assert_eq!(output.status, NvmExitCode::MissingTarget);
    }

    #[test]
    fn finish_with_carries_the_script() {
        let script = Script::new().hash_reset();
        let output = Transcript::default().finish_with(NvmExitCode::Success, script);
        assert_eq!(output.script.render(), "hash -r 2>/dev/null || true\n");
    }

    #[test]
    fn absorbed_shell_code_comes_before_the_code_given_at_the_end() {
        let mut transcript = Transcript::default();
        transcript.absorb(Output::default().with_script(Script::new().hash_reset()));
        let last = Script::new().unset("NVM_BIN").unwrap();
        let output = transcript.finish_with(NvmExitCode::Success, last);
        assert_eq!(
            output.script.render(),
            "hash -r 2>/dev/null || true\nunset NVM_BIN\n"
        );
    }

    #[test]
    fn absorbing_an_output_adds_its_non_empty_streams() {
        let mut transcript = Transcript::default();
        transcript.absorb(Output::stdout("a -> b").with_stderr("careful"));
        transcript.absorb(Output::default());
        let output = transcript.finish(NvmExitCode::Success);
        assert_eq!(
            (output.stdout.as_str(), output.stderr.as_str()),
            ("a -> b", "careful")
        );
    }
}

use std::io::{self, BufRead, IsTerminal, Write};

use crate::ports::Prompt;

/// The real [`Prompt`]: the question goes to standard error and one line of
/// standard input is the answer, when standard input is a terminal.
pub struct StdPrompt;

impl Prompt for StdPrompt {
    fn confirm(&self, question: &str) -> io::Result<bool> {
        let stdin = io::stdin();
        let interactive = stdin.is_terminal();
        ask(question, interactive, &mut stdin.lock(), &mut io::stderr())
    }
}

/// Asks `question` on `output` and reads the answer from `input`, only when
/// `interactive`; otherwise reads nothing.
fn ask(
    question: &str,
    interactive: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> io::Result<bool> {
    if !interactive {
        return Err(io::Error::from(io::ErrorKind::Unsupported));
    }
    output.write_all(question.as_bytes())?;
    output.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    Ok(is_yes(&answer))
}

/// `y` or `yes` in any case, surrounding blanks ignored.
fn is_yes(answer: &str) -> bool {
    let answer = answer.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use super::*;

    fn answer(line: &str) -> (bool, String) {
        let mut output = Vec::new();
        let confirmed = ask("Apply? ", true, &mut Cursor::new(line), &mut output).unwrap();
        (confirmed, String::from_utf8(output).unwrap())
    }

    #[test]
    fn y_and_yes_in_any_case_confirm_and_the_question_is_shown() {
        for line in ["y\n", "Y\n", "yes\n", "YeS\n", "  yes  \n", "y"] {
            assert_eq!(answer(line), (true, "Apply? ".to_owned()), "{line:?}");
        }
    }

    #[test]
    fn anything_else_declines() {
        for line in ["n\n", "no\n", "\n", "", "yep\n", "y es\n"] {
            assert!(!answer(line).0, "{line:?}");
        }
    }

    struct Untouchable;

    impl Read for Untouchable {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            panic!("standard input must not be read")
        }
    }

    impl BufRead for Untouchable {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            panic!("standard input must not be read")
        }

        fn consume(&mut self, _amount: usize) {}
    }

    #[test]
    fn without_a_terminal_it_is_unsupported_and_neither_reads_nor_asks() {
        let mut output = Vec::new();
        let error = ask("Apply? ", false, &mut Untouchable, &mut output).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(output.is_empty());
    }

    #[test]
    fn the_real_prompt_is_unsupported_when_stdin_is_not_a_terminal() {
        // Never block the suite: only checked when `cargo test` runs with
        // standard input redirected (CI, pipes), where nothing can answer.
        if io::stdin().is_terminal() {
            return;
        }
        let error = StdPrompt.confirm("Apply? ").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}

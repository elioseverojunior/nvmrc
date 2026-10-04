use std::io;

pub trait Terminal {
    /// Whether standard output is a terminal (`[ -t 1 ]`).
    fn stdout_is_terminal(&self) -> bool;
}

/// Asks the person at the terminal.
pub trait Prompt {
    /// Shows `question` and waits for a yes or no; only `y` or `yes` (any
    /// case) count as yes.
    ///
    /// # Errors
    /// `Unsupported` when nobody can answer (standard input is not a
    /// terminal); otherwise propagates the underlying I/O error.
    fn confirm(&self, question: &str) -> io::Result<bool>;
}

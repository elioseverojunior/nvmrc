use std::io::IsTerminal;

use crate::ports::Terminal;

/// The real [`Terminal`]: asks the operating system about standard output.
pub struct StdTerminal;

impl Terminal for StdTerminal {
    fn stdout_is_terminal(&self) -> bool {
        std::io::stdout().is_terminal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_without_panicking() {
        // Under `cargo test` stdout is captured, so the answer depends on the
        // harness; only the call itself is checked.
        let _answer = StdTerminal.stdout_is_terminal();
    }
}

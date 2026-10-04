use crate::ports::Terminal;

/// Standard output as a terminal or as a pipe.
pub struct FakeTerminal(bool);

impl FakeTerminal {
    #[must_use]
    pub fn terminal() -> Self {
        Self(true)
    }

    #[must_use]
    pub fn pipe() -> Self {
        Self(false)
    }
}

impl Terminal for FakeTerminal {
    fn stdout_is_terminal(&self) -> bool {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_what_it_was_made_as() {
        assert!(FakeTerminal::terminal().stdout_is_terminal());
        assert!(!FakeTerminal::pipe().stdout_is_terminal());
    }
}

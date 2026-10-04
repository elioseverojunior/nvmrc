use crate::ports::Terminal;

/// The `Terminal` of a `Context` that was not given one: output is a pipe.
pub struct NoTerminal;

impl Terminal for NoTerminal {
    fn stdout_is_terminal(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_never_a_terminal() {
        assert!(!NoTerminal.stdout_is_terminal());
    }
}

use std::io;

use crate::ports::Prompt;

/// The `Prompt` of a `Context` that was not given one: nobody can answer.
pub struct NoPrompt;

impl Prompt for NoPrompt {
    fn confirm(&self, _question: &str) -> io::Result<bool> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_never_ask() {
        let error = NoPrompt.confirm("Apply?").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}

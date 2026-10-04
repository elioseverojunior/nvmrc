use std::cell::RefCell;
use std::io;

use crate::ports::Prompt;

/// A person who always gives the same answer, or nobody at all.
pub struct FakePrompt {
    answer: Option<bool>,
    asked: RefCell<Vec<String>>,
}

impl FakePrompt {
    #[must_use]
    pub fn answering(answer: bool) -> Self {
        Self {
            answer: Some(answer),
            asked: RefCell::default(),
        }
    }

    /// Standard input is not a terminal: every question is `Unsupported`.
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            answer: None,
            asked: RefCell::default(),
        }
    }

    /// The questions asked so far, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }
}

impl Prompt for FakePrompt {
    fn confirm(&self, question: &str) -> io::Result<bool> {
        self.asked.borrow_mut().push(question.to_owned());
        self.answer
            .ok_or_else(|| io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_as_told_and_remembers_the_questions() {
        let yes = FakePrompt::answering(true);
        assert!(yes.confirm("Apply?").unwrap());
        assert!(!FakePrompt::answering(false).confirm("Apply?").unwrap());
        assert_eq!(yes.asked(), ["Apply?"]);
    }

    #[test]
    fn unavailable_is_unsupported_and_still_records_the_question() {
        let nobody = FakePrompt::unavailable();
        let error = nobody.confirm("Apply?").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(nobody.asked(), ["Apply?"]);
    }
}

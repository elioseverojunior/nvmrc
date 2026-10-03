use std::cell::RefCell;
use std::io;

use crate::ports::ScriptChannel;

/// Keeps the shell code it is sent, or fails every send.
#[derive(Default)]
pub struct FakeScriptChannel {
    sent: RefCell<String>,
    broken: bool,
}

impl FakeScriptChannel {
    /// A channel whose every send fails, as a closed descriptor would.
    #[must_use]
    pub fn broken() -> Self {
        Self {
            broken: true,
            ..Self::default()
        }
    }

    /// Everything sent so far.
    #[must_use]
    pub fn sent(&self) -> String {
        self.sent.borrow().clone()
    }
}

impl ScriptChannel for FakeScriptChannel {
    fn send(&self, code: &str) -> io::Result<()> {
        if self.broken {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        self.sent.borrow_mut().push_str(code);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_what_it_is_sent_unless_broken() {
        let channel = FakeScriptChannel::default();
        channel.send("a\n").unwrap();
        channel.send("b\n").unwrap();
        assert_eq!(channel.sent(), "a\nb\n");
        assert!(FakeScriptChannel::broken().send("a").is_err());
    }
}

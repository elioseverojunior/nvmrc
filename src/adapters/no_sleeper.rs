use std::time::Duration;

use crate::ports::Sleeper;

/// The `Sleeper` of a `Context` that was not given one: it never waits.
pub struct NoSleeper;

impl Sleeper for NoSleeper {
    fn sleep(&self, _duration: Duration) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_at_once() {
        NoSleeper.sleep(Duration::from_secs(3600));
    }
}

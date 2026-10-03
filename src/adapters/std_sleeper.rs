use std::time::Duration;

use crate::ports::Sleeper;

/// The real [`Sleeper`]: blocks the current thread for the whole duration.
pub struct StdSleeper;

impl Sleeper for StdSleeper {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

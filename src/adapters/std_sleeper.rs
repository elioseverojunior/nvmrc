use std::time::Duration;

use crate::ports::Sleeper;

pub struct StdSleeper;

impl Sleeper for StdSleeper {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

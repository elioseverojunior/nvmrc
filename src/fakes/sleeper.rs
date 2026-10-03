use std::cell::RefCell;
use std::time::Duration;

use crate::ports::Sleeper;

/// Records how long it was asked to sleep, and returns at once.
#[derive(Default)]
pub struct FakeSleeper {
    slept: RefCell<Vec<Duration>>,
}

impl FakeSleeper {
    #[must_use]
    pub fn slept(&self) -> Vec<Duration> {
        self.slept.borrow().clone()
    }
}

impl Sleeper for FakeSleeper {
    fn sleep(&self, duration: Duration) {
        self.slept.borrow_mut().push(duration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_instead_of_sleeping() {
        let sleeper = FakeSleeper::default();
        sleeper.sleep(Duration::from_millis(250));
        sleeper.sleep(Duration::from_millis(500));
        assert_eq!(
            sleeper.slept(),
            [Duration::from_millis(250), Duration::from_millis(500)]
        );
    }
}

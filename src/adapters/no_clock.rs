use crate::ports::Clock;

/// The `Clock` of a `Context` that was not given one: always the Unix epoch.
pub struct NoClock;

impl Clock for NoClock {
    fn unix_seconds(&self) -> i64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_always_the_epoch() {
        assert_eq!(NoClock.unix_seconds(), 0);
    }
}

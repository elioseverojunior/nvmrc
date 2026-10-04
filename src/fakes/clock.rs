use crate::ports::Clock;

/// A clock stopped at one moment.
pub struct FakeClock(i64);

impl FakeClock {
    #[must_use]
    pub fn at(unix_seconds: i64) -> Self {
        Self(unix_seconds)
    }
}

impl Clock for FakeClock {
    fn unix_seconds(&self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_the_time_it_was_set_to() {
        assert_eq!(FakeClock::at(1_791_110_040).unix_seconds(), 1_791_110_040);
    }
}

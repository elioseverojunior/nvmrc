use std::time::Duration;

pub trait Cpu {
    /// How many processors this machine has for a build to use, when known.
    fn cores(&self) -> Option<usize>;
}

pub trait Sleeper {
    /// Waits for `duration` (between retries).
    fn sleep(&self, duration: Duration);
}

pub trait Clock {
    /// The current time, in seconds since the Unix epoch (UTC); negative
    /// before 1970.
    fn unix_seconds(&self) -> i64;
}

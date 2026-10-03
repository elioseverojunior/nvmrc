use std::cell::Cell;

use super::*;
use crate::fakes::{FakeFileSystem, FakeSleeper};

const ROOT: &str = "/n/.cache/locks";

fn request(stale_minutes: u64, timeout_seconds: u64) -> LockRequest<'static> {
    LockRequest {
        root: Path::new(ROOT),
        version: "v20.10.0",
        timeout_seconds,
        stale_minutes,
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(10_000),
    }
}

fn exists(fs: &FakeFileSystem, name: &str) -> bool {
    fs.file_info(&Path::new(ROOT).join(name)).is_ok()
}

/// Lets go of the lock once it has been asked to sleep `after` times.
struct ReleasesAfter<'a> {
    fs: &'a FakeFileSystem,
    after: u32,
    slept: Cell<u32>,
}

impl Sleeper for ReleasesAfter<'_> {
    fn sleep(&self, _duration: Duration) {
        self.slept.set(self.slept.get() + 1);
        if self.slept.get() == self.after {
            self.fs
                .remove_dir_all(&Path::new(ROOT).join("v20.10.0"))
                .unwrap();
        }
    }
}

#[test]
fn a_free_lock_is_taken_at_once_and_released_when_dropped() {
    let fs = FakeFileSystem::default();
    let sleeper = FakeSleeper::default();
    let mut notes = Vec::new();
    let lock = acquire(&fs, &sleeper, &request(0, 600), &mut notes).unwrap();
    assert!(exists(&fs, "v20.10.0"));
    assert!(notes.is_empty() && sleeper.slept().is_empty());
    drop(lock);
    assert!(!exists(&fs, "v20.10.0"));
}

#[test]
fn the_name_of_the_lock_keeps_only_safe_characters() {
    assert_eq!(lock_name("iojs-v3.3.1"), "iojs-v3.3.1");
    assert_eq!(lock_name("v1/../2 x"), "v1_.._2_x");
}

#[test]
fn a_held_lock_is_waited_for_a_second_at_a_time_and_announced_once() {
    let fs = FakeFileSystem::default().with_dir("/n/.cache/locks/v20.10.0");
    let sleeper = ReleasesAfter {
        fs: &fs,
        after: 3,
        slept: Cell::new(0),
    };
    let mut notes = Vec::new();
    let lock = acquire(&fs, &sleeper, &request(0, 600), &mut notes).unwrap();
    assert!(lock.is_some());
    assert_eq!(sleeper.slept.get(), 3);
    assert_eq!(
        notes,
        ["Waiting for another install of v20.10.0 to finish..."]
    );
}

#[test]
fn a_lock_that_stays_held_times_out_with_both_messages() {
    let fs = FakeFileSystem::default().with_dir("/n/.cache/locks/v20.10.0");
    let sleeper = FakeSleeper::default();
    let error = acquire(&fs, &sleeper, &request(0, 2), &mut Vec::new())
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "Timed out after 2s waiting for another install of v20.10.0 to finish.\n\
         If no other install is running, remove /n/.cache/locks/v20.10.0 and try again."
    );
    assert_eq!(sleeper.slept().len(), 2);
}

#[test]
fn an_old_lock_is_stolen_when_a_stale_age_is_set() {
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000 - 11 * 60);
    let fs = FakeFileSystem::default()
        .with_dir("/n/.cache/locks/v20.10.0")
        .with_modified("/n/.cache/locks/v20.10.0", old);
    let mut notes = Vec::new();
    let lock = acquire(&fs, &FakeSleeper::default(), &request(10, 600), &mut notes).unwrap();
    assert!(lock.is_some());
    assert_eq!(
        notes,
        ["Removing stale install lock for v20.10.0 (older than 10 minute(s))"]
    );
}

#[test]
fn a_recent_lock_is_not_stolen_and_stale_zero_never_steals() {
    let recent = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000 - 60);
    let held = || {
        FakeFileSystem::default()
            .with_dir("/n/.cache/locks/v20.10.0")
            .with_modified("/n/.cache/locks/v20.10.0", recent)
    };
    for stale in [10, 0] {
        let fs = held();
        let result = acquire(
            &fs,
            &FakeSleeper::default(),
            &request(stale, 1),
            &mut Vec::new(),
        );
        assert!(result.is_err(), "stale = {stale}");
    }
}

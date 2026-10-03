//! The per-version install lock of `nvm_acquire_install_lock`: a directory
//! under `$NVM_DIR/.cache/locks`, created atomically, so two installs of one
//! version never work on its directory at once.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::error::CliError;
use crate::ports::{FileSystem, Sleeper};

const SECONDS_PER_MINUTE: u64 = 60;

pub struct LockRequest<'a> {
    /// `$NVM_DIR/.cache/locks`.
    pub root: &'a Path,
    /// The version being installed: its characters other than letters,
    /// digits and `._+-` become `_` in the lock's name.
    pub version: &'a str,
    pub timeout_seconds: u64,
    /// A lock older than this many minutes is stolen; 0 never steals.
    pub stale_minutes: u64,
    pub now: SystemTime,
}

/// A held lock, released when dropped.
pub struct InstallLock<'a> {
    fs: &'a dyn FileSystem,
    path: PathBuf,
}

impl Drop for InstallLock<'_> {
    fn drop(&mut self) {
        // Nothing can be done about a lock that cannot be removed.
        let _ = self.fs.remove_dir_all(&self.path);
    }
}

fn lock_name(version: &str) -> String {
    version
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._+-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn is_stale(fs: &dyn FileSystem, path: &Path, request: &LockRequest<'_>, waited: u64) -> bool {
    if request.stale_minutes == 0 {
        return false;
    }
    let age = fs
        .file_info(path)
        .ok()
        .and_then(|info| info.modified)
        .and_then(|modified| {
            let now = request.now + Duration::from_secs(waited);
            now.duration_since(modified).ok()
        });
    age.is_some_and(|age| age > Duration::from_secs(request.stale_minutes * SECONDS_PER_MINUTE))
}

/// Takes the lock, waiting a second at a time for another install to finish.
/// `None` when the lock directory cannot be created at all: `nvm.sh` does not
/// let that stop an install. `notes` collects what `nvm.sh` prints on stderr.
///
/// # Errors
/// [`CliError::InvalidArgument`] (exit 1) when the lock is still held after
/// `timeout_seconds`.
pub fn acquire<'a>(
    fs: &'a dyn FileSystem,
    sleeper: &dyn Sleeper,
    request: &LockRequest<'_>,
    notes: &mut Vec<String>,
) -> Result<Option<InstallLock<'a>>, CliError> {
    let root_exists = fs.create_dir_all(request.root).is_ok();
    if !root_exists {
        return Ok(None);
    }
    let path = request.root.join(lock_name(request.version));
    let mut waited = 0;
    loop {
        match fs.create_dir(&path) {
            Ok(()) => return Ok(Some(InstallLock { fs, path })),
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Ok(None),
            Err(_) => {}
        }
        if steal_if_stale(fs, &path, request, waited, notes) {
            continue;
        }
        if waited >= request.timeout_seconds {
            return Err(timed_out(request, &path));
        }
        if waited == 0 {
            notes.push(waiting(request));
        }
        sleeper.sleep(Duration::from_secs(1));
        waited += 1;
    }
}

/// An abandoned lock is removed, with a note; true when it was.
fn steal_if_stale(
    fs: &dyn FileSystem,
    path: &Path,
    request: &LockRequest<'_>,
    waited: u64,
    notes: &mut Vec<String>,
) -> bool {
    if !is_stale(fs, path, request, waited) {
        return false;
    }
    notes.push(format!(
        "Removing stale install lock for {} (older than {} minute(s))",
        request.version, request.stale_minutes
    ));
    let _ = fs.remove_dir_all(path);
    true
}

fn waiting(request: &LockRequest<'_>) -> String {
    format!(
        "Waiting for another install of {} to finish...",
        request.version
    )
}

fn timed_out(request: &LockRequest<'_>, path: &Path) -> CliError {
    CliError::InvalidArgument(format!(
        "Timed out after {}s waiting for another install of {} to finish.\n\
         If no other install is running, remove {} and try again.",
        request.timeout_seconds,
        request.version,
        path.display()
    ))
}

#[cfg(test)]
mod tests;

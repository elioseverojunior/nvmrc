//! Replacing a file atomically, through a symbolic link.

use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// How many names are tried before giving up on a free temporary file.
const NAME_ATTEMPTS: u32 = 100;

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

pub(super) fn replace_file(
    path: &Path,
    contents: &str,
    verify: &dyn Fn(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let target = resolve_target(path)?;
    let permissions = permissions_of(&target)?;
    let (file, temporary) = create_temporary(&target)?;
    let result = fill(file, contents, permissions)
        .and_then(|()| verify(&temporary))
        .and_then(|()| fs::rename(&temporary, &target));
    if result.is_err() {
        // The rename did not happen, so the temporary file is still there.
        let _ = fs::remove_file(&temporary);
    } else {
        sync_directory(&target);
    }
    result
}

/// Makes the rename itself durable by syncing the directory entry. Best
/// effort: the new contents are already in place and synced.
fn sync_directory(target: &Path) {
    #[cfg(unix)]
    if let Some(directory) = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let _ = File::open(directory).and_then(|handle| handle.sync_all());
    }
}

/// The file a write to `path` must replace: the end of a symbolic link (the
/// link itself is kept), or `path`, which may not exist yet.
fn resolve_target(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::canonicalize(path).map_err(|error| unfollowable(path, &error))
        }
        Ok(_) => Ok(path.to_path_buf()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

/// The error for a link at `link` that `canonicalize` could not follow:
/// "dangling" only when its target is missing (a loop, ELOOP, or a denied
/// directory is not).
fn unfollowable(link: &Path, error: &io::Error) -> io::Error {
    let what = if error.kind() == io::ErrorKind::NotFound {
        "dangling symbolic link"
    } else {
        "symbolic link that cannot be followed"
    };
    io::Error::new(
        error.kind(),
        format!("{}: {what} ({error})", link.display()),
    )
}

/// The permissions of `target`, or those of a new file when it is missing.
fn permissions_of(target: &Path) -> io::Result<Option<Permissions>> {
    match fs::metadata(target) {
        Ok(metadata) => Ok(Some(metadata.permissions())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(new_file_permissions()),
        Err(error) => Err(error),
    }
}

fn new_file_permissions() -> Option<Permissions> {
    use std::os::unix::fs::PermissionsExt;
    Some(Permissions::from_mode(0o644))
}

/// A new, empty file next to `target`, with a name no one else uses.
fn create_temporary(target: &Path) -> io::Result<(File, PathBuf)> {
    let directory = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut last_error = io::Error::from(io::ErrorKind::AlreadyExists);
    for _ in 0..NAME_ATTEMPTS {
        let counter = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let name = format!(".nvmrc-tmp-{}-{counter}", std::process::id());
        let temporary = directory.join(name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((file, temporary)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => last_error = error,
            Err(error) => return Err(error),
        }
    }
    Err(last_error)
}

/// Sets the permissions, writes `contents` and syncs them to the disk.
fn fill(mut file: File, contents: &str, permissions: Option<Permissions>) -> io::Result<()> {
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    file.write_all(contents.as_bytes())?;
    file.sync_all()
}

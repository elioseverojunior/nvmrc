//! Creating a new file that keeps the permissions of the file it copies.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// The permissions of a copy whose original has none to give.
const PRIVATE: u32 = 0o600;

pub(super) fn create_new_file(path: &Path, contents: &str, mode_of: &Path) -> io::Result<()> {
    let file = open_new(path)?;
    let result = fill(file, contents, mode_of);
    if result.is_err() {
        // The file is ours (create_new), and incomplete.
        let _ = fs::remove_file(path);
    }
    result
}

/// `path`, created (never opened through a link) and private until filled.
fn open_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    options.mode(PRIVATE);
    options.open(path)
}

fn fill(mut file: File, contents: &str, mode_of: &Path) -> io::Result<()> {
    copy_mode(&file, mode_of)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()
}

/// Gives `file` the permission bits of `mode_of` (links followed), `0o600`
/// when it has none.
fn copy_mode(file: &File, mode_of: &Path) -> io::Result<()> {
    let bits =
        fs::metadata(mode_of).map_or(PRIVATE, |metadata| metadata.permissions().mode() & 0o777);
    file.set_permissions(fs::Permissions::from_mode(bits))
}

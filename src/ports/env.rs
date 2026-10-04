use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

pub trait Env {
    /// The variable as text; `None` when unset or not valid UTF-8.
    fn var(&self, key: &str) -> Option<String>;

    /// The variable as an OS string, so non-UTF-8 paths survive.
    fn var_os(&self, key: &str) -> Option<OsString>;

    /// Every variable, as text. A variable whose name or value is not valid
    /// UTF-8 is skipped.
    fn vars(&self) -> Vec<(String, String)>;

    /// The directory of the process, as the system resolves it.
    ///
    /// # Errors
    /// Fails when it cannot be read (removed, or not searchable).
    fn current_dir(&self) -> io::Result<PathBuf>;
}

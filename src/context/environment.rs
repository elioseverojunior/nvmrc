//! The working directory and the variables a command rewrites, read as a
//! shell would read them.

use std::path::{Path, PathBuf};

use super::Context;
use crate::error::CliError;

impl Context<'_> {
    /// The logical working directory, by the rule bash applies at startup:
    /// `$PWD` when it is absolute, has no `.` or `..` component and is the
    /// same directory as the process's own; otherwise the process's own
    /// directory. When that cannot be read, an absolute `$PWD` is kept, as
    /// bash keeps it then.
    #[must_use]
    pub fn working_directory(&self) -> Option<PathBuf> {
        let logical = self
            .env
            .var_os("PWD")
            .map(PathBuf::from)
            .filter(|directory| is_plain_absolute(directory));
        let Ok(actual) = self.env.current_dir() else {
            return logical;
        };
        let kept = logical.filter(|directory| self.fs.same_directory(directory, &actual));
        Some(kept.unwrap_or(actual))
    }

    /// The text of the variable `name`, empty when it is unset.
    ///
    /// # Errors
    /// [`CliError::NotText`] when it is set but is not UTF-8: a command that
    /// rewrites it must not rebuild it from nothing.
    pub fn text_var(&self, name: &str) -> Result<String, CliError> {
        match self.env.var_os(name) {
            None => Ok(String::new()),
            Some(value) => value
                .into_string()
                .map_err(|_| CliError::NotText(name.to_owned())),
        }
    }
}

/// Absolute, without a `.` or `..` component.
fn is_plain_absolute(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    path.is_absolute()
        && bytes
            .split(|byte| *byte == b'/')
            .all(|component| component != b"." && component != b"..")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    /// `/real` and `/other`, with `/link` pointing at `/real`.
    fn tree() -> FakeFileSystem {
        let fs = FakeFileSystem::default()
            .with_dir("/real")
            .with_dir("/other");
        fs.symlink(Path::new("/real"), Path::new("/link")).unwrap();
        fs
    }

    fn working_directory(pwd: Option<&str>, actual: Option<&str>) -> Option<PathBuf> {
        let fs = tree();
        let env = match pwd {
            Some(pwd) => FakeEnv::default().with_var("PWD", pwd),
            None => FakeEnv::default(),
        };
        let env = match actual {
            Some(actual) => env.with_current_dir(actual),
            None => env,
        };
        Context::new(&fs, &env).working_directory()
    }

    #[test]
    fn a_pwd_that_names_the_directory_is_kept_even_through_a_symlink() {
        let kept = working_directory(Some("/link"), Some("/real"));
        assert_eq!(kept, Some(PathBuf::from("/link")));
    }

    #[test]
    fn a_stale_relative_or_missing_pwd_gives_way_to_the_actual_directory() {
        for pwd in [Some("/other"), Some("real"), Some("/gone"), None] {
            let found = working_directory(pwd, Some("/real"));
            assert_eq!(found, Some(PathBuf::from("/real")), "{pwd:?}");
        }
    }

    #[test]
    fn without_an_actual_directory_only_an_absolute_pwd_is_kept() {
        let kept = working_directory(Some("/link"), None);
        assert_eq!(kept, Some(PathBuf::from("/link")));
        assert_eq!(working_directory(Some("link"), None), None);
        assert_eq!(working_directory(None, None), None);
    }

    #[test]
    fn a_pwd_with_dot_components_is_not_plain() {
        assert!(is_plain_absolute(Path::new("/a/b")));
        for path in ["/a/../b", "/a/./b", "/a/..", "a/b", ""] {
            assert!(!is_plain_absolute(Path::new(path)), "{path}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn text_var_tells_unset_from_not_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let fs = FakeFileSystem::default();
        let env = FakeEnv::default()
            .with_var("PATH", "/usr/bin")
            .with_var_os("MANPATH", OsString::from_vec(b"/caf\xe9".to_vec()));
        let context = Context::new(&fs, &env);
        assert_eq!(context.text_var("PATH").unwrap(), "/usr/bin");
        assert_eq!(context.text_var("NODE_PATH").unwrap(), "");
        let error = context.text_var("MANPATH").unwrap_err();
        assert_eq!(
            error.to_string(),
            "$MANPATH is not valid UTF-8, so nvm cannot change it."
        );
        assert_eq!(error.exit_code(), NvmExitCode::Failure);
    }
}

//! The descriptor the `nvm` function opens for the shell code, taken over at
//! startup so that no program the binary starts can inherit it.

use std::fs::File;
use std::io::{self, Write};

use crate::ports::{Env, ScriptChannel};
use crate::shell::script_descriptor;

/// The write end of the channel: a close-on-exec copy of the descriptor
/// `NVMRC_SCRIPT_FD` names, whose original is closed.
pub struct FdChannel {
    file: File,
}

impl FdChannel {
    /// Takes over the descriptor `NVMRC_SCRIPT_FD` names in `env`. `None`
    /// (the binary runs on its own) when the variable is unset, is not the
    /// number of a descriptor above stderr, or names no open descriptor.
    ///
    /// Call it once, at startup, before anything else opens a descriptor.
    #[must_use]
    pub fn from_env(env: &dyn Env) -> Option<Self> {
        let number = script_descriptor(env)?.parse::<i32>().ok()?;
        (number > 2)
            .then(|| adopt(number))?
            .map(|file| Self { file })
    }
}

impl ScriptChannel for FdChannel {
    fn send(&self, code: &str) -> io::Result<()> {
        let mut file = &self.file;
        file.write_all(code.as_bytes())?;
        file.flush()
    }
}

/// A close-on-exec copy of `descriptor` (`F_DUPFD_CLOEXEC`), the original
/// closed. `None` when it cannot be copied: a closed descriptor (EBADF)
/// leaves nothing to close; on any other failure, such as EMFILE, the
/// descriptor is left open (and inheritable) and the code falls back to
/// stdout as if the binary ran on its own.
#[cfg(unix)]
fn adopt(descriptor: i32) -> Option<File> {
    use std::os::fd::{FromRawFd, OwnedFd};
    // SAFETY: `from_raw_fd` requires an open descriptor that nothing else
    // owns. The generated `nvm` function establishes that: it opens fd N for
    // this process alone (`3>&1` in its `$(...)`) and exports
    // `NVMRC_SCRIPT_FD=N`, and the caller has checked N is digits only and
    // above stderr. The handle is taken first and the descriptor checked
    // after: if it is not open after all (the variable set by hand),
    // `try_clone` fails and the handle is `mem::forget`-ed, so it is never
    // closed or used, and nothing is closed twice. That is acceptable because
    // this runs once, at single-threaded startup, before anything else opens
    // a descriptor that could take the number N in the meantime.
    let inherited = unsafe { OwnedFd::from_raw_fd(descriptor) };
    match inherited.try_clone() {
        Ok(copy) => Some(File::from(copy)),
        Err(_) => {
            std::mem::forget(inherited);
            None
        }
    }
}

#[cfg(not(unix))]
fn adopt(_descriptor: i32) -> Option<File> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::fd::IntoRawFd;

    use super::*;
    use crate::fakes::FakeEnv;

    fn channel(value: &str) -> Option<FdChannel> {
        FdChannel::from_env(&FakeEnv::default().with_var("NVMRC_SCRIPT_FD", value))
    }

    #[test]
    fn the_code_goes_to_the_descriptor_named() {
        let target = tempfile::NamedTempFile::new().unwrap();
        let descriptor = File::create(target.path()).unwrap().into_raw_fd();
        let channel = channel(&descriptor.to_string()).expect("an open descriptor");
        channel.send("unset NVM_BIN\n").unwrap();
        let written = std::fs::read_to_string(target.path()).unwrap();
        assert_eq!(written, "unset NVM_BIN\n");
    }

    #[test]
    fn no_channel_without_a_usable_descriptor() {
        assert!(FdChannel::from_env(&FakeEnv::default()).is_none());
        let unusable = ["", "x", "3x", "-1", "0", "1", "2", "99999999999", "1000000"];
        for value in unusable {
            assert!(channel(value).is_none(), "{value}");
        }
    }
}

//! Which archive `nvm.sh` downloads: `nvm_get_artifact_compression` and
//! `nvm_supports_xz`, as a pure function of the version and the system.
//!
//! `nvm.sh` also asks whether `xz`, a new enough macOS or `liblzma` is on the
//! machine, because it hands the archive to `tar`. This port unpacks `.tar.xz`
//! itself, so only the version and the operating system decide.

use crate::domain::platform::Os;
use crate::domain::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Xz,
}

impl Compression {
    /// The compression `nvm.sh` picks for `version` on `os`.
    #[must_use]
    pub fn preferred(version: &Version, os: Os) -> Self {
        if supports_xz(version.triple(), os) {
            Self::Xz
        } else {
            Self::Gzip
        }
    }

    /// The extension of an archive, without the dot.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Gzip => "tar.gz",
            Self::Xz => "tar.xz",
        }
    }
}

/// Whether nodejs.org publishes a `.tar.xz` for `triple`, in the order
/// `nvm_supports_xz` asks.
fn supports_xz(triple: (u64, u64, u64), os: Os) -> bool {
    if triple >= (4, 0, 0) {
        return true;
    }
    if ((0, 12, 10)..(0, 13, 0)).contains(&triple) {
        return true;
    }
    if ((0, 10, 42)..(0, 11, 0)).contains(&triple) {
        return true;
    }
    // io.js only has xz from 1.0.0, and on macOS from 2.3.2.
    let first_iojs = if os == Os::Darwin {
        (2, 3, 2)
    } else {
        (1, 0, 0)
    };
    triple >= first_iojs
}

#[cfg(test)]
mod tests;

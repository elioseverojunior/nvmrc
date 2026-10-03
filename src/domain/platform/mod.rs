//! Which prebuilt binary fits this machine: `nvm_get_os`, `nvm_get_arch` and
//! `nvm_get_download_slug`, as pure functions of the host description.

use crate::domain::version::{Flavor, Version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Darwin,
    Aix,
    /// No official binaries: it is built from source.
    FreeBsd,
    /// No official binaries: it is built from source.
    OpenBsd,
}

impl Os {
    /// The word `nvm.sh` uses in a download name.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Darwin => "darwin",
            Self::Aix => "aix",
            Self::FreeBsd => "freebsd",
            Self::OpenBsd => "openbsd",
        }
    }

    /// Whether nodejs.org publishes binaries for it.
    #[must_use]
    pub fn has_binaries(self) -> bool {
        !matches!(self, Self::FreeBsd | Self::OpenBsd)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    /// The architecture as nodejs.org names it: `x64`, `arm64`, `armv7l`, ...
    pub arch: String,
}

impl Platform {
    /// The platform from Rust's own names for it (`std::env::consts`), or
    /// `None` for an operating system that has no official binaries here.
    /// `musl` is true on Alpine, whose binaries live under a `-musl` name.
    #[must_use]
    pub fn from_host(os: &str, arch: &str, musl: bool) -> Option<Self> {
        let os = match os {
            "linux" => Os::Linux,
            "macos" => Os::Darwin,
            "aix" => Os::Aix,
            "freebsd" => Os::FreeBsd,
            "openbsd" => Os::OpenBsd,
            _ => return None,
        };
        let mut arch = match arch {
            "x86_64" => "x64",
            "x86" => "x86",
            "aarch64" => "arm64",
            "arm" => "armv7l",
            "loongarch64" => "loong64",
            "powerpc64" => "ppc64",
            other => other,
        }
        .to_owned();
        if musl && os == Os::Linux && matches!(arch.as_str(), "x64" | "arm64") {
            arch.push_str("-musl");
        }
        Some(Self { os, arch })
    }

    /// `node-v20.1.0-linux-x64`: the name of the archive without its extension
    /// and of the directory the cache keeps it in.
    #[must_use]
    pub fn download_slug(&self, version: &Version) -> String {
        let flavor = match version.flavor {
            Flavor::Node => "node",
            Flavor::IoJs => "iojs",
        };
        format!(
            "{flavor}-{}-{}-{}",
            version.directory_name(),
            self.os.slug(),
            self.arch_for(version)
        )
    }

    /// Old 32-bit ARM builds are named `arm-pi`, and Apple Silicon runs the
    /// x64 build of anything before Node 16.
    fn arch_for(&self, version: &Version) -> &str {
        let merged = version.major >= 4;
        if !merged && matches!(self.arch.as_str(), "armv6l" | "armv7l") {
            return "arm-pi";
        }
        if version.major < 16 && self.os == Os::Darwin && self.arch == "arm64" {
            return "x64";
        }
        &self.arch
    }
}

/// Binaries exist from Node 0.8.6 on.
#[must_use]
pub fn binary_available(version: &Version) -> bool {
    version.triple() >= (0, 8, 6)
}

#[cfg(test)]
mod tests;

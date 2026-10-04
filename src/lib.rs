//! nvmrc: a native Rust port of nvm.

// nvmrc runs on Unix only (Linux and macOS): the shell channel is an inherited
// file descriptor, `nvm-exec` replaces itself with `exec`, and installs set
// Unix permissions. See "Platforms" in docs/deviations.md.
#[cfg(not(unix))]
compile_error!("nvmrc supports Unix only (Linux and macOS); see docs/deviations.md");

pub mod adapters;
pub mod cli;
pub mod commands;
pub mod context;
pub mod domain;
pub mod error;
pub mod ports;
pub mod shell;

#[cfg(test)]
pub(crate) mod fakes;

//! nvmrc: a native Rust port of nvm.

pub mod adapters;
pub mod domain;
pub mod error;
pub mod ports;

#[cfg(test)]
pub(crate) mod fakes;

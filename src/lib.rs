//! nvmrc: a native Rust port of nvm.

pub mod adapters;
pub mod cli;
pub mod commands;
pub mod context;
pub mod domain;
pub mod error;
pub mod ports;

#[cfg(test)]
pub(crate) mod fakes;

//! Reading the policy from where it lives and deciding whether it applies.

pub mod load;
pub mod mode;

pub use load::{REPOSITORY_POLICY_FILE, policy_mode_requested, read_location};
pub use mode::{Mode, resolve_mode};

#[cfg(test)]
mod tests;

//! Reading the policy from where it lives and deciding whether it applies.

mod advisories;
mod cve_layers;
pub mod enforce;
pub mod gate;
pub mod load;
pub mod mode;

pub use load::{REPOSITORY_POLICY_FILE, policy_mode_requested, read_location};
pub use mode::{EnforcedPolicy, Mode, resolve_mode};

#[cfg(test)]
mod tests;

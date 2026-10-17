//! Reading the policy from where it lives and deciding whether it applies.

mod advisories;
pub mod bundled;
mod cve_layers;
pub mod enforce;
pub mod gate;
pub mod judge;
pub mod load;
pub mod mode;
mod report;
mod tool_layers;
pub mod tools;

pub use load::{REPOSITORY_POLICY_FILE, policy_mode_requested, read_location};
pub use mode::{EnforcedPolicy, Mode, resolve_mode};

#[cfg(test)]
mod tests;

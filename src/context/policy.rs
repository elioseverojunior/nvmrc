//! The policy settings a command carries: the flags and the advisory mode.

use super::Context;
use crate::domain::policy::PolicyFlags;

impl Context<'_> {
    /// The policy flags of the command line, already stripped from it; a
    /// context starts out with none.
    #[must_use]
    pub fn with_policy_flags(mut self, flags: PolicyFlags) -> Self {
        self.policy_flags = flags;
        self
    }

    #[must_use]
    pub fn policy_flags(&self) -> &PolicyFlags {
        &self.policy_flags
    }

    /// Makes the policy advise instead of enforce; a context starts out
    /// enforcing.
    #[must_use]
    pub fn advisory(mut self) -> Self {
        self.advisory = true;
        self
    }

    #[must_use]
    pub fn is_advisory(&self) -> bool {
        self.advisory
    }
}

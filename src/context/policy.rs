//! The policy settings a command carries: the flags, the advisory mode and
//! the client the policy fetches with.

use super::Context;
use crate::domain::policy::PolicyFlags;
use crate::ports::Http;

impl<'a> Context<'a> {
    /// The client of the automatic paths: one attempt, a short timeout, so a
    /// slow or silent network never holds the shell. A context starts out
    /// without one, and then fetches with [`Self::http`] on every path.
    #[must_use]
    pub fn with_quick_http(mut self, http: &'a dyn Http) -> Self {
        self.quick_http = Some(http);
        self
    }

    /// The client of a policy fetch: the quick one when advising, the regular
    /// (retrying) one when enforcing.
    #[must_use]
    pub fn policy_http(&self) -> &'a dyn Http {
        match self.quick_http {
            Some(quick) if self.advisory => quick,
            _ => self.http,
        }
    }
}

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

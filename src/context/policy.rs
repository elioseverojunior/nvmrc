//! The policy settings a command carries: the flags, the advisory mode and
//! the clients the policy and the third-party fetches go through.

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

    /// The clients of the hosts `NVM_AUTH_HEADER` is not meant for (the npm
    /// registry, its tarballs, the built-in vulnerability index): `regular`
    /// retries, `quick` is the one of the automatic paths. A context starts
    /// out without either.
    #[must_use]
    pub fn with_third_party_http(mut self, regular: &'a dyn Http, quick: &'a dyn Http) -> Self {
        self.third_party_http = regular;
        self.quick_third_party_http = quick;
        self
    }

    /// The client of a third-party fetch: the quick one when advising, the
    /// regular one when enforcing, as [`Self::policy_http`].
    #[must_use]
    pub fn third_party_http(&self) -> &'a dyn Http {
        if self.advisory {
            self.quick_third_party_http
        } else {
            self.third_party_http
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

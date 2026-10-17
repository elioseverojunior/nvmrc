//! "Is this version allowed?" for callers that choose versions (`nvm init`):
//! the mode resolved once, the vulnerability index read at most once.

use super::advisories::{failing, load_advisories};
use super::enforce::today;
use super::mode::{EnforcedPolicy, Mode, resolve_mode};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::policy::{Advisory, ReleaseInfo, Violation, findings};
use crate::domain::tool::Tool;
use crate::domain::version::{Flavor, Version};
use crate::error::PolicyError;

/// Decides whether a version is allowed; without a policy everything is.
pub struct Judge {
    policy: Option<EnforcedPolicy>,
    advisories: Option<Vec<Advisory>>,
}

impl Judge {
    /// Resolves the mode once and prints its warnings to `transcript`.
    ///
    /// # Errors
    /// A [`PolicyError`] when a policy source in use cannot be read.
    pub fn load(context: &Context<'_>, transcript: &mut Transcript) -> Result<Self, PolicyError> {
        let (mode, warnings) = resolve_mode(context)?;
        warnings.into_iter().for_each(|line| transcript.err(line));
        let policy = match mode {
            Mode::Enforced(policy) => Some(policy),
            Mode::Legacy => None,
        };
        Ok(Self {
            policy,
            advisories: None,
        })
    }

    /// Why `version` of `tool` is not allowed; empty when it is. For node
    /// the vulnerability index is read on the first call and reused.
    ///
    /// # Errors
    /// [`PolicyError`] when the index cannot be read or understood.
    pub fn violations(
        &mut self,
        context: &Context<'_>,
        tool: Tool,
        version: &Version,
        release: &ReleaseInfo,
    ) -> Result<Vec<Violation>, PolicyError> {
        let Some(policy) = &self.policy else {
            return Ok(Vec::new());
        };
        let mut found = policy
            .tool(tool)
            .violations(version, Some(release), today(context))
            .map_err(|_| PolicyError::ReleaseInfoUnavailable("release data".to_owned()))?;
        if tool == Tool::Node && version.flavor == Flavor::Node {
            found.extend(self.vulnerable(context, version)?);
        }
        Ok(found)
    }

    fn vulnerable(
        &mut self,
        context: &Context<'_>,
        version: &Version,
    ) -> Result<Option<Violation>, PolicyError> {
        let Some(policy) = &self.policy else {
            return Ok(None);
        };
        if self.advisories.is_none() {
            self.advisories = Some(load_advisories(context, &policy.cve)?);
        }
        let advisories = self.advisories.as_deref().unwrap_or_default();
        let os = context.platform().map(|platform| platform.os.slug());
        let judgement = failing(findings(advisories, version, os), policy.cve.limit.value);
        if judgement.failing.is_empty() {
            return Ok(None);
        }
        Ok(Some(Violation::Vulnerable {
            findings: judgement.failing,
            limit: policy.cve.limit.clone(),
        }))
    }
}

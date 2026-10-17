//! "Is this version allowed?" for callers that choose versions (`nvm init`):
//! the mode resolved once, the vulnerability index read at most once.

use super::advisories::{below_limit_warning, load_advisories, vulnerable};
use super::enforce::{index_location, today};
use super::mode::{EnforcedPolicy, Mode, resolve_mode};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::policy::{Advisory, Finding, MissingReleaseInfo, ReleaseInfo, Violation};
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
            .map_err(|MissingReleaseInfo| missing_release_info(context, tool, version))?;
        if tool == Tool::Node && version.flavor == Flavor::Node {
            found.extend(self.node_vulnerability(context, version)?);
        }
        Ok(found)
    }

    /// The warning for the vulnerabilities of node `version` below the CVE
    /// limit, if it has any; the index is read as by [`Self::violations`].
    ///
    /// # Errors
    /// [`PolicyError`] when the index cannot be read or understood.
    pub fn below_limit_warning(
        &mut self,
        context: &Context<'_>,
        version: &Version,
    ) -> Result<Option<String>, PolicyError> {
        if version.flavor != Flavor::Node {
            return Ok(None);
        }
        let (_, below) = self.node_findings(context, version)?;
        Ok(self
            .policy
            .as_ref()
            .and_then(|policy| below_limit_warning(version, &policy.cve.limit.value, &below)))
    }

    fn node_vulnerability(
        &mut self,
        context: &Context<'_>,
        version: &Version,
    ) -> Result<Option<Violation>, PolicyError> {
        Ok(self.node_findings(context, version)?.0)
    }

    /// What the index says of node `version`, split by the limit (see
    /// [`vulnerable`]); nothing without a policy.
    fn node_findings(
        &mut self,
        context: &Context<'_>,
        version: &Version,
    ) -> Result<(Option<Violation>, Vec<Finding>), PolicyError> {
        let Some(policy) = &self.policy else {
            return Ok((None, Vec::new()));
        };
        let advisories = match &self.advisories {
            Some(loaded) => loaded,
            None => self
                .advisories
                .insert(load_advisories(context, &policy.cve)?),
        };
        Ok(vulnerable(context, advisories, version, &policy.cve.limit))
    }
}

/// Where the release data a rule needs should have come from: the node
/// index, or the registry entry of a package manager.
fn missing_release_info(context: &Context<'_>, tool: Tool, version: &Version) -> PolicyError {
    PolicyError::ReleaseInfoUnavailable(match tool {
        Tool::Node => index_location(context),
        _ => format!(
            "{} {} has no publish date in the registry",
            tool.name(),
            tool.show(version)
        ),
    })
}

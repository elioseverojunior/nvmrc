//! "Is this version allowed?" for callers that choose versions (`nvm init`):
//! the mode resolved once, the vulnerability index read at most once.

use super::advisories::{below_limit_warning, load_advisories, vulnerable};
use super::enforce::{index_location, today};
use super::mode::{EnforcedPolicy, Mode, resolve_mode};
use super::tools::release_info;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::policy::{Advisory, Finding, MissingReleaseInfo, ReleaseInfo, Violation};
use crate::domain::tool::Tool;
use crate::domain::version::{Flavor, Version};
use crate::error::PolicyError;

/// What the policy says of the npm a node release bundles, when npm is not
/// pinned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundledNpm {
    Allowed,
    /// npm has rules, is not pinned, and the index gives no npm for the
    /// release.
    Unknown,
    NotAllowed {
        npm: Version,
        violations: Vec<Violation>,
    },
    /// The check failed on a release its own violations already refuse;
    /// the warning to print after the block.
    Unchecked(String),
}

impl BundledNpm {
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Why the policy refuses a release: the rules it breaks and, for node,
/// what it says of the npm the release bundles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub violations: Vec<Violation>,
    pub bundled: BundledNpm,
}

impl Refusal {
    /// Whether nothing is refused.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.violations.is_empty() && self.bundled.is_allowed()
    }
}

/// Decides whether a version is allowed; without a policy everything is.
pub struct Judge {
    policy: Option<EnforcedPolicy>,
    advisories: Option<Vec<Advisory>>,
    /// Whether npm is pinned once the caller is done; when it is not, the
    /// npm a node release bundles is the one in effect.
    npm_pinned: bool,
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
            npm_pinned: false,
        })
    }

    /// The judge told whether npm is pinned once the caller is done.
    #[must_use]
    pub fn with_npm_pinned(self, npm_pinned: bool) -> Self {
        Self { npm_pinned, ..self }
    }

    /// What the policy says of the npm bundled with a release of `tool`:
    /// allowed for a tool other than node, with npm pinned or without an
    /// npm rule; never when the bundled npm is unknown.
    ///
    /// # Errors
    /// [`PolicyError`] when the release date an npm age rule needs cannot
    /// be had.
    pub fn bundled_npm(
        &mut self,
        context: &Context<'_>,
        tool: Tool,
        bundled: Option<&Version>,
    ) -> Result<BundledNpm, PolicyError> {
        let Some(policy) = self.policy.as_ref().filter(|_| tool == Tool::Node) else {
            return Ok(BundledNpm::Allowed);
        };
        let rules = policy.tool(Tool::Npm);
        if self.npm_pinned || rules.is_unrestricted() {
            return Ok(BundledNpm::Allowed);
        }
        let Some(npm) = bundled else {
            return Ok(BundledNpm::Unknown);
        };
        let release = release_info(context, false, rules, npm)?.unwrap_or(ReleaseInfo {
            lts: None,
            published_day: None,
        });
        let violations = self.violations(context, Tool::Npm, npm, &release)?;
        Ok(if violations.is_empty() {
            BundledNpm::Allowed
        } else {
            BundledNpm::NotAllowed {
                npm: *npm,
                violations,
            }
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

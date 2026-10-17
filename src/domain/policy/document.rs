//! The policy file: its types, which are the schema's single source of truth,
//! and the parsing that enforces what a schema cannot.

use std::fmt;

use nodejs_semver::Range;
use serde::Deserialize;

use crate::domain::version::{Flavor, Version, VersionPattern};

const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[cfg_attr(
    test,
    schemars(extend(
        "$id" = "https://raw.githubusercontent.com/elioseverojunior/nvmrc/main/schemas/nvmrc-policy.v1.schema.json"
    ))
)]
pub struct PolicyDocument {
    /// Lets editors validate the file; ignored.
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    #[cfg_attr(test, schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
    #[serde(default)]
    pub tools: Tools,
    pub cve: Option<CveSettings>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Tools {
    pub node: Option<NodeRules>,
    pub npm: Option<PackageRules>,
    pub yarn: Option<PackageRules>,
    pub pnpm: Option<PackageRules>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct NodeRules {
    /// The lowest allowed version; a partial version means its lowest release.
    #[cfg_attr(test, schemars(regex(pattern = r"^v?\d+(\.\d+){0,2}$")))]
    pub minimum: Option<String>,
    /// An npm-style semver range.
    pub allow: Option<String>,
    /// Exact versions that are never allowed.
    #[cfg_attr(test, schemars(inner(regex(pattern = r"^v?\d+\.\d+\.\d+$"))))]
    pub deny: Option<Vec<String>>,
    /// Allow only releases that have an LTS name in the index.
    pub lts_only: Option<bool>,
    /// Days a release must have been published before it is allowed.
    pub minimum_release_age_days: Option<u32>,
}

/// The rules of npm, yarn and pnpm: those of node, without `ltsOnly`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct PackageRules {
    /// The lowest allowed version; a partial version means its lowest release.
    #[cfg_attr(test, schemars(regex(pattern = r"^v?\d+(\.\d+){0,2}$")))]
    pub minimum: Option<String>,
    /// An npm-style semver range.
    pub allow: Option<String>,
    /// Exact versions that are never allowed.
    #[cfg_attr(test, schemars(inner(regex(pattern = r"^v?\d+\.\d+\.\d+$"))))]
    pub deny: Option<Vec<String>>,
    /// Days a release must have been published before it is allowed.
    pub minimum_release_age_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct CveSettings {
    /// The lowest severity that fails the check; `none` only reports.
    pub fail_on_severity: Option<Severity>,
    /// A URL or local path of the vulnerability index.
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    None,
}

impl Severity {
    /// Orders severities: `None` is lowest, `Critical` highest.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::None => "none",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("the policy is not valid: {0}")]
    Malformed(String),
    #[error("schemaVersion {0} is not understood; this nvmrc understands version 1")]
    UnsupportedSchemaVersion(u32),
    #[error("{field}: '{value}' is not a valid version")]
    InvalidVersion { field: String, value: String },
    #[error("{field}: '{value}' is not a valid semver range")]
    InvalidRange { field: String, value: String },
}

/// # Errors
/// Returns a [`DocumentError`] when `text` is not JSON, breaks the schema, has
/// an unsupported `schemaVersion`, or holds a version or range that is invalid.
pub fn parse_document(text: &str) -> Result<PolicyDocument, DocumentError> {
    let document: PolicyDocument =
        serde_json::from_str(text).map_err(|error| DocumentError::Malformed(error.to_string()))?;
    if document.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(DocumentError::UnsupportedSchemaVersion(
            document.schema_version,
        ));
    }
    let tools = &document.tools;
    let rules = [
        ("node", tools.node.as_ref().map(NodeRules::common)),
        ("npm", tools.npm.as_ref().map(PackageRules::common)),
        ("yarn", tools.yarn.as_ref().map(PackageRules::common)),
        ("pnpm", tools.pnpm.as_ref().map(PackageRules::common)),
    ];
    for (name, common) in rules {
        if let Some(common) = common {
            validate_rules(&format!("tools.{name}"), &common)?;
        }
    }
    Ok(document)
}

/// The fields every tool shares, borrowed for validation.
struct CommonRules<'a> {
    minimum: Option<&'a str>,
    allow: Option<&'a str>,
    deny: &'a [String],
}

impl NodeRules {
    fn common(&self) -> CommonRules<'_> {
        CommonRules {
            minimum: self.minimum.as_deref(),
            allow: self.allow.as_deref(),
            deny: self.deny.as_deref().unwrap_or_default(),
        }
    }
}

impl PackageRules {
    fn common(&self) -> CommonRules<'_> {
        CommonRules {
            minimum: self.minimum.as_deref(),
            allow: self.allow.as_deref(),
            deny: self.deny.as_deref().unwrap_or_default(),
        }
    }
}

fn validate_rules(prefix: &str, rules: &CommonRules<'_>) -> Result<(), DocumentError> {
    if let Some(minimum) = rules.minimum {
        parse_minimum(format!("{prefix}.minimum"), minimum)?;
    }
    if let Some(allow) = rules.allow {
        parse_range(format!("{prefix}.allow"), allow)?;
    }
    for (index, entry) in rules.deny.iter().enumerate() {
        parse_exact(format!("{prefix}.deny[{index}]"), entry)?;
    }
    Ok(())
}

/// A minimum: a Node version, partial or full, standing for its lowest
/// release. An `iojs-` version would never match a Node release.
///
/// # Errors
/// Returns [`DocumentError::InvalidVersion`] naming `field`.
pub fn parse_minimum(field: String, text: &str) -> Result<Version, DocumentError> {
    text.parse::<VersionPattern>()
        .ok()
        .filter(|pattern| pattern.flavor.is_none())
        .map(|pattern| pattern.lowest())
        .ok_or_else(|| invalid_version(field, text))
}

/// A deny entry: an exact Node version.
///
/// # Errors
/// Returns [`DocumentError::InvalidVersion`] naming `field`.
pub fn parse_exact(field: String, text: &str) -> Result<Version, DocumentError> {
    text.parse::<Version>()
        .ok()
        .filter(|version| version.flavor == Flavor::Node)
        .ok_or_else(|| invalid_version(field, text))
}

/// An allowed range; a blank one, which semver reads as `*`, is rejected so a
/// typo never allows everything.
///
/// # Errors
/// Returns [`DocumentError::InvalidRange`] naming `field`.
pub fn parse_range(field: String, text: &str) -> Result<Range, DocumentError> {
    Some(text)
        .filter(|text| !text.trim().is_empty())
        .and_then(|text| Range::parse(text).ok())
        .ok_or_else(|| DocumentError::InvalidRange {
            field,
            value: text.to_owned(),
        })
}

fn invalid_version(field: String, value: &str) -> DocumentError {
    DocumentError::InvalidVersion {
        field,
        value: value.to_owned(),
    }
}

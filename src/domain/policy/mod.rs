//! The security policy: what versions of node, npm, yarn and pnpm may be used.

pub mod advisories;
pub mod cve;
pub mod document;
pub mod flags;
pub mod rules;
pub mod source;
pub mod verdict;

pub use advisories::{Advisory, Finding, IndexError, findings, parse_advisories};
pub use cve::{CveLayer, CvePolicy, DEFAULT_INDEX, merge_cve};
pub use document::{
    CveSettings, DocumentError, NodeRules, PackageRules, PolicyDocument, Severity, Tools,
    parse_document,
};
pub use flags::{FlagError, POLICY_FLAGS, PolicyFlag, PolicyFlags, extract};
pub use rules::{NodeLayer, NodePolicy, merge};
pub use source::{Source, Sourced};
pub use verdict::{MissingReleaseInfo, ReleaseInfo, Violation, finding_list};

#[cfg(test)]
mod advisories_tests;
#[cfg(test)]
mod cve_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod flags_tests;
#[cfg(test)]
mod rules_tests;
#[cfg(test)]
mod verdict_tests;

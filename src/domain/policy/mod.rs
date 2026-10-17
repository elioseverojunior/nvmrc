//! The security policy: what versions of node, npm, yarn and pnpm may be used.

pub mod document;
pub mod flags;
pub mod rules;
pub mod source;
pub mod verdict;

pub use document::{
    CveSettings, DocumentError, NodeRules, PackageRules, PolicyDocument, Severity, Tools,
    parse_document,
};
pub use flags::{FlagError, POLICY_FLAGS, PolicyFlag, PolicyFlags, extract};
pub use rules::{NodeLayer, NodePolicy, merge};
pub use source::{Source, Sourced};
pub use verdict::{MissingReleaseInfo, ReleaseInfo, Violation};

#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod flags_tests;
#[cfg(test)]
mod rules_tests;
#[cfg(test)]
mod verdict_tests;

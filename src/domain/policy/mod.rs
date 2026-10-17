//! The security policy: what versions of node, npm, yarn and pnpm may be used.

pub mod document;
pub mod rules;
pub mod source;

pub use document::{
    CveSettings, DocumentError, NodeRules, PackageRules, PolicyDocument, Severity, Tools,
    parse_document,
};
pub use rules::{NodeLayer, NodePolicy, merge};
pub use source::{Source, Sourced};

#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod rules_tests;

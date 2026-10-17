//! The security policy: what versions of node, npm, yarn and pnpm may be used.

pub mod document;

pub use document::{
    CveSettings, DocumentError, NodeRules, PackageRules, PolicyDocument, Severity, Tools,
    parse_document,
};

#[cfg(test)]
mod document_tests;

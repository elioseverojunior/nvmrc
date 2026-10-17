//! What the gate prints: one block per offending tool, node first, each with
//! the rules it breaks and the fix.

use super::enforce::{Gate, Origin, Requested};
use super::judge::{BundledNpm, Refusal};
use super::tools::{ToolFinding, ToolOrigin};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::policy::Violation;
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// What the check found: the violations of node (its rules and its known
/// vulnerabilities), those of npm, yarn and pnpm, and the warnings printed
/// after the report so the output reads top-down.
#[derive(Default)]
pub(super) struct Checked {
    pub node: Vec<Violation>,
    pub tools: Vec<ToolFinding>,
    pub notes: Vec<String>,
}

impl Checked {
    pub fn is_clean(&self) -> bool {
        self.node.is_empty() && self.tools.is_empty()
    }
}

/// Prints the blocks of `checked`; any violation stops with exit 7, unless
/// the context is advisory, where the blocks are warnings and the gate
/// passes.
pub(super) fn report(
    context: &Context<'_>,
    requested: &Requested,
    checked: &Checked,
    transcript: &mut Transcript,
) -> Gate {
    if checked.is_clean() {
        return Gate::Pass;
    }
    let prefix = if context.is_advisory() {
        "nvm: warning:"
    } else {
        "nvm:"
    };
    if !checked.node.is_empty() {
        node_block(prefix, requested, &checked.node, transcript);
    }
    for finding in &checked.tools {
        tool_block(prefix, finding, transcript);
    }
    if context.is_advisory() {
        Gate::Pass
    } else {
        Gate::Stop(NvmExitCode::BelowVersionFloor)
    }
}

fn node_block(
    prefix: &str,
    requested: &Requested,
    violations: &[Violation],
    transcript: &mut Transcript,
) {
    let origin = &requested.origin;
    let subject = format!(
        "{prefix} node {} {}",
        requested.version,
        origin_text(origin)
    );
    let reasons = reasons(violations);
    block(&subject, &reasons, &fix_text(origin), transcript);
}

fn tool_block(prefix: &str, finding: &ToolFinding, transcript: &mut Transcript) {
    let (tool, version, origin, violations) = finding;
    let (name, shown) = (tool.name(), tool.show(version));
    let subject = format!("{prefix} {name} {shown} {}", tool_origin_text(origin));
    let reasons = reasons(violations);
    block(
        &subject,
        &reasons,
        &tool_fix_text(*tool, origin),
        transcript,
    );
}

/// The block for `version` of `tool` given on the command line, as
/// `nvm init` prints it.
pub fn argument_block(
    tool: Tool,
    version: &Version,
    refused: &Refusal,
    transcript: &mut Transcript,
) {
    let origin = Origin::Argument;
    let subject = subject(tool, version, &origin_text(&origin));
    let reasons = refusal_reasons(refused);
    block(&subject, &reasons, &fix_text(&origin), transcript);
    bundled_note(&refused.bundled, transcript);
}

/// The block for `version`, the newest release of `tool` in `major` (in
/// any major for `None`), ending in `fix`, as `nvm upgrade` prints it when
/// nothing there is allowed.
pub fn newest_block(
    tool: Tool,
    version: &Version,
    major: Option<u64>,
    refused: &Refusal,
    fix: &str,
    transcript: &mut Transcript,
) {
    let scope = match major {
        Some(major) => format!("(the newest in major {major})"),
        None => "(the newest release)".to_owned(),
    };
    let subject = subject(tool, version, &scope);
    block(&subject, &refusal_reasons(refused), fix, transcript);
    bundled_note(&refused.bundled, transcript);
}

/// `nvm: node v20.18.0 (from the command line)`.
fn subject(tool: Tool, version: &Version, origin: &str) -> String {
    format!("nvm: {} {} {origin}", tool.name(), tool.show(version))
}

fn block(subject: &str, reasons: &[String], fix: &str, transcript: &mut Transcript) {
    transcript.err(format!("{subject} is not allowed by policy:"));
    reasons
        .iter()
        .for_each(|reason| transcript.err(reason.clone()));
    transcript.err(fix.to_owned());
}

/// A bullet for each violation.
fn reasons(violations: &[Violation]) -> Vec<String> {
    violations
        .iter()
        .map(|violation| format!("  - {violation}"))
        .collect()
}

/// The bullets of the release's own violations, then those of the npm it
/// bundles.
fn refusal_reasons(refused: &Refusal) -> Vec<String> {
    let mut found = reasons(&refused.violations);
    found.extend(bundled_reasons(&refused.bundled));
    found
}

/// The warning of a bundled npm that could not be checked, after the block.
fn bundled_note(bundled: &BundledNpm, transcript: &mut Transcript) {
    if let BundledNpm::Unchecked(note) = bundled {
        transcript.err(note.clone());
    }
}

/// `  - the npm it bundles, npm 9.8.1, is below the minimum 10.0.0 (from …)`.
fn bundled_reasons(bundled: &BundledNpm) -> Vec<String> {
    match bundled {
        BundledNpm::Allowed | BundledNpm::Unchecked(_) => Vec::new(),
        BundledNpm::Unknown => {
            vec!["  - the npm it bundles is unknown, and npm has rules".to_owned()]
        }
        BundledNpm::NotAllowed { npm, violations } => {
            let shown = Tool::Npm.show(npm);
            violations
                .iter()
                .map(|violation| format!("  - the npm it bundles, npm {shown}, is {violation}"))
                .collect()
        }
    }
}

fn origin_text(origin: &Origin) -> String {
    match origin {
        Origin::Nvmrc(path) => format!("(from {})", path.display()),
        Origin::Argument => "(from the command line)".to_owned(),
        Origin::Active => "(the active version)".to_owned(),
        Origin::System => "(system)".to_owned(),
    }
}

fn fix_text(origin: &Origin) -> String {
    match origin {
        Origin::Nvmrc(path) => format!("Raise {} to an allowed version.", path.display()),
        _ => "Use an allowed version.".to_owned(),
    }
}

fn tool_origin_text(origin: &ToolOrigin) -> String {
    match origin {
        ToolOrigin::Nvmrc(path) => format!("(from {})", path.display()),
        ToolOrigin::Bundled(node) => format!("(bundled with node {node})"),
        ToolOrigin::System => "(system)".to_owned(),
    }
}

fn tool_fix_text(tool: Tool, origin: &ToolOrigin) -> String {
    match origin {
        ToolOrigin::Nvmrc(path) => format!("Raise {} to an allowed version.", path.display()),
        ToolOrigin::System => format!("Use an allowed {}.", tool.name()),
        ToolOrigin::Bundled(_) => {
            let name = tool.name();
            format!("Pin an allowed {name} in .nvmrc ({name}=<version>).")
        }
    }
}

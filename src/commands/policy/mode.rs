//! Legacy floor or enforced policy: which one applies, and the merged rules.

use super::load::{central_location, policy_mode_requested, read_location, repository_policy};
use crate::context::Context;
use crate::domain::policy::{NodeLayer, NodePolicy, Source, merge, parse_document};
use crate::domain::version::{Version, VersionPattern};
use crate::error::PolicyError;

const MINIMUM_FLAG: &str = "--min-node";
const MINIMUM_VARIABLE: &str = "NVM_MIN_VERSION";

#[derive(Debug)]
pub enum Mode {
    /// Nothing asks for a policy: the existing version floor applies.
    Legacy,
    Enforced(NodePolicy),
}

/// Decides the mode and, when a policy is enforced, merges every source:
/// central policy, repository policy, `--min-node`, `NVM_MIN_VERSION` and
/// `$NVM_DIR/min-version`, in that order, so a tie goes to the central
/// policy. The warnings name the overrides that were looser than the winner.
///
/// # Errors
/// A [`PolicyError`] when a source in use cannot be read or is not valid.
pub fn resolve_mode(context: &Context<'_>) -> Result<(Mode, Vec<String>), PolicyError> {
    if !policy_mode_requested(context) {
        return Ok((Mode::Legacy, Vec::new()));
    }
    let mut layers = Vec::new();
    layers.extend(central_layer(context)?);
    layers.extend(repository_layer(context)?);
    layers.extend(minimum_layer(
        Source::Flag(MINIMUM_FLAG),
        context.policy_flags().get(MINIMUM_FLAG).map(str::to_owned),
    )?);
    layers.extend(minimum_layer(
        Source::Variable(MINIMUM_VARIABLE),
        context.env.var(MINIMUM_VARIABLE),
    )?);
    layers.extend(file_layer(context)?);
    let (policy, warnings) = merge(layers);
    Ok((Mode::Enforced(policy), warnings))
}

fn central_layer(context: &Context<'_>) -> Result<Option<NodeLayer>, PolicyError> {
    let Some(location) = central_location(context) else {
        return Ok(None);
    };
    let text = read_location(context, &location)?;
    document_layer(&text, &location, Source::CentralPolicy(location.clone()))
}

fn repository_layer(context: &Context<'_>) -> Result<Option<NodeLayer>, PolicyError> {
    let Some(path) = repository_policy(context) else {
        return Ok(None);
    };
    let location = path.to_string_lossy().into_owned();
    let text = read_location(context, &location)?;
    document_layer(&text, &location, Source::RepositoryPolicy(path))
}

fn document_layer(
    text: &str,
    location: &str,
    source: Source,
) -> Result<Option<NodeLayer>, PolicyError> {
    let invalid = |error: crate::domain::policy::DocumentError| PolicyError::Invalid {
        location: location.to_owned(),
        reason: error.to_string(),
    };
    let document = parse_document(text).map_err(invalid)?;
    let Some(rules) = document.tools.node else {
        return Ok(None);
    };
    NodeLayer::from_rules(source, &rules)
        .map(Some)
        .map_err(invalid)
}

fn file_layer(context: &Context<'_>) -> Result<Option<NodeLayer>, PolicyError> {
    let Ok(directory) = context.nvm_dir() else {
        return Ok(None);
    };
    let path = directory.join("min-version");
    let text = match context.fs.read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(PolicyError::Unreadable {
                location: path.display().to_string(),
                reason: error.to_string(),
            });
        }
    };
    let first_line = text.lines().next().map(str::to_owned);
    minimum_layer(Source::MinVersionFile(path), first_line)
}

/// A layer that only sets the minimum, from `raw`; blank is unset.
fn minimum_layer(source: Source, raw: Option<String>) -> Result<Option<NodeLayer>, PolicyError> {
    let Some(raw) = raw
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
    else {
        return Ok(None);
    };
    let minimum = parse_minimum(&source, &raw)?;
    Ok(Some(NodeLayer::minimum_only(source, minimum)))
}

fn parse_minimum(source: &Source, raw: &str) -> Result<Version, PolicyError> {
    let pattern: VersionPattern = raw.parse().map_err(|_| PolicyError::Invalid {
        location: source.to_string(),
        reason: format!("'{raw}' is not a version"),
    })?;
    Ok(pattern.lowest())
}

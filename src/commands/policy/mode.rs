//! Legacy floor or enforced policy: which one applies, and the merged rules.

use super::cve_layers::{document_cve_layer, severity_layers};
use super::load::{
    absolute, central_location, is_https, policy_mode_requested, read_location, repository_policy,
};
use std::path::Path;

use crate::context::Context;
use crate::domain::policy::{
    CveLayer, CvePolicy, Source, ToolLayer, ToolPolicy, merge, merge_cve, parse_document,
};
use crate::domain::tool::Tool;
use crate::domain::version::{Version, VersionPattern};
use crate::error::PolicyError;

const MINIMUM_FLAG: &str = "--min-node";
const MINIMUM_VARIABLE: &str = "NVM_MIN_VERSION";

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // built once per command; boxing only adds a hop
pub enum Mode {
    /// Nothing asks for a policy: the existing version floor applies.
    Legacy,
    Enforced(EnforcedPolicy),
}

/// The merged rules of an enforced policy.
#[derive(Debug)]
pub struct EnforcedPolicy {
    pub node: ToolPolicy,
    pub cve: CvePolicy,
}

/// What one policy document contributes.
#[derive(Default)]
struct DocumentLayers {
    node: Option<ToolLayer>,
    cve: Option<CveLayer>,
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
    let central = central_layer(context)?;
    let repository = repository_layer(context)?;
    let mut layers = Vec::new();
    layers.extend(central.node);
    layers.extend(repository.node);
    layers.extend(minimum_layer(
        Source::Flag(MINIMUM_FLAG),
        context.policy_flags().get(MINIMUM_FLAG).map(str::to_owned),
    )?);
    layers.extend(minimum_layer(
        Source::Variable(MINIMUM_VARIABLE),
        context.env.var(MINIMUM_VARIABLE),
    )?);
    layers.extend(file_layer(context)?);
    let (node, mut warnings) = merge(Tool::Node, layers);
    let (cve, cve_warnings) = cve_policy(context, central.cve, repository.cve)?;
    warnings.extend(cve_warnings);
    Ok((Mode::Enforced(EnforcedPolicy { node, cve }), warnings))
}

/// Merges the CVE layers of both policies, the severity flag and the variable.
fn cve_policy(
    context: &Context<'_>,
    central: Option<CveLayer>,
    repository: Option<CveLayer>,
) -> Result<(CvePolicy, Vec<String>), PolicyError> {
    let mut layers = Vec::new();
    layers.extend(central);
    layers.extend(repository);
    layers.extend(severity_layers(context)?);
    Ok(merge_cve(layers))
}

/// A configured central policy always has a CVE layer, even an empty one, so
/// the repository policy never chooses the index once a central policy exists.
fn central_layer(context: &Context<'_>) -> Result<DocumentLayers, PolicyError> {
    let Some(location) = central_location(context) else {
        return Ok(DocumentLayers::default());
    };
    let text = read_location(context, &location)?;
    let directory = (!is_https(&location))
        .then(|| absolute(context, &location).parent().map(Path::to_path_buf))
        .flatten();
    let source = Source::CentralPolicy(location.clone());
    let mut layers = document_layers(&text, &location, source.clone(), directory.as_deref())?;
    layers.cve.get_or_insert(CveLayer {
        source,
        limit: None,
        index: None,
    });
    Ok(layers)
}

fn repository_layer(context: &Context<'_>) -> Result<DocumentLayers, PolicyError> {
    let Some(path) = repository_policy(context) else {
        return Ok(DocumentLayers::default());
    };
    let location = path.to_string_lossy().into_owned();
    let text = read_location(context, &location)?;
    let directory = path.parent().map(Path::to_path_buf);
    let source = Source::RepositoryPolicy(path);
    document_layers(&text, &location, source, directory.as_deref())
}

fn document_layers(
    text: &str,
    location: &str,
    source: Source,
    directory: Option<&Path>,
) -> Result<DocumentLayers, PolicyError> {
    let invalid = |error: crate::domain::policy::DocumentError| PolicyError::Invalid {
        location: location.to_owned(),
        reason: error.to_string(),
    };
    let document = parse_document(text).map_err(invalid)?;
    let cve = document_cve_layer(&document, location, source.clone(), directory)?;
    let node = match &document.tools.node {
        Some(rules) => Some(ToolLayer::from_node(source, rules).map_err(invalid)?),
        None => None,
    };
    Ok(DocumentLayers { node, cve })
}

fn file_layer(context: &Context<'_>) -> Result<Option<ToolLayer>, PolicyError> {
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
fn minimum_layer(source: Source, raw: Option<String>) -> Result<Option<ToolLayer>, PolicyError> {
    let Some(raw) = raw
        .map(|raw| raw.trim().to_owned())
        .filter(|raw| !raw.is_empty())
    else {
        return Ok(None);
    };
    let minimum = parse_minimum(&source, &raw)?;
    Ok(Some(ToolLayer::minimum_only(source, minimum)))
}

fn parse_minimum(source: &Source, raw: &str) -> Result<Version, PolicyError> {
    let pattern: VersionPattern = raw.parse().map_err(|_| PolicyError::Invalid {
        location: source.to_string(),
        reason: format!("'{raw}' is not a version"),
    })?;
    Ok(pattern.lowest())
}

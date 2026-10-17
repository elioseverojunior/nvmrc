//! The CVE layers of the policy sources: documents, the flag and the variable.

use std::path::Path;

use crate::context::Context;
use crate::domain::policy::{CveLayer, PolicyDocument, Severity, Source};
use crate::error::PolicyError;

const SEVERITY_FLAG: &str = "--fail-on-severity";
const SEVERITY_VARIABLE: &str = "NVMRC_FAIL_ON_SEVERITY";
const SEVERITIES: [Severity; 5] = [
    Severity::Critical,
    Severity::High,
    Severity::Medium,
    Severity::Low,
    Severity::None,
];

/// The CVE layer of a policy document, if it has `cve` settings. A relative
/// `cve.source` is read next to the policy; `policy_directory` is where the
/// policy was read from, `None` for a policy that came over https.
///
/// # Errors
/// [`PolicyError::Invalid`] for a relative source in a remote policy.
pub(super) fn document_cve_layer(
    document: &PolicyDocument,
    location: &str,
    source: Source,
    policy_directory: Option<&Path>,
) -> Result<Option<CveLayer>, PolicyError> {
    let Some(settings) = &document.cve else {
        return Ok(None);
    };
    let index = match &settings.source {
        Some(value) => Some(resolve_index(value, location, policy_directory)?),
        None => None,
    };
    Ok(Some(CveLayer {
        source,
        limit: settings.fail_on_severity,
        index,
    }))
}

fn resolve_index(
    value: &str,
    location: &str,
    policy_directory: Option<&Path>,
) -> Result<String, PolicyError> {
    if value.contains("://") || Path::new(value).is_absolute() {
        return Ok(value.to_owned());
    }
    let Some(directory) = policy_directory else {
        return Err(PolicyError::Invalid {
            location: location.to_owned(),
            reason: format!("cve.source '{value}' is relative, which a remote policy cannot be"),
        });
    };
    Ok(directory.join(value).to_string_lossy().into_owned())
}

/// The layers of the severity flag and then the variable; blank is unset.
///
/// # Errors
/// [`PolicyError::Invalid`] for a value that is not a severity.
pub(super) fn severity_layers(context: &Context<'_>) -> Result<Vec<CveLayer>, PolicyError> {
    let flag = context.policy_flags().get(SEVERITY_FLAG).map(str::to_owned);
    let variable = context.env.var(SEVERITY_VARIABLE);
    [
        (Source::Flag(SEVERITY_FLAG), flag),
        (Source::Variable(SEVERITY_VARIABLE), variable),
    ]
    .into_iter()
    .filter_map(|(source, raw)| Some((source, raw?.trim().to_owned())))
    .filter(|(_, raw)| !raw.is_empty())
    .map(|(source, raw)| {
        let limit = parse_severity(&source, &raw)?;
        Ok(CveLayer {
            source,
            limit: Some(limit),
            index: None,
        })
    })
    .collect()
}

fn parse_severity(source: &Source, raw: &str) -> Result<Severity, PolicyError> {
    SEVERITIES
        .into_iter()
        .find(|severity| severity.to_string() == raw)
        .ok_or_else(|| PolicyError::Invalid {
            location: source.to_string(),
            reason: format!("'{raw}' is not one of critical, high, medium, low, none"),
        })
}

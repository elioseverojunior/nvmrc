//! Merging the CVE settings: overrides can only tighten the severity limit.

use super::document::Severity;
use super::source::{Source, Sourced};

/// The vulnerability index used when no source names another.
pub const DEFAULT_INDEX: &str =
    "https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json";

/// The CVE settings one source contributes.
#[derive(Debug, Clone)]
pub struct CveLayer {
    pub source: Source,
    pub limit: Option<Severity>,
    pub index: Option<String>,
}

/// The merged CVE settings, each with the source that decided it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CvePolicy {
    pub limit: Sourced<Severity>,
    pub index: Sourced<String>,
}

/// Merges the layers (the central one first) into one policy and the warnings
/// for overrides that were looser than the winner.
#[must_use]
pub fn merge_cve(layers: Vec<CveLayer>) -> (CvePolicy, Vec<String>) {
    let mut warnings = Vec::new();
    let limit = merge_limit(&layers, &mut warnings);
    let index = merge_index(&layers);
    (CvePolicy { limit, index }, warnings)
}

/// Lower is stricter; `none` only reports, so it is the loosest of all.
fn strictness(severity: Severity) -> u8 {
    match severity {
        Severity::None => u8::MAX,
        other => other.rank(),
    }
}

fn merge_limit(layers: &[CveLayer], warnings: &mut Vec<String>) -> Sourced<Severity> {
    let entries = || {
        layers
            .iter()
            .filter_map(|layer| Some((&layer.source, layer.limit?)))
    };
    let mut best: Option<(&Source, Severity)> = None;
    for (source, value) in entries() {
        if best.is_none_or(|(_, current)| strictness(value) < strictness(current)) {
            best = Some((source, value));
        }
    }
    let Some((winner_source, winner)) = best else {
        return Sourced {
            value: Severity::High,
            source: Source::Default,
        };
    };
    for (source, value) in entries() {
        if source.is_override() && strictness(value) > strictness(winner) {
            warnings.push(format!(
                "nvm: {source} {value} is looser than {winner} from {winner_source}; using {winner}."
            ));
        }
    }
    Sourced {
        value: winner,
        source: winner_source.clone(),
    }
}

/// The central policy decides the index; a repository policy only does when
/// there is no central one.
fn merge_index(layers: &[CveLayer]) -> Sourced<String> {
    let is_central = |layer: &&CveLayer| matches!(layer.source, Source::CentralPolicy(_));
    let decider = layers.iter().find(is_central).or_else(|| {
        layers
            .iter()
            .find(|layer| matches!(layer.source, Source::RepositoryPolicy(_)))
    });
    match decider.and_then(|layer| Some((layer.index.clone()?, layer.source.clone()))) {
        Some((value, source)) => Sourced { value, source },
        None => Sourced {
            value: DEFAULT_INDEX.into(),
            source: Source::Default,
        },
    }
}

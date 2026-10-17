//! Merging the rule sources: overrides can only tighten the central policy.

use std::fmt::Display;

use nodejs_semver::Range;

use super::document::{DocumentError, NodeRules};
use super::source::{Source, Sourced};
use crate::domain::version::{Version, VersionPattern};

/// The node rules one source contributes.
#[derive(Debug, Clone)]
pub struct NodeLayer {
    pub source: Source,
    pub minimum: Option<Version>,
    pub allow: Option<Range>,
    pub allow_text: Option<String>,
    pub deny: Vec<Version>,
    pub lts_only: Option<bool>,
    pub minimum_release_age_days: Option<u32>,
}

impl NodeLayer {
    /// # Errors
    /// Returns a [`DocumentError`] when a version or range is invalid.
    pub fn from_rules(source: Source, rules: &NodeRules) -> Result<Self, DocumentError> {
        Ok(Self {
            source,
            minimum: parse_minimum(rules.minimum.as_deref())?,
            allow: parse_allow(rules.allow.as_deref())?,
            allow_text: rules.allow.clone(),
            deny: parse_deny(rules.deny.as_deref().unwrap_or_default())?,
            lts_only: rules.lts_only,
            minimum_release_age_days: rules.minimum_release_age_days,
        })
    }

    #[must_use]
    pub fn minimum_only(source: Source, minimum: Version) -> Self {
        Self {
            source,
            minimum: Some(minimum),
            allow: None,
            allow_text: None,
            deny: Vec::new(),
            lts_only: None,
            minimum_release_age_days: None,
        }
    }
}

fn parse_minimum(text: Option<&str>) -> Result<Option<Version>, DocumentError> {
    text.map(|text| {
        text.parse::<VersionPattern>()
            .map(|pattern| pattern.lowest())
            .map_err(|_| invalid_version("tools.node.minimum".into(), text))
    })
    .transpose()
}

fn parse_allow(text: Option<&str>) -> Result<Option<Range>, DocumentError> {
    text.map(|text| {
        Range::parse(text).map_err(|_| DocumentError::InvalidRange {
            field: "tools.node.allow".into(),
            value: text.to_owned(),
        })
    })
    .transpose()
}

fn parse_deny(entries: &[String]) -> Result<Vec<Version>, DocumentError> {
    entries
        .iter()
        .enumerate()
        .map(|(index, text)| {
            text.parse::<Version>()
                .map_err(|_| invalid_version(format!("tools.node.deny[{index}]"), text))
        })
        .collect()
}

fn invalid_version(field: String, value: &str) -> DocumentError {
    DocumentError::InvalidVersion {
        field,
        value: value.to_owned(),
    }
}

/// The merged node rules, each with the source that decided it.
#[derive(Debug, Clone, Default)]
pub struct NodePolicy {
    pub minimum: Option<Sourced<Version>>,
    pub allow: Vec<Sourced<(Range, String)>>,
    pub deny: Vec<Sourced<Version>>,
    pub lts_only: Option<Source>,
    pub minimum_release_age_days: Option<Sourced<u32>>,
}

/// Merges the layers into one policy and the warnings for overrides that were
/// looser than the winner.
#[must_use]
pub fn merge(layers: Vec<NodeLayer>) -> (NodePolicy, Vec<String>) {
    let mut warnings = Vec::new();
    let minimum = tighten(
        layers.iter().filter_map(|l| Some((&l.source, l.minimum?))),
        &mut warnings,
    );
    let minimum_release_age_days = tighten(
        layers
            .iter()
            .filter_map(|l| Some((&l.source, l.minimum_release_age_days?))),
        &mut warnings,
    );
    let lts_only = merge_lts_only(&layers, &mut warnings);
    let policy = NodePolicy {
        minimum,
        allow: collect_allow(&layers),
        deny: collect_deny(&layers),
        lts_only,
        minimum_release_age_days,
    };
    (policy, warnings)
}

/// The largest value wins (the first on a tie); losing overrides warn.
fn tighten<'a, T: Ord + Copy + Display>(
    entries: impl Iterator<Item = (&'a Source, T)> + Clone,
    warnings: &mut Vec<String>,
) -> Option<Sourced<T>> {
    let mut best: Option<(&Source, T)> = None;
    for (source, value) in entries.clone() {
        if best.is_none_or(|(_, current)| value > current) {
            best = Some((source, value));
        }
    }
    let (winner_source, winner) = best?;
    for (source, value) in entries {
        if source.is_override() && value < winner {
            warnings.push(loosening(source, value, winner, winner_source));
        }
    }
    Some(Sourced {
        value: winner,
        source: winner_source.clone(),
    })
}

fn merge_lts_only(layers: &[NodeLayer], warnings: &mut Vec<String>) -> Option<Source> {
    let winner = layers
        .iter()
        .find(|layer| layer.lts_only == Some(true))
        .map(|layer| layer.source.clone())?;
    for layer in layers {
        if layer.source.is_override() && layer.lts_only == Some(false) {
            warnings.push(loosening(&layer.source, false, true, &winner));
        }
    }
    Some(winner)
}

fn collect_allow(layers: &[NodeLayer]) -> Vec<Sourced<(Range, String)>> {
    layers
        .iter()
        .filter_map(|layer| {
            let range = layer.allow.clone()?;
            let text = layer.allow_text.clone().unwrap_or_default();
            Some(Sourced {
                value: (range, text),
                source: layer.source.clone(),
            })
        })
        .collect()
}

fn collect_deny(layers: &[NodeLayer]) -> Vec<Sourced<Version>> {
    layers
        .iter()
        .flat_map(|layer| {
            layer.deny.iter().map(|version| Sourced {
                value: *version,
                source: layer.source.clone(),
            })
        })
        .collect()
}

fn loosening(source: &Source, value: impl Display, winner: impl Display, from: &Source) -> String {
    format!("nvm: {source} {value} is looser than {winner} from {from}; using {winner}.")
}

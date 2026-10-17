//! Merging the rule sources: overrides can only tighten the central policy.

use std::fmt::Display;

use nodejs_semver::Range;

use super::document::{
    DocumentError, NodeRules, PackageRules, parse_exact, parse_minimum, parse_range,
};
use super::source::{Source, Sourced};
use crate::domain::tool::Tool;
use crate::domain::version::Version;

/// The rules of one tool that one source contributes.
#[derive(Debug, Clone)]
pub struct ToolLayer {
    pub source: Source,
    pub minimum: Option<Version>,
    pub allow: Option<Range>,
    pub allow_text: Option<String>,
    pub deny: Vec<Version>,
    pub lts_only: Option<bool>,
    pub minimum_release_age_days: Option<u32>,
}

/// The fields every tool's rules share, borrowed.
struct Fields<'a> {
    minimum: Option<&'a str>,
    allow: Option<&'a str>,
    deny: &'a [String],
    minimum_release_age_days: Option<u32>,
}

impl ToolLayer {
    /// # Errors
    /// Returns a [`DocumentError`] when a version or range is invalid.
    pub fn from_node(source: Source, rules: &NodeRules) -> Result<Self, DocumentError> {
        let fields = Fields {
            minimum: rules.minimum.as_deref(),
            allow: rules.allow.as_deref(),
            deny: rules.deny.as_deref().unwrap_or_default(),
            minimum_release_age_days: rules.minimum_release_age_days,
        };
        Self::from_fields(Tool::Node, source, &fields, rules.lts_only)
    }

    /// # Errors
    /// Returns a [`DocumentError`] when a version or range is invalid.
    pub fn from_package(
        tool: Tool,
        source: Source,
        rules: &PackageRules,
    ) -> Result<Self, DocumentError> {
        let fields = Fields {
            minimum: rules.minimum.as_deref(),
            allow: rules.allow.as_deref(),
            deny: rules.deny.as_deref().unwrap_or_default(),
            minimum_release_age_days: rules.minimum_release_age_days,
        };
        Self::from_fields(tool, source, &fields, None)
    }

    fn from_fields(
        tool: Tool,
        source: Source,
        fields: &Fields<'_>,
        lts_only: Option<bool>,
    ) -> Result<Self, DocumentError> {
        let field = |name: &str| format!("tools.{}.{name}", tool.name());
        Ok(Self {
            source,
            minimum: fields
                .minimum
                .map(|text| parse_minimum(field("minimum"), text))
                .transpose()?,
            allow: fields
                .allow
                .map(|text| parse_range(field("allow"), text))
                .transpose()?,
            allow_text: fields.allow.map(str::to_owned),
            deny: parse_deny(&field("deny"), fields.deny)?,
            lts_only,
            minimum_release_age_days: fields.minimum_release_age_days,
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

fn parse_deny(field: &str, entries: &[String]) -> Result<Vec<Version>, DocumentError> {
    entries
        .iter()
        .enumerate()
        .map(|(index, text)| parse_exact(format!("{field}[{index}]"), text))
        .collect()
}

/// The merged rules of one tool, each with the source that decided it.
#[derive(Debug, Clone)]
pub struct ToolPolicy {
    pub tool: Tool,
    pub minimum: Option<Sourced<Version>>,
    pub allow: Vec<Sourced<(Range, String)>>,
    pub deny: Vec<Sourced<Version>>,
    pub lts_only: Option<Source>,
    pub minimum_release_age_days: Option<Sourced<u32>>,
}

impl ToolPolicy {
    /// A policy with no rule at all.
    #[must_use]
    pub fn unrestricted(tool: Tool) -> Self {
        Self {
            tool,
            minimum: None,
            allow: Vec::new(),
            deny: Vec::new(),
            lts_only: None,
            minimum_release_age_days: None,
        }
    }

    /// Whether no rule is set.
    #[must_use]
    pub fn is_unrestricted(&self) -> bool {
        self.minimum.is_none()
            && self.allow.is_empty()
            && self.deny.is_empty()
            && self.lts_only.is_none()
            && self.minimum_release_age_days.is_none()
    }
}

/// Merges the layers of `tool` into one policy and the warnings for overrides
/// that were looser than the winner.
#[must_use]
pub fn merge(tool: Tool, layers: Vec<ToolLayer>) -> (ToolPolicy, Vec<String>) {
    let mut warnings = Vec::new();
    let minimum = tighten(
        layers.iter().filter_map(|l| Some((&l.source, l.minimum?))),
        &|version| tool.show(&version),
        &mut warnings,
    );
    let minimum_release_age_days = tighten(
        layers
            .iter()
            .filter_map(|l| Some((&l.source, l.minimum_release_age_days?))),
        &|days| days.to_string(),
        &mut warnings,
    );
    let lts_only = merge_lts_only(&layers, &mut warnings);
    let policy = ToolPolicy {
        tool,
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
    show: &dyn Fn(T) -> String,
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
            warnings.push(loosening(source, show(value), show(winner), winner_source));
        }
    }
    Some(Sourced {
        value: winner,
        source: winner_source.clone(),
    })
}

fn merge_lts_only(layers: &[ToolLayer], warnings: &mut Vec<String>) -> Option<Source> {
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

fn collect_allow(layers: &[ToolLayer]) -> Vec<Sourced<(Range, String)>> {
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

fn collect_deny(layers: &[ToolLayer]) -> Vec<Sourced<Version>> {
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

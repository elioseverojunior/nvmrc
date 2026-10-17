//! The npm, yarn and pnpm layers of the policy sources: documents, flags and
//! variables.

use super::mode::minimum_layer;
use crate::context::Context;
use crate::domain::policy::{
    CveLayer, DocumentError, PolicyDocument, Source, ToolLayer, ToolPolicy, merge,
};
use crate::domain::tool::Tool;
use crate::error::PolicyError;

/// What one policy document contributes.
#[derive(Default)]
pub(super) struct DocumentLayers {
    pub node: Option<ToolLayer>,
    pub packages: Vec<(Tool, ToolLayer)>,
    pub cve: Option<CveLayer>,
}

impl DocumentLayers {
    fn take_package(&mut self, tool: Tool) -> Option<ToolLayer> {
        let index = self.packages.iter().position(|(owner, _)| *owner == tool)?;
        Some(self.packages.remove(index).1)
    }
}

/// The flag and the variable that set the minimum of a package manager.
fn minimum_names(tool: Tool) -> (&'static str, &'static str) {
    match tool {
        Tool::Yarn => ("--min-yarn", "NVMRC_MIN_YARN"),
        Tool::Pnpm => ("--min-pnpm", "NVMRC_MIN_PNPM"),
        _ => ("--min-npm", "NVMRC_MIN_NPM"),
    }
}

/// The layers a document holds for npm, yarn and pnpm.
///
/// # Errors
/// A [`DocumentError`] when a version or range in the rules is invalid.
pub(super) fn package_layers(
    document: &PolicyDocument,
    source: &Source,
) -> Result<Vec<(Tool, ToolLayer)>, DocumentError> {
    let rules = [
        (Tool::Npm, &document.tools.npm),
        (Tool::Yarn, &document.tools.yarn),
        (Tool::Pnpm, &document.tools.pnpm),
    ];
    let mut layers = Vec::new();
    for (tool, rules) in rules {
        if let Some(rules) = rules {
            layers.push((tool, ToolLayer::from_package(tool, source.clone(), rules)?));
        }
    }
    Ok(layers)
}

/// Merges one package manager's rules: central policy, repository policy,
/// its minimum flag and its variable, in that order, so a tie goes to the
/// central policy.
///
/// # Errors
/// A [`PolicyError::Invalid`] naming a flag or variable that is not a version.
pub(super) fn package_policy(
    context: &Context<'_>,
    tool: Tool,
    central: &mut DocumentLayers,
    repository: &mut DocumentLayers,
) -> Result<(ToolPolicy, Vec<String>), PolicyError> {
    let (flag, variable) = minimum_names(tool);
    let mut layers = Vec::new();
    layers.extend(central.take_package(tool));
    layers.extend(repository.take_package(tool));
    let flagged = context.policy_flags().get(flag).map(str::to_owned);
    layers.extend(minimum_layer(Source::Flag(flag), flagged)?);
    layers.extend(minimum_layer(
        Source::Variable(variable),
        context.env.var(variable),
    )?);
    Ok(merge(tool, layers))
}

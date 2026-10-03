//! Digest 6.2: the version `nvm exec` runs, from `--lts`, the first word
//! (when it is a version), the `.nvmrc`, or the active version.

use super::fallback_warning;
use super::options::Options;
use crate::commands::rc_version::{RcVersion, rc_version};
use crate::commands::transcript::Transcript;
use crate::commands::use_version::target::lookup::{Resolution, nvm_version};
use crate::context::Context;
use crate::domain::valid_version::is_valid_version;
use crate::error::CliError;

/// What was asked for, the `VERSION` nvm.sh hands `nvm-exec`, and the
/// command left to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Selection {
    /// What `nvm_ensure_version_installed` checks.
    pub provided: String,
    /// `NODE_VERSION`: resolved, or `lts/<name>` as given.
    pub version: String,
    pub command: Vec<String>,
}

pub(super) fn select(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Result<Selection, CliError> {
    let command = options.rest.clone();
    if let Some(lts) = &options.lts {
        let provided = format!("lts/{lts}");
        return Ok(Selection {
            version: provided.clone(),
            provided,
            command,
        });
    }
    if let Some(selection) = from_first_word(context, options, transcript)? {
        return Ok(selection);
    }
    if !options.silent {
        fallback_warning("exec")
            .into_iter()
            .for_each(|line| transcript.err(line));
    }
    let resolution = nvm_version(context, "current")?;
    Ok(selection("current".to_owned(), &resolution, command))
}

/// The first word when it resolves or looks like a version; otherwise it
/// is the command, and the `.nvmrc` names the version, if there is one.
fn from_first_word(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Result<Option<Selection>, CliError> {
    let Some((first, rest)) = options.rest.split_first() else {
        return Ok(None);
    };
    let resolution = nvm_version(context, first)?;
    if resolution != Resolution::NotAvailable || is_valid_version(first) {
        return Ok(Some(selection(first.clone(), &resolution, rest.to_vec())));
    }
    let RcVersion::Found { version, .. } = rc_version(context, options.silent, transcript) else {
        return Ok(None);
    };
    let resolution = nvm_version(context, &version)?;
    Ok(Some(selection(version, &resolution, options.rest.clone())))
}

fn selection(provided: String, resolution: &Resolution, command: Vec<String>) -> Selection {
    Selection {
        provided,
        version: resolution.text(),
        command,
    }
}

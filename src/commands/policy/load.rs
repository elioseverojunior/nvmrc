//! Where policy text comes from: an https URL, a file, or the repository.

use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::domain::nvmrc::find_up;
use crate::domain::policy::POLICY_FLAGS;
use crate::error::PolicyError;

pub const REPOSITORY_POLICY_FILE: &str = ".nvmrc-policy.json";

const CENTRAL_FLAG: &str = "--policy";
const CENTRAL_VARIABLE: &str = "NVMRC_POLICY";

/// The text at `location`: an https URL, or a path (relative to the working
/// directory when it is not absolute). Any other scheme is refused, as a
/// policy read over a channel that can be altered is not a policy.
///
/// # Errors
/// [`PolicyError::InsecureUrl`] for a URL that is not https;
/// [`PolicyError::Unreadable`] when the text cannot be fetched or read.
pub fn read_location(context: &Context<'_>, location: &str) -> Result<String, PolicyError> {
    if location.starts_with("https://") {
        // Never follows a redirect away from https.
        return context
            .policy_http()
            .get_secure_text(location)
            .map_err(|error| unreadable(location, error));
    }
    if location.contains("://") {
        return Err(PolicyError::InsecureUrl(location.to_owned()));
    }
    let path = absolute(context, location);
    context
        .fs
        .read_to_string(&path)
        .map_err(|error| unreadable(location, error))
}

fn unreadable(location: &str, reason: impl std::fmt::Display) -> PolicyError {
    PolicyError::Unreadable {
        location: location.to_owned(),
        reason: reason.to_string(),
    }
}

pub(super) fn absolute(context: &Context<'_>, location: &str) -> PathBuf {
    let path = Path::new(location);
    match context.working_directory() {
        Some(directory) if path.is_relative() => directory.join(path),
        _ => path.to_path_buf(),
    }
}

/// Where the central policy lives: `--policy`, else a non-empty
/// `NVMRC_POLICY`.
pub(super) fn central_location(context: &Context<'_>) -> Option<String> {
    context
        .policy_flags()
        .get(CENTRAL_FLAG)
        .map(str::to_owned)
        .or_else(|| context.env.var(CENTRAL_VARIABLE))
        .filter(|location| !location.is_empty())
}

/// The nearest repository policy file above the working directory.
pub(super) fn repository_policy(context: &Context<'_>) -> Option<PathBuf> {
    find_up(
        context.fs,
        &context.working_directory()?,
        REPOSITORY_POLICY_FILE,
    )
}

/// Whether anything asks for the policy: a policy flag, a policy variable of
/// the `NVMRC_` family, a central policy, or a repository policy file.
#[must_use]
pub fn policy_mode_requested(context: &Context<'_>) -> bool {
    !context.policy_flags().is_empty()
        || policy_variable_set(context)
        || central_location(context).is_some()
        || repository_policy(context).is_some()
}

/// Whether an `NVMRC_` policy variable is set and not blank; `NVM_MIN_VERSION`
/// is nvm's own and does not ask for the policy.
fn policy_variable_set(context: &Context<'_>) -> bool {
    POLICY_FLAGS
        .iter()
        .filter(|flag| flag.variable.starts_with("NVMRC_"))
        .filter_map(|flag| context.env.var(flag.variable))
        .any(|value| !value.trim().is_empty())
}

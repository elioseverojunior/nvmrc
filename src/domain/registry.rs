//! What the npm registry says about a package manager: where it lives, what
//! its manifest declares and when a version was published.

use std::path::{Component, Path};

use serde_json::Value;
use thiserror::Error;

use crate::domain::nvmrc::pins::exact_version;
use crate::domain::timestamp::days_from_iso_date;
use crate::domain::tool::Tool;
use crate::domain::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error("not valid JSON: {0}")]
    Json(String),
    #[error("no `{0}` in the document")]
    Missing(&'static str),
    #[error("`bin` must list at least one command")]
    NoCommands,
    #[error("the command name `{0}` is not usable")]
    BadCommand(String),
    #[error("the command `{command}` points outside the package: `{target}`")]
    BadTarget { command: String, target: String },
    #[error("`{0}` is not a date")]
    BadDate(String),
}

/// What an install needs of a version's manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub tarball: String,
    pub integrity: String,
    /// `(command, path inside the package)`, in the manifest's order.
    pub commands: Vec<(String, String)>,
}

/// The registry package that ships `tool`: yarn 2 and later is
/// `@yarnpkg/cli-dist`.
#[must_use]
pub fn package_name(tool: Tool, version: &Version) -> &'static str {
    match tool {
        Tool::Yarn if version.major >= 2 => "@yarnpkg/cli-dist",
        Tool::Yarn => "yarn",
        Tool::Pnpm => "pnpm",
        Tool::Npm | Tool::Node => "npm",
    }
}

fn encoded_name(tool: Tool, version: &Version) -> String {
    package_name(tool, version).replace('/', "%2f")
}

#[must_use]
pub fn manifest_url(registry: &str, tool: Tool, version: &Version) -> String {
    let name = encoded_name(tool, version);
    format!("{registry}/{name}/{}", tool.show(version))
}

#[must_use]
pub fn packument_url(registry: &str, tool: Tool, version: &Version) -> String {
    format!("{registry}/{}", encoded_name(tool, version))
}

fn parse(text: &str) -> Result<Value, RegistryError> {
    serde_json::from_str(text).map_err(|error| RegistryError::Json(error.to_string()))
}

fn dist_text(document: &Value, key: &'static str) -> Result<String, RegistryError> {
    let text = document["dist"][key].as_str();
    text.map(str::to_owned).ok_or(RegistryError::Missing(key))
}

fn stays_inside(target: &str) -> bool {
    let mut components = Path::new(target).components().peekable();
    components.peek().is_some()
        && components.all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

/// One normal path component and nothing else, on every platform.
fn is_plain_file_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    let single = matches!(components.next(), Some(Component::Normal(part)) if part == name);
    single && components.next().is_none() && !name.contains(['\\', '\0'])
}

fn command(name: &str, target: &Value) -> Result<(String, String), RegistryError> {
    if !is_plain_file_name(name) {
        return Err(RegistryError::BadCommand(name.to_owned()));
    }
    let target = target.as_str().unwrap_or_default();
    if !stays_inside(target) {
        return Err(RegistryError::BadTarget {
            command: name.to_owned(),
            target: target.to_owned(),
        });
    }
    Ok((name.to_owned(), target.to_owned()))
}

/// # Errors
/// When the text is not JSON, `dist.tarball` or `dist.integrity` is missing,
/// `bin` is not a non-empty object, a command name is empty or has a `/`, or a
/// target is absolute or climbs out of the package.
pub fn parse_manifest(text: &str) -> Result<Manifest, RegistryError> {
    let document = parse(text)?;
    let bin = document["bin"]
        .as_object()
        .filter(|bin| !bin.is_empty())
        .ok_or(RegistryError::NoCommands)?;
    let commands = bin
        .iter()
        .map(|(name, target)| command(name, target))
        .collect::<Result<_, _>>()?;
    Ok(Manifest {
        tarball: dist_text(&document, "tarball")?,
        integrity: dist_text(&document, "integrity")?,
        commands,
    })
}

/// The day `version` was published, in days since 1970-01-01, from a
/// packument's `time` table; `None` when the table does not list it.
///
/// # Errors
/// When the text is not JSON or the entry is not an ISO date.
pub fn release_day(packument: &str, version: &Version) -> Result<Option<i64>, RegistryError> {
    let key = format!("{}.{}.{}", version.major, version.minor, version.patch);
    day_of(&parse(packument)?, &key)
}

fn day_of(document: &Value, key: &str) -> Result<Option<i64>, RegistryError> {
    let Some(stamp) = document["time"][key].as_str() else {
        return Ok(None);
    };
    stamp
        .get(..10)
        .and_then(days_from_iso_date)
        .map(Some)
        .ok_or_else(|| RegistryError::BadDate(stamp.to_owned()))
}

/// Every published `major.minor.patch` of a packument (prereleases and
/// `dist-tags` are not listed), oldest first, with the day it was published.
///
/// # Errors
/// When the text is not JSON, has no `versions` object or a `time` entry of a
/// listed version is not an ISO date.
pub fn published_versions(packument: &str) -> Result<Vec<(Version, Option<i64>)>, RegistryError> {
    let document = parse(packument)?;
    let listed = document["versions"]
        .as_object()
        .ok_or(RegistryError::Missing("versions"))?;
    let mut found = Vec::new();
    for key in listed.keys() {
        if let Some(version) = exact_version(key) {
            found.push((version, day_of(&document, key)?));
        }
    }
    found.sort_by_key(|(version, _)| *version);
    Ok(found)
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;

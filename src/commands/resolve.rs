//! Shared by the commands that take a version or alias: resolve the name
//! through the alias files, then match it against the installed versions.

use crate::context::Context;
use crate::domain::alias;
use crate::domain::version::{Flavor, Version, VersionPattern};
use crate::error::CliError;

#[derive(Debug, PartialEq, Eq)]
pub enum Resolved {
    Installed(Version),
    /// No installed version matches. `resolved` is where the alias chain
    /// ended, which is the name itself when it is not an alias.
    Missing {
        resolved: String,
    },
}

/// # Errors
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn resolve_installed(context: &Context<'_>, name: &str) -> Result<Resolved, CliError> {
    let resolved = alias::resolve(&context.alias_store()?, name)?;
    let installed = context.installed_versions()?;
    let found = find_installed(&resolved, &installed);
    Ok(found.map_or(Resolved::Missing { resolved }, Resolved::Installed))
}

/// `node` and `iojs` are built-in aliases for the latest installed version of
/// that flavor; anything else is matched as a version pattern.
fn find_installed(resolved: &str, installed: &[Version]) -> Option<Version> {
    match resolved {
        "node" => latest_of(installed, Flavor::Node),
        "iojs" => latest_of(installed, Flavor::IoJs),
        other => other
            .parse::<VersionPattern>()
            .ok()
            .and_then(|pattern| pattern.highest_match(installed).copied()),
    }
}

fn latest_of(installed: &[Version], flavor: Flavor) -> Option<Version> {
    installed
        .iter()
        .filter(|version| version.flavor == flavor)
        .max()
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AliasError;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn resolve_with(fs: &FakeFileSystem, name: &str) -> Result<Resolved, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        resolve_installed(&Context { fs, env: &env }, name)
    }

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/v20.10.0/bin/node", "")
            .with_file("/n/versions/node/v18.9.0/bin/node", "")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("valid version")
    }

    #[test]
    fn a_pattern_resolves_to_the_highest_installed_match() {
        let resolved = resolve_with(&installed(), "20").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
    }

    #[test]
    fn node_is_the_latest_installed_node_ignoring_iojs() {
        let fs = installed().with_file("/n/versions/io.js/v3.0.0/bin/node", "");
        let resolved = resolve_with(&fs, "node").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
    }

    #[test]
    fn iojs_is_the_latest_installed_iojs() {
        let fs = installed()
            .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
            .with_file("/n/versions/io.js/v2.5.0/bin/node", "");
        let resolved = resolve_with(&fs, "iojs").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("iojs-v3.0.0")));
    }

    #[test]
    fn a_built_in_alias_can_be_the_target_of_another_alias() {
        let fs = installed().with_file("/n/alias/default", "node");
        let resolved = resolve_with(&fs, "default").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
    }

    #[test]
    fn a_built_in_alias_without_a_matching_install_is_missing() {
        let fs = FakeFileSystem::default().with_dir("/n/versions/node");
        let resolved = resolve_with(&fs, "node").unwrap();
        assert_eq!(
            resolved,
            Resolved::Missing {
                resolved: "node".into()
            }
        );
    }

    #[test]
    fn an_alias_resolves_through_its_chain() {
        let fs = installed().with_file("/n/alias/default", "v18");
        let resolved = resolve_with(&fs, "default").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("v18.9.0")));
    }

    #[test]
    fn a_missing_version_reports_where_the_chain_ended() {
        let fs = installed().with_file("/n/alias/old", "v16");
        let resolved = resolve_with(&fs, "old").unwrap();
        assert_eq!(
            resolved,
            Resolved::Missing {
                resolved: "v16".into()
            }
        );
    }

    #[test]
    fn a_name_that_is_not_a_version_is_missing_not_an_error() {
        let resolved = resolve_with(&installed(), "foo").unwrap();
        assert_eq!(
            resolved,
            Resolved::Missing {
                resolved: "foo".into()
            }
        );
    }

    #[test]
    fn an_alias_loop_is_an_error() {
        let fs = installed()
            .with_file("/n/alias/a", "b")
            .with_file("/n/alias/b", "a");
        let error = resolve_with(&fs, "a").unwrap_err();
        assert!(matches!(error, CliError::Alias(AliasError::Loop(_))));
    }
}

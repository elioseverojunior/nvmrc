//! Shared by the commands that take a version or alias: resolve the name
//! through the alias files, then match it against the installed versions.

use crate::context::Context;
use crate::domain::alias;
use crate::domain::version::{Version, VersionPattern};
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
    let found = resolved
        .parse::<VersionPattern>()
        .ok()
        .and_then(|pattern| pattern.highest_match(&installed).copied());
    Ok(found.map_or(Resolved::Missing { resolved }, Resolved::Installed))
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

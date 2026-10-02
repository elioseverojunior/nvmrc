//! `nvm version <pattern>`: the highest installed version matching a pattern.

use crate::adapters::fs_alias_store::FsAliasStore;
use crate::context::Context;
use crate::domain::alias;
use crate::domain::version::VersionPattern;
use crate::error::CliError;

/// # Errors
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::Version`] when the resolved name is not a version pattern.
/// - [`CliError::NotInstalled`] when no installed version matches.
pub fn run(context: &Context<'_>, name: &str) -> Result<String, CliError> {
    let store = FsAliasStore::new(context.fs, &context.nvm_dir());
    let resolved = alias::resolve(&store, name)?;
    let pattern: VersionPattern = resolved.parse()?;
    let installed = context.installed_versions();
    pattern
        .highest_match(&installed)
        .map(ToString::to_string)
        .ok_or(CliError::NotInstalled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn run_with(fs: &FakeFileSystem, name: &str) -> Result<String, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        run(&Context { fs, env: &env }, name)
    }

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/v20.10.0/bin/node", "")
            .with_file("/n/versions/node/v18.9.0/bin/node", "")
    }

    #[test]
    fn resolves_a_major_to_the_highest_installed_version() {
        assert_eq!(run_with(&installed(), "20").unwrap(), "v20.10.0");
    }

    #[test]
    fn resolves_through_an_alias() {
        let fs = installed().with_file("/n/alias/default", "v18");
        assert_eq!(run_with(&fs, "default").unwrap(), "v18.9.0");
    }

    #[test]
    fn a_missing_version_is_not_installed_with_exit_code_3() {
        let error = run_with(&installed(), "16").unwrap_err();
        assert_eq!(error.to_string(), "N/A");
        assert_eq!(error.exit_code(), NvmExitCode::InvalidVersion);
    }

    #[test]
    fn an_alias_loop_has_exit_code_8() {
        let fs = installed()
            .with_file("/n/alias/a", "b")
            .with_file("/n/alias/b", "a");
        let error = run_with(&fs, "a").unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::AliasLoop);
    }
}

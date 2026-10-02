//! `nvm version <pattern>`: the highest installed version matching a pattern.

use crate::commands::Output;
use crate::commands::resolve::{Resolved, resolve_installed};
use crate::context::Context;
use crate::error::CliError;

/// # Errors
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::NotInstalled`] when no installed version matches, or the
///   resolved name is not a version pattern (as in `nvm.sh`, which prints `N/A`).
pub fn run(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
    match resolve_installed(context, name)? {
        Resolved::Installed(version) => Ok(Output::stdout(version.to_string())),
        Resolved::Missing { .. } => Err(CliError::NotInstalled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn run_with(fs: &FakeFileSystem, name: &str) -> Result<Output, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        run(&Context { fs, env: &env }, name)
    }

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/v20.10.0/bin/node", "")
    }

    #[test]
    fn prints_the_highest_installed_version() {
        let output = run_with(&installed(), "20").unwrap();
        assert_eq!(output, Output::stdout("v20.10.0"));
    }

    #[test]
    fn a_missing_version_is_not_installed_with_exit_code_3() {
        let error = run_with(&installed(), "16").unwrap_err();
        assert_eq!(error.to_string(), "N/A");
        assert_eq!(error.exit_code(), NvmExitCode::InvalidVersion);
    }

    #[test]
    fn a_name_that_is_not_a_version_is_not_installed_too() {
        let error = run_with(&installed(), "foo").unwrap_err();
        assert!(matches!(error, CliError::NotInstalled));
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

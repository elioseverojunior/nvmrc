//! `nvm version <pattern>`: the highest installed version matching a pattern.

use crate::commands::resolve::{Resolved, resolve_installed};
use crate::commands::{Output, current};
use crate::context::Context;
use crate::error::CliError;

/// # Errors
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::NotInstalled`] when no installed version matches, or the
///   resolved name is not a version pattern (as in `nvm.sh`, which prints `N/A`).
pub fn run(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
    if name == "current" {
        return current::run(context);
    }
    match resolve_installed(context, name)? {
        Resolved::Installed(version) => Ok(Output::stdout(version.to_string())),
        Resolved::System => Ok(Output::stdout("system")),
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
    fn current_is_the_version_of_the_active_node() {
        let fs = installed().with_file("/usr/bin/node", "");
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", "/n/versions/node/v20.1.0/bin:/usr/bin");
        let output = run(&Context { fs: &fs, env: &env }, "current").unwrap();
        assert_eq!(output, Output::stdout("v20.1.0"));
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

    #[test]
    fn an_alias_to_system_prints_system() {
        let fs = installed()
            .with_file("/usr/bin/node", "")
            .with_file("/n/alias/default", "system");
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", "/usr/bin");
        let output = run(&Context { fs: &fs, env: &env }, "default").unwrap();
        assert_eq!(output, Output::stdout("system"));
    }

    #[test]
    fn an_alias_to_system_without_a_system_node_is_not_installed() {
        let fs = installed().with_file("/n/alias/default", "system");
        let error = run_with(&fs, "default").unwrap_err();
        assert!(matches!(error, CliError::NotInstalled));
    }
}

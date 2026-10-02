//! `nvm current`: the version of the `node` that is first on `PATH`.

use crate::commands::Output;
use crate::context::Context;
use crate::domain::current::{Current, classify};
use crate::domain::path_search::find_in_path;
use crate::error::CliError;

/// The classification of the `node` that is first on `PATH`.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn detect(context: &Context<'_>) -> Result<Current, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let path_variable = context.env.var_os("PATH").unwrap_or_default();
    Ok(find_in_path(context.fs, &path_variable, "node")
        .map_or(Current::None, |node| classify(&node, &nvm_dir)))
}

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>) -> Result<Output, CliError> {
    Ok(Output::stdout(detect(context)?.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn current_with(fs: &FakeFileSystem, path: &str) -> String {
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", path);
        run(&Context { fs, env: &env }).unwrap().stdout
    }

    #[test]
    fn reports_the_nvm_version_first_on_path() {
        let fs = FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/usr/bin/node", "");
        let path = "/n/versions/node/v20.1.0/bin:/usr/bin";
        assert_eq!(current_with(&fs, path), "v20.1.0");
    }

    #[test]
    fn reports_system_when_the_first_node_is_outside_nvm_dir() {
        let fs = FakeFileSystem::default().with_file("/usr/bin/node", "");
        assert_eq!(current_with(&fs, "/usr/bin"), "system");
    }

    #[test]
    fn reports_none_without_any_node() {
        assert_eq!(current_with(&FakeFileSystem::default(), "/usr/bin"), "none");
    }

    #[test]
    fn reports_none_when_path_is_unset() {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let fs = FakeFileSystem::default();
        let output = run(&Context { fs: &fs, env: &env }).unwrap();
        assert_eq!(output.stdout, "none");
    }
}

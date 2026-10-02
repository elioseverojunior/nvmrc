//! `nvm which <version>`: the path to the `node` binary of a version.

use std::path::PathBuf;

use crate::commands::Output;
use crate::commands::current;
use crate::commands::resolve::{Resolved, resolve_installed};
use crate::context::Context;
use crate::domain::path_search::find_in_dirs;
use crate::domain::version::Version;
use crate::error::CliError;

const USAGE: &str = "Usage: nvm which [current | <version>]\n  \
Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] when no version is given.
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::VersionNotInstalled`] when the version is not installed.
/// - [`CliError::SystemNodeNotFound`] for `system` without a system node.
pub fn run(context: &Context<'_>, name: Option<&str>) -> Result<Output, CliError> {
    let name = name.ok_or_else(|| CliError::Usage(USAGE.to_owned()))?;
    let name = if name == "current" {
        current::detect(context)?.to_string()
    } else {
        name.to_owned()
    };
    if name == "system" {
        return system_node(context);
    }
    match resolve_installed(context, &name)? {
        Resolved::Installed(version) => {
            let binary = node_binary(context, &version)?;
            Ok(Output::stdout(binary.to_string_lossy()))
        }
        Resolved::Missing { resolved } => Err(CliError::VersionNotInstalled(
            not_installed_message(&name, &resolved),
        )),
    }
}

fn node_binary(context: &Context<'_>, version: &Version) -> Result<PathBuf, CliError> {
    Ok(context
        .nvm_dir()?
        .join("versions")
        .join(version.flavor.versions_directory())
        .join(version.directory_name())
        .join("bin")
        .join("node"))
}

/// The first `node` on `PATH` that does not live under `$NVM_DIR`.
fn system_node(context: &Context<'_>) -> Result<Output, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let path_variable = context.env.var_os("PATH").unwrap_or_default();
    let outside_nvm = std::env::split_paths(&path_variable)
        .filter(|directory| !directory.starts_with(&nvm_dir))
        .collect::<Vec<_>>();
    find_in_dirs(context.fs, outside_nvm, "node")
        .map(|node| Output::stdout(node.to_string_lossy()))
        .ok_or(CliError::SystemNodeNotFound)
}

/// `20` is shown as `v20`, like `nvm_ensure_version_prefix`.
fn with_v_prefix(name: &str) -> String {
    if name.starts_with(|first: char| first.is_ascii_digit()) {
        format!("v{name}")
    } else {
        name.to_owned()
    }
}

fn not_installed_message(name: &str, resolved: &str) -> String {
    let shown = if resolved == name {
        with_v_prefix(name)
    } else {
        format!("{name} -> {}", with_v_prefix(resolved))
    };
    format!(
        "N/A: version \"{shown}\" is not yet installed.\n\n\
         You need to run `nvm install {name}` to install and use it."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/v18.9.0/bin/node", "")
            .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
    }

    fn which_with(fs: &FakeFileSystem, path: &str, name: Option<&str>) -> Result<Output, CliError> {
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", path);
        run(&Context { fs, env: &env }, name)
    }

    fn which(name: &str) -> Result<Output, CliError> {
        which_with(&installed(), "/usr/bin", Some(name))
    }

    #[test]
    fn prints_the_node_binary_of_the_highest_match() {
        let output = which("20").unwrap();
        assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
    }

    #[test]
    fn follows_aliases() {
        let fs = installed().with_file("/n/alias/default", "v18");
        let output = which_with(&fs, "/usr/bin", Some("default")).unwrap();
        assert_eq!(output, Output::stdout("/n/versions/node/v18.9.0/bin/node"));
    }

    #[test]
    fn iojs_binaries_live_under_the_io_js_directory() {
        let output = which("iojs-v3.0.0").unwrap();
        assert_eq!(output, Output::stdout("/n/versions/io.js/v3.0.0/bin/node"));
    }

    #[test]
    fn current_means_the_active_version() {
        let path = "/n/versions/node/v18.9.0/bin";
        let output = which_with(&installed(), path, Some("current")).unwrap();
        assert_eq!(output, Output::stdout("/n/versions/node/v18.9.0/bin/node"));
    }

    #[test]
    fn a_missing_version_gets_the_install_hint_and_exit_1() {
        let error = which("16").unwrap_err();
        let expected = "N/A: version \"v16\" is not yet installed.\n\n\
                        You need to run `nvm install 16` to install and use it.";
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.exit_code(), NvmExitCode::Failure);
    }

    #[test]
    fn a_missing_alias_target_shows_the_chain() {
        let fs = installed().with_file("/n/alias/old", "16");
        let error = which_with(&fs, "/usr/bin", Some("old")).unwrap_err();
        let expected = "N/A: version \"old -> v16\" is not yet installed.\n\n\
                        You need to run `nvm install old` to install and use it.";
        assert_eq!(error.to_string(), expected);
    }

    #[test]
    fn an_alias_loop_has_exit_code_8_and_names_the_alias() {
        let fs = installed().with_file("/n/alias/loop", "loop");
        let error = which_with(&fs, "/usr/bin", Some("loop")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "The alias \"loop\" leads to an infinite loop. Aborting."
        );
        assert_eq!(error.exit_code(), NvmExitCode::AliasLoop);
    }

    #[test]
    fn system_is_the_first_node_outside_nvm_dir() {
        let fs = installed().with_file("/usr/bin/node", "");
        let path = "/n/versions/node/v20.1.0/bin:/usr/bin";
        let output = which_with(&fs, path, Some("system")).unwrap();
        assert_eq!(output, Output::stdout("/usr/bin/node"));
    }

    #[test]
    fn system_without_a_system_node_is_exit_127() {
        let path = "/n/versions/node/v20.1.0/bin";
        let error = which_with(&installed(), path, Some("system")).unwrap_err();
        assert_eq!(error.to_string(), "System version of node not found.");
        assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    }

    #[test]
    fn without_a_version_it_is_a_usage_error_with_exit_127() {
        let error = which_with(&installed(), "/usr/bin", None).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Usage: nvm which [current | <version>]")
        );
        assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    }
}

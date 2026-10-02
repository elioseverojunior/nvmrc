//! `nvm unalias <name>`: delete an alias file.

use crate::commands::Output;
use crate::context::Context;
use crate::domain::alias::{AliasStore, BUILTIN_ALIASES};
use crate::error::CliError;

const USAGE: &str = "Usage: nvm unalias <name>\n  Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] unless exactly one name is given.
/// - [`CliError::InvalidArgument`] for a name with a `/`, or a built-in alias.
/// - [`CliError::Io`] when the alias file cannot be removed.
///
/// Like `nvm.sh`, an alias that does not exist is only a warning on stderr.
pub fn run(context: &Context<'_>, names: &[String]) -> Result<Output, CliError> {
    let [name] = names else {
        return Err(CliError::Usage(USAGE.to_owned()));
    };
    if name.contains('/') {
        let message = "Aliases in subdirectories are not supported.";
        return Err(CliError::InvalidArgument(message.to_owned()));
    }
    let path = context.alias_dir()?.join(name);
    if !context.fs.is_file(&path) {
        return missing(name);
    }
    let original = context.alias_store()?.target(name).unwrap_or_default();
    context
        .fs
        .remove_file(&path)
        .map_err(|source| CliError::io(&path, source))?;
    Ok(Output::stdout(format!(
        "Deleted alias {name} - restore it with `nvm alias \"{name}\" \"{original}\"`"
    )))
}

fn missing(name: &str) -> Result<Output, CliError> {
    if BUILTIN_ALIASES.contains(&name) {
        let message = format!("{name} is a default (built-in) alias and cannot be deleted.");
        return Err(CliError::InvalidArgument(message));
    }
    Ok(Output::default().with_stderr(format!("Alias {name} doesn't exist!")))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn unalias(fs: &FakeFileSystem, names: &[&str]) -> Result<Output, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let names: Vec<String> = names.iter().map(ToString::to_string).collect();
        run(&Context { fs, env: &env }, &names)
    }

    #[test]
    fn removes_the_alias_file_and_says_how_to_restore_it() {
        let fs = FakeFileSystem::default().with_file("/n/alias/work", "v18.9.0\n");
        let output = unalias(&fs, &["work"]).unwrap();
        assert_eq!(
            output,
            Output::stdout("Deleted alias work - restore it with `nvm alias \"work\" \"v18.9.0\"`")
        );
        assert!(!fs.is_file(Path::new("/n/alias/work")));
    }

    #[test]
    fn a_missing_alias_is_a_warning_on_stderr_and_still_succeeds() {
        let output = unalias(&FakeFileSystem::default(), &["nope"]).unwrap();
        assert_eq!(
            output,
            Output::default().with_stderr("Alias nope doesn't exist!")
        );
    }

    #[test]
    fn built_in_aliases_cannot_be_deleted() {
        for name in ["stable", "unstable", "iojs", "node", "system"] {
            let error = unalias(&FakeFileSystem::default(), &[name]).unwrap_err();
            let expected = format!("{name} is a default (built-in) alias and cannot be deleted.");
            assert_eq!(error.to_string(), expected);
            assert_eq!(error.exit_code(), NvmExitCode::Failure);
        }
    }

    #[test]
    fn aliases_in_subdirectories_are_rejected() {
        let fs = FakeFileSystem::default().with_file("/n/alias/lts/iron", "v20");
        let error = unalias(&fs, &["lts/iron"]).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Aliases in subdirectories are not supported."
        );
        assert!(fs.is_file(Path::new("/n/alias/lts/iron")));
    }

    #[test]
    fn exactly_one_name_is_required() {
        for names in [&[][..], &["a", "b"][..]] {
            let error = unalias(&FakeFileSystem::default(), names).unwrap_err();
            assert!(error.to_string().starts_with("Usage: nvm unalias <name>"));
            assert_eq!(error.exit_code(), NvmExitCode::NotFound);
        }
    }
}

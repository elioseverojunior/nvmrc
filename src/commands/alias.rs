//! `nvm alias <name> <target>`: create an alias file.
//!
//! Listing aliases (`nvm alias` without a target) lives in
//! [`crate::commands::aliases`], which shares its formatting. Output is always
//! plain, as `nvm.sh` prints when stdout is not a terminal.

use crate::commands::resolve::{Shown, shown};
use crate::commands::{Output, unalias};
use crate::context::Context;
use crate::domain::alias_format::format_line;
use crate::error::CliError;

/// # Errors
/// - [`CliError::InvalidArgument`] for a name that is empty, `.`, `..`, or
///   contains `#` or `/`.
/// - [`CliError::Io`] when the alias directory cannot be created or the alias
///   file cannot be written.
/// - An empty target deletes the alias instead, with the errors of
///   [`unalias::run`].
///
/// A target that is not installed is only a warning on stderr.
pub fn run(context: &Context<'_>, name: &str, target: &str) -> Result<Output, CliError> {
    if target.is_empty() {
        return unalias::run(context, &[name.to_owned()]);
    }
    validate_name(name)?;
    let version = shown(context, target)?;
    let alias_dir = context.alias_dir()?;
    context
        .fs
        .create_dir_all(&alias_dir)
        .map_err(|source| CliError::io(&alias_dir, source))?;
    let alias_file = alias_dir.join(name);
    context
        .fs
        .write_file(&alias_file, &format!("{target}\n"))
        .map_err(|source| CliError::io(&alias_file, source))?;
    let line = format_line(
        name,
        target,
        &version.to_string(),
        version.is_available(),
        false,
    );
    let output = Output::stdout(line);
    if version == Shown::NotAvailable {
        return Ok(output.with_stderr(format!("! WARNING: Version '{target}' does not exist.")));
    }
    Ok(output)
}

fn validate_name(name: &str) -> Result<(), CliError> {
    let message = if name.contains('#') {
        "Aliases with a comment delimiter (#) are not supported.".to_owned()
    } else if name.contains('/') {
        "Aliases in subdirectories are not supported.".to_owned()
    } else if matches!(name, "" | "." | "..") {
        format!("invalid alias name: {name}")
    } else {
        return Ok(());
    };
    Err(CliError::InvalidArgument(message))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "")
    }

    fn alias(fs: &FakeFileSystem, name: &str, target: &str) -> Result<Output, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        run(&Context::new(fs, &env), name, target)
    }

    fn alias_with_system_node(name: &str, target: &str) -> Result<Output, CliError> {
        let fs = installed()
            .with_file("/usr/bin/node", "")
            .with_file("/n/alias/default", "system");
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", "/usr/bin");
        run(&Context::new(&fs, &env), name, target)
    }

    fn stored(fs: &FakeFileSystem, name: &str) -> Option<String> {
        fs.read_to_string(&Path::new("/n/alias").join(name)).ok()
    }

    #[test]
    fn writes_the_target_as_the_alias_file() {
        let fs = installed();
        alias(&fs, "test", "v20.1.0").unwrap();
        assert_eq!(stored(&fs, "test"), Some("v20.1.0\n".to_owned()));
    }

    #[test]
    fn an_exact_installed_target_prints_one_arrow() {
        let output = alias(&installed(), "test", "v20.1.0").unwrap();
        assert_eq!(output, Output::stdout("test -> v20.1.0 *"));
    }

    #[test]
    fn a_pattern_target_shows_what_it_resolves_to() {
        let output = alias(&installed(), "work", "20").unwrap();
        assert_eq!(output, Output::stdout("work -> 20 (-> v20.1.0 *)"));
    }

    #[test]
    fn a_target_that_is_not_installed_warns_but_still_creates_the_alias() {
        let fs = FakeFileSystem::default();
        let output = alias(&fs, "test", "v0.1.2").unwrap();
        assert_eq!(output.stdout, "test -> v0.1.2 (-> N/A)");
        assert_eq!(output.stderr, "! WARNING: Version 'v0.1.2' does not exist.");
        assert_eq!(stored(&fs, "test"), Some("v0.1.2\n".to_owned()));
    }

    #[test]
    fn system_with_a_system_node_is_a_valid_target_without_a_warning() {
        let output = alias_with_system_node("default", "system").unwrap();
        assert_eq!(output, Output::stdout("default -> system *"));
    }

    #[test]
    fn an_alias_to_an_alias_of_system_shows_system() {
        let output = alias_with_system_node("foo", "default").unwrap();
        assert_eq!(output, Output::stdout("foo -> default (-> system *)"));
    }

    #[test]
    fn system_without_a_system_node_warns_like_any_missing_target() {
        let output = alias(&FakeFileSystem::default(), "default", "system").unwrap();
        assert_eq!(output.stdout, "default -> system (-> N/A)");
        assert_eq!(output.stderr, "! WARNING: Version 'system' does not exist.");
    }

    #[test]
    fn a_target_whose_chain_loops_shows_infinity_without_a_warning() {
        let fs = installed()
            .with_file("/n/alias/x", "y")
            .with_file("/n/alias/y", "x");
        let output = alias(&fs, "z", "x").unwrap();
        assert_eq!(output, Output::stdout("z -> x (-> ∞)"));
    }

    #[test]
    fn an_existing_alias_is_overwritten() {
        let fs = installed().with_file("/n/alias/work", "v18\n");
        alias(&fs, "work", "20").unwrap();
        assert_eq!(stored(&fs, "work"), Some("20\n".to_owned()));
    }

    #[test]
    fn an_empty_target_deletes_the_alias() {
        let fs = installed().with_file("/n/alias/work", "v18\n");
        let output = alias(&fs, "work", "").unwrap();
        assert!(output.stdout.starts_with("Deleted alias work"));
        assert_eq!(stored(&fs, "work"), None);
    }

    #[test]
    fn unsupported_names_are_rejected_and_nothing_is_written() {
        let cases = [
            (
                "a#b",
                "Aliases with a comment delimiter (#) are not supported.",
            ),
            ("lts/iron", "Aliases in subdirectories are not supported."),
            ("..", "invalid alias name: .."),
            (".", "invalid alias name: ."),
            ("", "invalid alias name: "),
        ];
        for (name, message) in cases {
            let fs = installed();
            let error = alias(&fs, name, "v20.1.0").unwrap_err();
            assert_eq!(error.to_string(), message);
            assert_eq!(fs.read_dir(Path::new("/n/alias")).ok(), None);
        }
    }
}

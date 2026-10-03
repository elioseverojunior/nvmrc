//! `nvm cache dir` and `nvm cache clear`: where downloads are kept, and
//! emptying that directory.

use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;

const USAGE: &str =
    "Usage: nvm cache dir\n       nvm cache clear\n  Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] for anything but `dir` and `clear`.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
/// - [`CliError::InvalidArgument`] when the directory cannot be cleared.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    match args.first().map(String::as_str) {
        Some("dir") => Ok(Output::stdout(context.cache_dir()?.display().to_string())),
        Some("clear") => clear(context),
        _ => Err(CliError::Usage(USAGE.to_owned())),
    }
}

fn clear(context: &Context<'_>) -> Result<Output, CliError> {
    let directory = context.cache_dir()?;
    let cleared = context
        .fs
        .remove_dir_all(&directory)
        .and_then(|()| context.fs.create_dir_all(&directory));
    if cleared.is_err() {
        let message = format!("Unable to clear nvm cache: {}", directory.display());
        return Err(CliError::InvalidArgument(message));
    }
    Ok(Output::stdout("nvm cache cleared."))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    fn env() -> FakeEnv {
        FakeEnv::default().with_var("NVM_DIR", "/n/")
    }

    #[test]
    fn dir_prints_the_cache_directory_under_nvm_dir() {
        let (fs, env) = (FakeFileSystem::default(), env());
        let output = run(&Context::new(&fs, &env), &words("dir")).unwrap();
        assert_eq!(output.stdout, "/n/.cache");
    }

    #[test]
    fn clear_empties_the_directory_and_recreates_it() {
        let fs = FakeFileSystem::default()
            .with_file("/n/.cache/bin/node-v20.1.0/node-v20.1.0.tar.gz", "x")
            .with_file("/n/alias/default", "node");
        let env = env();
        let output = run(&Context::new(&fs, &env), &words("clear")).unwrap();
        assert_eq!(output.stdout, "nvm cache cleared.");
        assert_eq!(fs.read_dir(Path::new("/n/.cache")).unwrap(), []);
        assert!(fs.is_file(Path::new("/n/alias/default")));
    }

    #[test]
    fn anything_else_is_the_usage_with_status_127() {
        let (fs, env) = (FakeFileSystem::default(), env());
        for line in ["", "list", "--help"] {
            let error = run(&Context::new(&fs, &env), &words(line)).unwrap_err();
            assert_eq!(error.exit_code(), NvmExitCode::NotFound);
            assert_eq!(error.to_string(), USAGE);
        }
    }
}

//! The aliases an install leaves behind: `--default`, `--alias=<name>`, and
//! the `default` alias when there is none yet (`nvm_ensure_default_set`).

use crate::commands::alias;
use crate::commands::install::flow::Step;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::alias::AliasStore;

/// `nvm alias <name> <target>`, with what it prints.
pub fn make_alias(
    context: &Context<'_>,
    name: &str,
    target: &str,
    transcript: &mut Transcript,
) -> Step<()> {
    transcript.absorb(alias::run(context, name, target)?);
    Ok(())
}

/// Creates `default` pointing at `target`, unless there is a `default`
/// already.
pub fn ensure_default(
    context: &Context<'_>,
    target: &str,
    transcript: &mut Transcript,
) -> Step<()> {
    if context.alias_store()?.target("default").is_some() {
        return Ok(());
    }
    let output = alias::run(context, "default", target)?;
    transcript.out(format!("Creating default alias: {}", output.stdout));
    if !output.stderr.is_empty() {
        transcript.err(output.stderr);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn lines(
        fs: &FakeFileSystem,
        run: impl Fn(&Context<'_>, &mut Transcript) -> Step<()>,
    ) -> (String, String) {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let context = Context::new(fs, &env);
        let mut transcript = Transcript::default();
        run(&context, &mut transcript).unwrap();
        let output = transcript.finish(NvmExitCode::Success);
        (output.stdout, output.stderr)
    }

    #[test]
    fn a_missing_default_is_created_and_announced() {
        let fs =
            FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "x");
        let (stdout, _) = lines(&fs, |c, t| ensure_default(c, "20", t));
        assert_eq!(
            stdout,
            "Creating default alias: default -> 20 (-> v20.10.0 *)"
        );
        assert_eq!(
            fs.read_to_string(Path::new("/n/alias/default")).unwrap(),
            "20\n"
        );
    }

    #[test]
    fn an_existing_default_is_left_alone() {
        let fs = FakeFileSystem::default().with_file("/n/alias/default", "18\n");
        let (stdout, stderr) = lines(&fs, |c, t| ensure_default(c, "20", t));
        assert_eq!((stdout.as_str(), stderr.as_str()), ("", ""));
        assert_eq!(
            fs.read_to_string(Path::new("/n/alias/default")).unwrap(),
            "18\n"
        );
    }

    #[test]
    fn make_alias_shows_the_line_nvm_alias_prints() {
        let fs =
            FakeFileSystem::default().with_executable("/n/versions/node/v18.19.0/bin/node", "x");
        let (stdout, _) = lines(&fs, |c, t| make_alias(c, "work", "18", t));
        assert_eq!(stdout, "work -> 18 (-> v18.19.0 *)");
    }
}

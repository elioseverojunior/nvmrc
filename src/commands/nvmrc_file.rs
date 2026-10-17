//! `--save` / `-w` of `install` and `use`: write a version to `.nvmrc` in
//! the current directory (`nvm_write_nvmrc`).

use std::io;
use std::path::Path;

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::with_version;
use crate::error::NvmExitCode;

/// Writes `version` to `.nvmrc` in the logical working directory
/// ([`Context::working_directory`]) and says so on stdout: it replaces the
/// bare version line and keeps the pins and every other line (a file with
/// nothing else becomes `<version>\n`). Failing to, or an `.nvmrc` that
/// cannot be read, is status 3, after the warning `nvm.sh` gives on stderr.
/// `silent` drops both messages, as `NVM_SILENT=1` does.
pub fn write(
    context: &Context<'_>,
    version: &str,
    silent: bool,
    transcript: &mut Transcript,
) -> NvmExitCode {
    let succeeded = context
        .working_directory()
        .is_some_and(|directory| save(context, &directory.join(".nvmrc"), version).is_ok());
    if !silent && succeeded {
        transcript.out(format!("Wrote version number ({version}) to .nvmrc"));
    } else if !silent {
        transcript.err(format!(
            "Warning: Unable to write version number ({version}) to .nvmrc"
        ));
    }
    if succeeded {
        NvmExitCode::Success
    } else {
        NvmExitCode::InvalidVersion
    }
}

/// `version` saved over the `.nvmrc` at `file` (none yet: an empty one).
fn save(context: &Context<'_>, file: &Path, version: &str) -> io::Result<()> {
    let contents = match context.fs.read_to_string(file) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    context
        .fs
        .write_file(file, &with_version(&contents, version))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    const VERSION: &str = "v20.10.0";

    #[test]
    fn it_writes_the_version_and_says_so() {
        let (fs, env) = (
            FakeFileSystem::default(),
            FakeEnv::default().with_var("PWD", "/work"),
        );
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), VERSION, false, &mut transcript);
        assert_eq!(status, NvmExitCode::Success);
        assert_eq!(
            fs.read_to_string(Path::new("/work/.nvmrc")).unwrap(),
            "v20.10.0\n"
        );
        let output = transcript.finish(status);
        assert_eq!(output.stdout, "Wrote version number (v20.10.0) to .nvmrc");
    }

    #[test]
    fn silent_writes_without_a_word_and_fails_without_a_warning() {
        let (fs, env) = (
            FakeFileSystem::default(),
            FakeEnv::default().with_var("PWD", "/work"),
        );
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), "system", true, &mut transcript);
        assert_eq!(status, NvmExitCode::Success);
        assert_eq!(
            fs.read_to_string(Path::new("/work/.nvmrc")).unwrap(),
            "system\n"
        );
        let unwritable = FakeEnv::default();
        let failed = write(
            &Context::new(&fs, &unwritable),
            "v1.0.0",
            true,
            &mut transcript,
        );
        assert_eq!(failed, NvmExitCode::InvalidVersion);
        let output = transcript.finish(status);
        assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
    }

    fn saved_over(contents: &str) -> (NvmExitCode, Option<String>) {
        let fs = FakeFileSystem::default().with_file("/work/.nvmrc", contents);
        let env = FakeEnv::default().with_var("PWD", "/work");
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), VERSION, true, &mut transcript);
        (status, fs.read_to_string(Path::new("/work/.nvmrc")).ok())
    }

    #[test]
    fn saving_keeps_the_pins_and_the_other_lines() {
        let (status, saved) = saved_over("18\n# tools\nnpm=10.9.0\npnpm=9.12.0\n");
        assert_eq!(status, NvmExitCode::Success);
        let expected = "v20.10.0\n# tools\nnpm=10.9.0\npnpm=9.12.0\n";
        assert_eq!(saved.as_deref(), Some(expected));
    }

    #[test]
    fn saving_over_a_lone_version_writes_just_the_version() {
        let (status, saved) = saved_over("18\n\n");
        assert_eq!(status, NvmExitCode::Success);
        assert_eq!(saved.as_deref(), Some("v20.10.0\n"));
    }

    #[test]
    fn an_unreadable_nvmrc_is_left_alone() {
        let fs = FakeFileSystem::default().with_bytes("/work/.nvmrc", &[0xff, b'\n']);
        let env = FakeEnv::default().with_var("PWD", "/work");
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), VERSION, false, &mut transcript);
        assert_eq!(status, NvmExitCode::InvalidVersion);
        assert!(fs.read_to_string(Path::new("/work/.nvmrc")).is_err());
    }

    #[test]
    fn no_current_directory_is_a_warning_and_status_3() {
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), VERSION, false, &mut transcript);
        assert_eq!(status, NvmExitCode::InvalidVersion);
        let output = transcript.finish(status);
        assert_eq!(
            output.stderr,
            "Warning: Unable to write version number (v20.10.0) to .nvmrc"
        );
    }
}

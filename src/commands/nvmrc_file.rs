//! `--save` / `-w` of `install` and `use`: write a version to `.nvmrc` in
//! the current directory (`nvm_write_nvmrc`).

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::NvmExitCode;

/// Writes `<version>\n` to `.nvmrc` in the logical working directory
/// ([`Context::working_directory`]) and says so on stdout. Failing to is
/// status 3, after the warning `nvm.sh` gives on stderr. `silent` drops both
/// messages, as `NVM_SILENT=1` does.
pub fn write(
    context: &Context<'_>,
    version: &str,
    silent: bool,
    transcript: &mut Transcript,
) -> NvmExitCode {
    let written = context
        .working_directory()
        .map(|directory| directory.join(".nvmrc"))
        .map(|file| context.fs.write_file(&file, &format!("{version}\n")));
    let succeeded = matches!(written, Some(Ok(())));
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

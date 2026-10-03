//! `--save` / `-w`: write the installed version to `.nvmrc` in the current
//! directory (`nvm_write_nvmrc`).

use std::path::PathBuf;

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// Writes `<version>\n` to `$PWD/.nvmrc`. Failing to is status 3, after the
/// warning `nvm.sh` gives.
pub fn write(context: &Context<'_>, version: &Version, transcript: &mut Transcript) -> NvmExitCode {
    let directory = context.env.var_os("PWD").map(PathBuf::from);
    let written = directory
        .map(|directory| directory.join(".nvmrc"))
        .map(|file| context.fs.write_file(&file, &format!("{version}\n")));
    if let Some(Ok(())) = written {
        transcript.out(format!("Wrote version number ({version}) to .nvmrc"));
        return NvmExitCode::Success;
    }
    transcript.err(format!(
        "Warning: Unable to write version number ({version}) to .nvmrc"
    ));
    NvmExitCode::InvalidVersion
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn version() -> Version {
        "v20.10.0".parse().unwrap()
    }

    #[test]
    fn it_writes_the_version_and_says_so() {
        let (fs, env) = (
            FakeFileSystem::default(),
            FakeEnv::default().with_var("PWD", "/work"),
        );
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), &version(), &mut transcript);
        assert_eq!(status, NvmExitCode::Success);
        assert_eq!(
            fs.read_to_string(Path::new("/work/.nvmrc")).unwrap(),
            "v20.10.0\n"
        );
        let output = transcript.finish(status);
        assert_eq!(output.stdout, "Wrote version number (v20.10.0) to .nvmrc");
    }

    #[test]
    fn no_current_directory_is_a_warning_and_status_3() {
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), &version(), &mut transcript);
        assert_eq!(status, NvmExitCode::InvalidVersion);
        let output = transcript.finish(status);
        assert_eq!(
            output.stderr,
            "Warning: Unable to write version number (v20.10.0) to .nvmrc"
        );
    }
}

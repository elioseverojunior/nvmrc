//! The pinned package managers on `PATH`.

use super::store::{bin_dir, is_installed};
use crate::commands::Output;
use crate::commands::project_pins::ProjectPins;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::tool::Tool;
use crate::domain::tool_path::{prepend, strip_tool_bins};
use crate::domain::version::Version;
use crate::error::{CliError, PinsError};

/// `path` with the tool directories of any other project taken out and the
/// `bin` of each pinned, installed tool ahead of everything else, in the
/// order npm, yarn, pnpm. A pinned tool that is not installed is an error,
/// or in an advisory context a warning, and is left out.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found, and
/// [`PinsError::NotInstalled`] for a pinned tool that is not installed.
pub fn pinned_path(
    context: &Context<'_>,
    path: &str,
    pins: &ProjectPins,
    transcript: &mut Transcript,
) -> Result<String, CliError> {
    let nvm_dir = context.nvm_dir()?.to_string_lossy().into_owned();
    let mut directories = Vec::new();
    for (tool, version) in pins.pins.iter() {
        if is_installed(context, tool, &version) {
            let bin = bin_dir(context, tool, &version)?;
            directories.push(bin.to_string_lossy().into_owned());
        } else {
            not_installed(context, (tool, &version), pins, transcript)?;
        }
    }
    Ok(prepend(&strip_tool_bins(path, &nvm_dir), &directories))
}

/// The output of a command that stops on `error`: what `transcript` holds,
/// then the error on stderr, with the status of the error.
#[must_use]
pub fn stopped(error: &CliError, mut transcript: Transcript) -> Output {
    transcript.err(error.to_string());
    transcript.finish(error.exit_code())
}

/// The error for a pin without an install, or in an advisory context the
/// warning that stands for it.
fn not_installed(
    context: &Context<'_>,
    (tool, version): (Tool, &Version),
    pins: &ProjectPins,
    transcript: &mut Transcript,
) -> Result<(), CliError> {
    let nvmrc = pins.nvmrc.as_ref().map(|path| path.display().to_string());
    let error = PinsError::NotInstalled {
        tool: format!("{} {}", tool.name(), tool.show(version)),
        nvmrc: nvmrc.unwrap_or_default(),
    };
    if !context.is_advisory() {
        return Err(error.into());
    }
    let message = error.to_string();
    let text = message.strip_prefix("nvm: ").unwrap_or(&message);
    transcript.err(format!("nvm: warning: {text}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::nvmrc::pins::parse_pins;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use std::path::PathBuf;

    const STORE: &str = "/n/package-managers";

    fn pins(contents: &str) -> ProjectPins {
        ProjectPins {
            nvmrc: Some(PathBuf::from("/p/.nvmrc")),
            pins: parse_pins(contents).unwrap(),
        }
    }

    fn installed(fs: FakeFileSystem, tool: &str, version: &str) -> FakeFileSystem {
        fs.with_file(
            &format!("{STORE}/{tool}/{version}/package/package.json"),
            "{}",
        )
        .with_executable(&format!("{STORE}/{tool}/{version}/bin/{tool}"), "")
    }

    fn pinned(
        fs: &FakeFileSystem,
        advisory: bool,
        contents: &str,
        transcript: &mut Transcript,
    ) -> Result<String, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let context = Context::new(fs, &env);
        let context = if advisory {
            context.advisory()
        } else {
            context
        };
        let path = "/n/package-managers/yarn/1.22.22/bin:/n/versions/node/v20.11.1/bin:/usr/bin";
        pinned_path(&context, path, &pins(contents), transcript)
    }

    #[test]
    fn pins_go_ahead_of_node() {
        let fs = installed(
            installed(FakeFileSystem::default(), "npm", "10.9.0"),
            "pnpm",
            "9.12.0",
        );
        let mut transcript = Transcript::default();
        let path = pinned(&fs, false, "20\nnpm=10.9.0\npnpm=9.12.0\n", &mut transcript).unwrap();
        let expected = format!(
            "{STORE}/npm/10.9.0/bin:{STORE}/pnpm/9.12.0/bin:/n/versions/node/v20.11.1/bin:/usr/bin"
        );
        assert_eq!(path, expected);
    }

    #[test]
    fn a_missing_tool_fails_explicitly() {
        let mut transcript = Transcript::default();
        let error = pinned(
            &FakeFileSystem::default(),
            false,
            "20\npnpm=9.12.0\n",
            &mut transcript,
        )
        .unwrap_err();
        let text = "nvm: pnpm 9.12.0 (from /p/.nvmrc) is not installed. Run `nvm install`.";
        assert_eq!(error.to_string(), text);
    }

    #[test]
    fn a_missing_tool_is_a_warning_when_advisory() {
        let fs = installed(FakeFileSystem::default(), "npm", "10.9.0");
        let mut transcript = Transcript::default();
        let contents = "20\nnpm=10.9.0\npnpm=9.12.0\n";
        let path = pinned(&fs, true, contents, &mut transcript).unwrap();
        assert_eq!(
            path,
            format!("{STORE}/npm/10.9.0/bin:/n/versions/node/v20.11.1/bin:/usr/bin")
        );
        let output = transcript.finish(crate::error::NvmExitCode::Success);
        let text =
            "nvm: warning: pnpm 9.12.0 (from /p/.nvmrc) is not installed. Run `nvm install`.";
        assert_eq!(output.stderr, text);
    }
}

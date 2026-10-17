//! The pins of the project: the nearest `.nvmrc`, checked against the
//! `package.json` beside it.

use std::path::{Path, PathBuf};

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::find_nvmrc;
use crate::domain::nvmrc::pins::{PinError, Pins, parse_pins};
use crate::domain::package_json::{
    PackageManagerError, check_package_manager, package_manager_field,
};
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::{CliError, PinsError};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProjectPins {
    pub nvmrc: Option<PathBuf>,
    pub pins: Pins,
}

/// The pins of the nearest `.nvmrc` from the working directory (none: no
/// pins), with the `package.json` beside it checked when yarn or pnpm is
/// pinned.
pub fn read(context: &Context<'_>) -> Result<ProjectPins, CliError> {
    let nvmrc = context
        .working_directory()
        .and_then(|directory| find_nvmrc(context.fs, &directory));
    let Some(nvmrc) = nvmrc else {
        return Ok(ProjectPins::default());
    };
    let contents = context
        .fs
        .read_to_string(&nvmrc)
        .map_err(|source| CliError::io(&nvmrc, source))?;
    let pins = parse_pins(&contents).map_err(|error| pin_error(error, &nvmrc))?;
    if let Some((tool, version)) = pins.package_manager() {
        check_beside(context, &nvmrc, tool, &version)?;
    }
    Ok(ProjectPins {
        nvmrc: Some(nvmrc),
        pins,
    })
}

/// Like [`read`], but in an advisory context an error is a warning and the
/// result is empty.
pub fn read_for(
    context: &Context<'_>,
    transcript: &mut Transcript,
) -> Result<ProjectPins, CliError> {
    match read(context) {
        Err(error) if context.is_advisory() => {
            let message = error.to_string();
            let text = message.strip_prefix("nvm: ").unwrap_or(&message);
            transcript.err(format!("nvm: warning: {text}"));
            Ok(ProjectPins::default())
        }
        other => other,
    }
}

fn pin_error(error: PinError, nvmrc: &Path) -> PinsError {
    let nvmrc = nvmrc.display().to_string();
    match error {
        PinError::NotExact { line } => PinsError::NotExact { nvmrc, line },
        PinError::YarnAndPnpm => PinsError::YarnAndPnpm { nvmrc },
    }
}

fn check_beside(
    context: &Context<'_>,
    nvmrc: &Path,
    tool: Tool,
    version: &Version,
) -> Result<(), CliError> {
    let package_json = nvmrc.with_file_name("package.json");
    if !context.fs.is_file(&package_json) {
        return Ok(());
    }
    let text = context
        .fs
        .read_to_string(&package_json)
        .map_err(|source| CliError::io(&package_json, source))?;
    check_package_manager(&text, tool, version)
        .map_err(|error| package_manager_error(error, tool, version, &package_json, nvmrc).into())
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

fn package_manager_error(
    error: PackageManagerError,
    tool: Tool,
    version: &Version,
    package_json: &Path,
    nvmrc: &Path,
) -> PinsError {
    let (package_json, nvmrc) = (shown(package_json), shown(nvmrc));
    let expected = package_manager_field(tool, version);
    match error {
        PackageManagerError::Malformed(reason) => PinsError::UnreadablePackageJson {
            package_json,
            reason,
        },
        PackageManagerError::Missing => PinsError::MissingPackageManager {
            package_json,
            nvmrc,
            expected,
        },
        PackageManagerError::Different { found } => PinsError::DifferentPackageManager {
            package_json,
            nvmrc,
            found,
            tool: tool.name().to_owned(),
            version: tool.show(version),
            expected,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::transcript::Transcript;
    use crate::context::Context;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn env() -> FakeEnv {
        FakeEnv::default().with_var("PWD", "/p")
    }

    #[test]
    fn no_nvmrc_means_no_pins() {
        let (fs, env) = (FakeFileSystem::default(), env());
        let found = read(&Context::new(&fs, &env)).unwrap();
        assert_eq!(found, ProjectPins::default());
    }

    #[test]
    fn no_package_json_passes() {
        let fs = FakeFileSystem::default().with_file("/p/.nvmrc", "20\npnpm=9.12.0\n");
        let env = env();
        let found = read(&Context::new(&fs, &env)).unwrap();
        assert_eq!(found.nvmrc, Some(PathBuf::from("/p/.nvmrc")));
        assert!(found.pins.pnpm.is_some());
    }

    #[test]
    fn a_divergent_package_json_exits_3() {
        let fs = FakeFileSystem::default()
            .with_file("/p/.nvmrc", "yarn=4.5.1\n")
            .with_file("/p/package.json", r#"{"packageManager": "yarn@4.5.0"}"#);
        let env = env();
        let error = read(&Context::new(&fs, &env)).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::InvalidVersion);
        assert_eq!(
            error.to_string(),
            "nvm: /p/package.json has packageManager \"yarn@4.5.0\", but /p/.nvmrc pins yarn 4.5.1; set it to \"yarn@4.5.1\"."
        );
    }

    #[test]
    fn the_other_pin_errors_print_their_messages() {
        let env = env();
        let cases = [
            (
                "pnpm=9.12\n",
                None,
                "nvm: /p/.nvmrc: 'pnpm=9.12' is not an exact version (major.minor.patch).",
            ),
            (
                "yarn=4.5.0\npnpm=9.12.0\n",
                None,
                "nvm: /p/.nvmrc pins both yarn and pnpm; pin at most one.",
            ),
            (
                "pnpm=9.12.0\n",
                Some("{}"),
                "nvm: /p/package.json has no packageManager; set it to \"pnpm@9.12.0\" to match /p/.nvmrc.",
            ),
            (
                "pnpm=9.12.0\n",
                Some("{"),
                "nvm: /p/package.json cannot be read as JSON: ",
            ),
        ];
        for (nvmrc, package_json, expected) in cases {
            let mut fs = FakeFileSystem::default().with_file("/p/.nvmrc", nvmrc);
            if let Some(text) = package_json {
                fs = fs.with_file("/p/package.json", text);
            }
            let message = read(&Context::new(&fs, &env)).unwrap_err().to_string();
            assert!(message.starts_with(expected), "{message}");
        }
    }

    #[test]
    fn an_advisory_context_warns_and_returns_no_pins() {
        let fs = FakeFileSystem::default().with_file("/p/.nvmrc", "pnpm=9.12\n");
        let env = env();
        let context = Context::new(&fs, &env).advisory();
        let mut transcript = Transcript::default();
        let found = read_for(&context, &mut transcript).unwrap();
        assert_eq!(found, ProjectPins::default());
        assert_eq!(
            transcript.finish(NvmExitCode::Success).stderr,
            "nvm: warning: /p/.nvmrc: 'pnpm=9.12' is not an exact version (major.minor.patch)."
        );
        let strict = Context::new(&fs, &env);
        assert!(read_for(&strict, &mut Transcript::default()).is_err());
    }
}

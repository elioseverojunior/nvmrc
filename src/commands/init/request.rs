//! The arguments of `nvm init`: a shell name with `--no-use` or `--install`
//! (shell mode), or `--node`, `--npm`, `--yarn` and `--pnpm`, each with an
//! optional exact version (project mode).

use crate::domain::nvmrc::pins::exact_version;
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::CliError;
use crate::shell::init::{InitOptions, Shell, USAGE};

/// What a project-mode flag asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanted {
    /// The flag had no version: the newest allowed release.
    Newest,
    Exact(Version),
}

/// The tools `nvm init` was asked to set up; at least one is `Some`, and
/// never both yarn and pnpm.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProjectRequest {
    pub node: Option<Wanted>,
    pub npm: Option<Wanted>,
    pub yarn: Option<Wanted>,
    pub pnpm: Option<Wanted>,
}

impl ProjectRequest {
    #[must_use]
    pub fn get(&self, tool: Tool) -> Option<&Wanted> {
        match tool {
            Tool::Node => self.node.as_ref(),
            Tool::Npm => self.npm.as_ref(),
            Tool::Yarn => self.yarn.as_ref(),
            Tool::Pnpm => self.pnpm.as_ref(),
        }
    }

    fn slot(&mut self, tool: Tool) -> &mut Option<Wanted> {
        match tool {
            Tool::Node => &mut self.node,
            Tool::Npm => &mut self.npm,
            Tool::Yarn => &mut self.yarn,
            Tool::Pnpm => &mut self.pnpm,
        }
    }
}

/// How `nvm init` was called.
#[derive(Debug, PartialEq, Eq)]
pub enum InitMode {
    Shell(Shell, InitOptions),
    Project(ProjectRequest),
}

/// Reads the arguments. Of `--no-use` and `--install`, the last one given
/// wins, as in `nvm_process_parameters`.
///
/// # Errors
/// [`CliError::Usage`] without a shell or project flag, or for an
/// unsupported shell; [`CliError::Unsupported`] for an unknown option or a
/// second shell; [`CliError::InvalidOptions`] for a shell with a project
/// flag, `--no-use` or `--install` with a project flag, yarn with pnpm, or a
/// flag given twice;
/// [`CliError::InvalidVersionArgument`] for a version that is not exact.
pub fn parse(args: &[String]) -> Result<InitMode, CliError> {
    let mut options: Option<InitOptions> = None;
    let mut shell_name: Option<&str> = None;
    let mut request = ProjectRequest::default();
    let mut asked = false;
    for arg in args {
        if let Some((tool, wanted)) = project_flag(arg)? {
            if request.slot(tool).replace(wanted).is_some() {
                let message = format!("nvm init: --{} is given twice.", tool.name());
                return Err(CliError::InvalidOptions(message));
            }
            asked = true;
            continue;
        }
        match arg.as_str() {
            "--no-use" => options = Some(auto_step(true, false)),
            "--install" => options = Some(auto_step(false, true)),
            word if word.starts_with('-') || shell_name.is_some() => {
                let message = format!("nvm init: unsupported argument \"{word}\".\n{USAGE}");
                return Err(CliError::Unsupported(message));
            }
            word => shell_name = Some(word),
        }
    }
    finish(shell_name, options, asked.then_some(request))
}

fn auto_step(no_use: bool, install: bool) -> InitOptions {
    InitOptions { no_use, install }
}

/// Settles the mode once every argument is read; `request` is `Some` when
/// a project flag was given, `options` when `--no-use` or `--install` was.
fn finish(
    shell_name: Option<&str>,
    options: Option<InitOptions>,
    request: Option<ProjectRequest>,
) -> Result<InitMode, CliError> {
    match (shell_name, request) {
        (Some(_), Some(_)) => Err(CliError::InvalidOptions(
            "nvm init: a shell name cannot be combined with --node, --npm, --yarn or --pnpm."
                .to_owned(),
        )),
        (Some(name), None) => Ok(InitMode::Shell(name.parse()?, options.unwrap_or_default())),
        (None, Some(_)) if options.is_some() => Err(CliError::InvalidOptions(
            "nvm init: --no-use and --install apply to the shell snippet only.".to_owned(),
        )),
        (None, Some(request)) if request.yarn.is_some() && request.pnpm.is_some() => {
            Err(CliError::InvalidOptions(
                "nvm init: --yarn and --pnpm cannot be used together.".to_owned(),
            ))
        }
        (None, Some(request)) => Ok(InitMode::Project(request)),
        (None, None) => Err(CliError::Usage(USAGE.to_owned())),
    }
}

/// The tool and wish of `--node`, `--npm=10.9.0` and the like; `None` for
/// any other argument.
fn project_flag(arg: &str) -> Result<Option<(Tool, Wanted)>, CliError> {
    let (name, value) = match arg.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (arg, None),
    };
    let tool = match name {
        "--node" => Tool::Node,
        "--npm" => Tool::Npm,
        "--yarn" => Tool::Yarn,
        "--pnpm" => Tool::Pnpm,
        _ => return Ok(None),
    };
    let wanted = match value {
        None => Wanted::Newest,
        Some(text) => Wanted::Exact(exact_for(tool, text).ok_or_else(|| {
            CliError::InvalidVersionArgument(format!(
                "nvm init: {arg} is not an exact version (major.minor.patch)."
            ))
        })?),
    };
    Ok(Some((tool, wanted)))
}

/// Node takes an optional `v`; the package managers, digits only.
fn exact_for(tool: Tool, text: &str) -> Option<Version> {
    match tool {
        Tool::Node => exact_version(text.strip_prefix('v').unwrap_or(text)),
        _ => exact_version(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(args: &[&str]) -> Vec<String> {
        args.iter().map(|word| (*word).to_owned()).collect()
    }

    fn failure(args: &[&str]) -> (u8, String) {
        let error = parse(&words(args)).expect_err("must fail");
        (error.exit_code().code(), error.to_string())
    }

    fn version(text: &str) -> Wanted {
        Wanted::Exact(text.parse().expect("test version"))
    }

    #[test]
    fn a_shell_name_is_shell_mode() {
        let mode = parse(&words(&["bash", "--no-use"])).expect("shell mode");
        let options = InitOptions {
            no_use: true,
            install: false,
        };
        assert_eq!(mode, InitMode::Shell(Shell::Bash, options));
    }

    #[test]
    fn project_flags_without_a_version_want_the_newest() {
        let mode = parse(&words(&["--node", "--npm", "--yarn"])).expect("project mode");
        let InitMode::Project(request) = mode else {
            panic!("not project mode");
        };
        assert_eq!(request.get(Tool::Node), Some(&Wanted::Newest));
        assert_eq!(request.get(Tool::Npm), Some(&Wanted::Newest));
        assert_eq!(request.get(Tool::Yarn), Some(&Wanted::Newest));
        assert_eq!(request.get(Tool::Pnpm), None);
    }

    #[test]
    fn project_flags_with_a_version_want_it_exactly() {
        let mode = parse(&words(&["--node=v20.11.1", "--pnpm=9.12.0"])).expect("project mode");
        let InitMode::Project(request) = mode else {
            panic!("not project mode");
        };
        assert_eq!(request.node, Some(version("20.11.1")));
        assert_eq!(request.pnpm, Some(version("9.12.0")));
    }

    #[test]
    fn a_shell_with_a_project_flag_is_a_usage_error() {
        let (code, message) = failure(&["bash", "--node"]);
        assert_eq!(code, 6);
        assert_eq!(
            message,
            "nvm init: a shell name cannot be combined with --node, --npm, --yarn or --pnpm."
        );
    }

    #[test]
    fn yarn_and_pnpm_together_are_a_usage_error() {
        let (code, message) = failure(&["--yarn", "--pnpm=9.12.0"]);
        assert_eq!(code, 6);
        assert_eq!(
            message,
            "nvm init: --yarn and --pnpm cannot be used together."
        );
    }

    #[test]
    fn a_repeated_flag_is_a_usage_error() {
        let (code, message) = failure(&["--npm", "--npm=10.9.0"]);
        assert_eq!(code, 6);
        assert_eq!(message, "nvm init: --npm is given twice.");
    }

    #[test]
    fn shell_options_with_a_project_flag_are_a_usage_error() {
        for args in [["--node", "--no-use"], ["--install", "--pnpm"]] {
            let (code, message) = failure(&args);
            assert_eq!(code, 6, "{args:?}");
            assert_eq!(
                message,
                "nvm init: --no-use and --install apply to the shell snippet only."
            );
        }
    }

    #[test]
    fn inexact_versions_are_rejected() {
        let flags = [
            "--node=20",
            "--node=iojs-v3.3.1",
            "--npm=v10.9.0",
            "--pnpm=latest",
        ];
        for flag in flags {
            let (code, message) = failure(&[flag]);
            assert_eq!(code, 3, "{flag}");
            let expected = format!("nvm init: {flag} is not an exact version (major.minor.patch).");
            assert_eq!(message, expected);
        }
    }
}

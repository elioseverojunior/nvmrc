//! The arguments of `nvm upgrade`: tool flags choosing what to upgrade, and
//! `--major` or `--major=<N>` choosing which major to move to.

use crate::domain::tool::Tool;
use crate::error::CliError;

/// The refusal of `--major=<N>` for a selection that is not one tool.
pub const ONE_TOOL: &str = "--major=<N> applies to one tool; name exactly one.";

/// Which major release each selected tool moves within.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Major {
    /// The major the tool already uses.
    Same,
    /// Any major: the newest allowed release.
    Any,
    Exactly(u64),
}

/// What `nvm upgrade` was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeRequest {
    /// The tools to upgrade in node, npm, yarn, pnpm order; empty means
    /// every tool the `.nvmrc` pins.
    pub tools: Vec<Tool>,
    pub major: Major,
}

/// Reads the arguments.
///
/// # Errors
/// [`CliError::InvalidOptions`] for a flag given twice, a `--major=` value
/// that is not digits, an unknown argument, or `--major=<N>` with two or more
/// tool flags. `--major=<N>` with none passes: whether the `.nvmrc` pins just
/// one tool is for the caller to check.
pub fn parse(args: &[String]) -> Result<UpgradeRequest, CliError> {
    let mut chosen: Vec<Tool> = Vec::new();
    let mut major: Option<Major> = None;
    for arg in args {
        if let Some(tool) = tool_flag(arg) {
            if chosen.contains(&tool) {
                return Err(invalid(format!("--{} is given twice.", tool.name())));
            }
            chosen.push(tool);
        } else if let Some(value) = major_flag(arg)? {
            if major.replace(value).is_some() {
                return Err(invalid("--major is given twice.".to_owned()));
            }
        } else {
            return Err(invalid(format!("unsupported argument \"{arg}\".")));
        }
    }
    let tools: Vec<Tool> = Tool::ALL
        .into_iter()
        .filter(|tool| chosen.contains(tool))
        .collect();
    let major = major.unwrap_or(Major::Same);
    if matches!(major, Major::Exactly(_)) && tools.len() > 1 {
        return Err(invalid(ONE_TOOL.to_owned()));
    }
    Ok(UpgradeRequest { tools, major })
}

fn invalid(message: String) -> CliError {
    CliError::InvalidOptions(format!("nvm upgrade: {message}"))
}

fn tool_flag(arg: &str) -> Option<Tool> {
    Tool::ALL
        .into_iter()
        .find(|tool| arg.strip_prefix("--") == Some(tool.name()))
}

/// The wish of `--major` or `--major=<N>`; `None` for any other argument.
fn major_flag(arg: &str) -> Result<Option<Major>, CliError> {
    if arg == "--major" {
        return Ok(Some(Major::Any));
    }
    let Some(digits) = arg.strip_prefix("--major=") else {
        return Ok(None);
    };
    let number = digits
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| digits.parse().ok())
        .flatten()
        .ok_or_else(|| invalid(format!("--major={digits} is not a major version.")))?;
    Ok(Some(Major::Exactly(number)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<UpgradeRequest, CliError> {
        let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        parse(&owned)
    }

    fn message(args: &[&str]) -> String {
        match run(args) {
            Err(CliError::InvalidOptions(text)) => text,
            other => panic!("expected InvalidOptions, got {other:?}"),
        }
    }

    #[test]
    fn no_flags_select_every_pinned_tool() {
        let request = run(&[]).unwrap();
        assert_eq!((request.tools, request.major), (vec![], Major::Same));
    }

    #[test]
    fn tool_flags_select_those_tools_in_order() {
        let request = run(&["--pnpm", "--node", "--npm"]).unwrap();
        assert_eq!(request.tools, [Tool::Node, Tool::Npm, Tool::Pnpm]);
    }

    #[test]
    fn major_alone_means_any() {
        assert_eq!(run(&["--major"]).unwrap().major, Major::Any);
    }

    #[test]
    fn major_with_a_number_means_that_major() {
        let request = run(&["--yarn", "--major=4"]).unwrap();
        assert_eq!(request.major, Major::Exactly(4));
        assert_eq!(request.tools, [Tool::Yarn]);
    }

    #[test]
    fn a_repeated_flag_is_a_usage_error() {
        assert_eq!(
            message(&["--npm", "--npm"]),
            "nvm upgrade: --npm is given twice."
        );
        assert_eq!(
            message(&["--major", "--major=3"]),
            "nvm upgrade: --major is given twice."
        );
    }

    #[test]
    fn a_major_that_is_not_a_number_is_a_usage_error() {
        for bad in ["x", "", "-1", "1.2"] {
            let text = message(&[&format!("--major={bad}")]);
            assert_eq!(
                text,
                format!("nvm upgrade: --major={bad} is not a major version.")
            );
        }
    }

    #[test]
    fn major_n_with_two_tools_is_a_usage_error() {
        let expected = "nvm upgrade: --major=<N> applies to one tool; name exactly one.";
        assert_eq!(message(&["--node", "--npm", "--major=2"]), expected);
    }

    #[test]
    fn major_n_without_a_tool_flag_is_left_to_the_nvmrc_check() {
        let request = run(&["--major=2"]).unwrap();
        assert_eq!((request.tools, request.major), (vec![], Major::Exactly(2)));
    }

    #[test]
    fn an_unknown_argument_is_a_usage_error() {
        assert_eq!(message(&["x"]), "nvm upgrade: unsupported argument \"x\".");
    }
}

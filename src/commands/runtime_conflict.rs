//! `nvm __conflict <helpers|function>`: the warning the init snippet has the
//! binary print, on stderr, when nvm.sh is still loaded in an interactive
//! shell (see [`crate::domain::conflict::RuntimeConflict`]). Exits 0: the
//! warning never fails the shell's start.

use crate::commands::Output;
use crate::domain::conflict::{RuntimeConflict, runtime_warning};
use crate::error::CliError;

/// How `nvm __conflict` is called.
pub const USAGE: &str = "Usage: nvm __conflict <helpers|function>";

/// Prints the warning for the reason named in `args`.
///
/// # Errors
/// [`CliError::Usage`] without exactly one argument naming a reason.
pub fn run(args: &[String]) -> Result<Output, CliError> {
    match args {
        [name] => RuntimeConflict::from_name(name)
            .map(|reason| Output::default().with_stderr(runtime_warning(reason)))
            .ok_or_else(usage),
        _ => Err(usage()),
    }
}

fn usage() -> CliError {
    CliError::Usage(USAGE.to_owned())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NvmExitCode;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn each_reason_prints_its_warning_on_stderr_only_and_succeeds() {
        for reason in RuntimeConflict::ALL {
            let output = run(&arguments(&[reason.name()])).unwrap();
            assert_eq!(output.stderr, runtime_warning(reason));
            assert_eq!(output.stdout, "");
            assert!(!output.blank_stdout);
            assert_eq!(output.status, NvmExitCode::Success);
        }
    }

    #[test]
    fn a_missing_unknown_or_extra_reason_is_a_usage_error() {
        for words in [&[][..], &["nvm.sh"], &["helpers", "function"], &[""]] {
            let error = run(&arguments(words)).expect_err("usage");
            assert!(
                matches!(&error, CliError::Usage(text) if text == USAGE),
                "{words:?}"
            );
        }
    }
}

//! The syntax check a candidate must pass before it replaces a startup file.

use std::cell::Cell;
use std::io;
use std::path::Path;

use crate::domain::migration::syntax_check;
use crate::ports::Process;

/// The syntax checker of one file, run on the candidate before it replaces
/// the file, remembering what happened.
pub struct SyntaxCheck<'a> {
    process: &'a dyn Process,
    command: Option<(&'static str, Vec<&'static str>)>,
    missing: Cell<bool>,
    rejected: Cell<bool>,
}

impl<'a> SyntaxCheck<'a> {
    pub fn new(process: &'a dyn Process, path: &Path) -> Self {
        Self {
            process,
            command: syntax_check(&path.display().to_string()),
            missing: Cell::new(false),
            rejected: Cell::new(false),
        }
    }

    /// Runs the checker on `temporary`: a failure or a non-success status is
    /// an error; a checker that is not installed is skipped.
    pub fn verify(&self, temporary: &Path) -> io::Result<()> {
        let Some((program, flags)) = &self.command else {
            return Ok(());
        };
        let temporary = temporary.to_string_lossy();
        let mut arguments = flags.clone();
        arguments.push(&temporary);
        match self.process.run(Path::new(program), &arguments) {
            Ok(output) if output.success => Ok(()),
            Ok(_) => {
                self.rejected.set(true);
                Err(io::Error::other("the syntax check failed"))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.missing.set(true);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Whether the checker refused the candidate.
    pub fn rejected(&self) -> bool {
        self.rejected.get()
    }

    /// The checker's program, when it was not found.
    pub fn missing(&self) -> Option<&'static str> {
        self.command
            .as_ref()
            .filter(|_| self.missing.get())
            .map(|(program, _)| *program)
    }

    /// `bash -n`, as the messages show it.
    pub fn command_line(&self) -> String {
        self.command
            .as_ref()
            .map(|(program, flags)| format!("{program} {}", flags.join(" ")))
            .unwrap_or_default()
    }
}

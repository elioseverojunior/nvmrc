//! The syntax check a candidate must pass before it replaces a startup file.

use std::cell::{Cell, RefCell};
use std::io;
use std::path::Path;

use crate::domain::migration::syntax_check;
use crate::ports::Process;

/// At most this many lines of what the checker said are shown.
const SHOWN_LINES: usize = 3;

/// The syntax checker of one file, run on the original and on the candidate,
/// remembering what happened.
pub struct SyntaxCheck<'a> {
    process: &'a dyn Process,
    command: Option<(&'static str, Vec<&'static str>)>,
    missing: Cell<bool>,
    rejected: Cell<bool>,
    /// The first lines the checker printed when it last refused a file,
    /// each indented by two spaces.
    said: RefCell<String>,
}

impl<'a> SyntaxCheck<'a> {
    pub fn new(process: &'a dyn Process, path: &Path) -> Self {
        Self {
            process,
            command: syntax_check(&path.display().to_string()),
            missing: Cell::new(false),
            rejected: Cell::new(false),
            said: RefCell::new(String::new()),
        }
    }

    /// Whether the file as it is passes. Only a refusal by the checker
    /// counts against it: a checker that is missing or could not run does
    /// not, so the candidate's check decides.
    pub fn original_passes(&self, original: &Path) -> bool {
        self.check(original).is_ok() || !self.rejected.get()
    }

    /// Runs the checker on `temporary`: a failure or a non-success status is
    /// an error; a checker that is not installed is skipped.
    pub fn verify(&self, temporary: &Path) -> io::Result<()> {
        self.check(temporary)
    }

    fn check(&self, file: &Path) -> io::Result<()> {
        let Some((program, flags)) = &self.command else {
            return Ok(());
        };
        let file = file.to_string_lossy();
        let mut arguments = flags.clone();
        arguments.push(&file);
        match self.process.run(Path::new(program), &arguments) {
            Ok(output) if output.success => Ok(()),
            Ok(output) => {
                self.rejected.set(true);
                *self.said.borrow_mut() = first_lines(&output.stderr);
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

    /// `:` and what the checker said, on the next lines; nothing when it
    /// said nothing.
    pub fn complaint(&self) -> String {
        let said = self.said.borrow();
        if said.is_empty() {
            String::new()
        } else {
            format!(":\n{said}")
        }
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

/// The first [`SHOWN_LINES`] non-blank lines of `stderr`, indented.
fn first_lines(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(SHOWN_LINES)
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

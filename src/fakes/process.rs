use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{Completed, Invocation, Process, ProcessOutput};

/// Programs by path: each prints a fixed output, or fails when it has none.
#[derive(Default)]
pub struct FakeProcess {
    outputs: BTreeMap<PathBuf, ProcessOutput>,
    executions: BTreeMap<(PathBuf, String), Completed>,
    executed: RefCell<Vec<Invocation>>,
}

impl FakeProcess {
    #[must_use]
    pub fn with_output(mut self, program: &str, stdout: &str) -> Self {
        let output = ProcessOutput {
            success: true,
            stdout: stdout.to_owned(),
        };
        self.outputs.insert(PathBuf::from(program), output);
        self
    }

    #[must_use]
    pub fn with_failure(mut self, program: &str) -> Self {
        let output = ProcessOutput {
            success: false,
            stdout: String::new(),
        };
        self.outputs.insert(PathBuf::from(program), output);
        self
    }
}

impl FakeProcess {
    /// What `execute` answers to `program` run with exactly `args` (joined
    /// with spaces); any other invocation is not found.
    #[must_use]
    pub fn with_execution(mut self, program: &str, args: &str, done: Completed) -> Self {
        self.executions
            .insert((PathBuf::from(program), args.to_owned()), done);
        self
    }

    /// A successful execution that printed `stdout`.
    #[must_use]
    pub fn with_success(self, program: &str, args: &str, stdout: &str) -> Self {
        let done = Completed {
            success: true,
            stdout: stdout.to_owned(),
            stderr: String::new(),
        };
        self.with_execution(program, args, done)
    }

    /// Every invocation `execute` was given, in order.
    #[must_use]
    pub fn executed(&self) -> Vec<Invocation> {
        self.executed.borrow().clone()
    }
}

impl Process for FakeProcess {
    fn execute(&self, invocation: &Invocation) -> io::Result<Completed> {
        self.executed.borrow_mut().push(invocation.clone());
        let key = (invocation.program.clone(), invocation.args.join(" "));
        self.executions
            .get(&key)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn run(&self, program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
        self.outputs
            .get(program)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_process_executes_by_program_and_arguments_and_records_what_it_ran() {
        let process = FakeProcess::default().with_success("/n/bin/npm", "--version", "10.2.3\n");
        let invocation = Invocation::new("/n/bin/npm")
            .args(&["--version"])
            .dir("/work");
        assert_eq!(process.execute(&invocation).unwrap().stdout, "10.2.3\n");
        let other = Invocation::new("/n/bin/npm").args(&["list"]);
        assert!(process.execute(&other).is_err());
        assert_eq!(process.executed(), [invocation, other]);
    }

    #[test]
    fn fake_process_answers_by_program_path() {
        let process = FakeProcess::default()
            .with_output("/usr/bin/node", "v22.1.0\n")
            .with_failure("/usr/bin/broken");
        let ok = process
            .run(Path::new("/usr/bin/node"), &["--version"])
            .unwrap();
        assert_eq!((ok.success, ok.stdout.as_str()), (true, "v22.1.0\n"));
        let failed = process.run(Path::new("/usr/bin/broken"), &[]).unwrap();
        assert!(!failed.success);
        assert!(process.run(Path::new("/missing"), &[]).is_err());
    }
}

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{Completed, Invocation, Process, ProcessOutput};

/// Programs by path: each prints a fixed output, or fails when it has none.
#[derive(Default)]
pub struct FakeProcess {
    outputs: BTreeMap<PathBuf, ProcessOutput>,
    runs: BTreeMap<(PathBuf, String), ProcessOutput>,
    executions: BTreeMap<(PathBuf, String), Completed>,
    effects: BTreeMap<(PathBuf, String), Box<dyn Fn()>>,
    executed: RefCell<Vec<Invocation>>,
    spawns: BTreeMap<(PathBuf, String), Result<i32, io::ErrorKind>>,
    spawned: RefCell<Vec<Invocation>>,
    handed_over: RefCell<Vec<Invocation>>,
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
    /// What `run` answers to `program` run with exactly `args` (joined with
    /// spaces), taking precedence over [`Self::with_output`].
    #[must_use]
    pub fn with_run(mut self, program: &str, args: &str, success: bool, stdout: &str) -> Self {
        let output = ProcessOutput {
            success,
            stdout: stdout.to_owned(),
        };
        self.runs
            .insert((PathBuf::from(program), args.to_owned()), output);
        self
    }

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
            ..Completed::default()
        };
        self.with_execution(program, args, done)
    }

    /// Something that happens when `execute` runs `program` with exactly
    /// `args`: a build that leaves files behind, say.
    #[must_use]
    pub fn with_effect(mut self, program: &str, args: &str, effect: impl Fn() + 'static) -> Self {
        self.effects
            .insert((PathBuf::from(program), args.to_owned()), Box::new(effect));
        self
    }

    /// The exit code `spawn` and `hand_over` answer to `program` run with exactly `args`
    /// (joined with spaces); any other spawn is not found.
    #[must_use]
    pub fn with_spawn(mut self, program: &str, args: &str, exit_code: i32) -> Self {
        self.spawns
            .insert((PathBuf::from(program), args.to_owned()), Ok(exit_code));
        self
    }

    /// `spawn` of `program` with exactly `args` fails with `kind`.
    #[must_use]
    pub fn with_spawn_failure(mut self, program: &str, args: &str, kind: io::ErrorKind) -> Self {
        self.spawns
            .insert((PathBuf::from(program), args.to_owned()), Err(kind));
        self
    }

    /// Every invocation `spawn` was given, in order.
    #[must_use]
    pub fn spawned(&self) -> Vec<Invocation> {
        self.spawned.borrow().clone()
    }

    /// Every invocation `hand_over` was given, in order.
    #[must_use]
    pub fn handed_over(&self) -> Vec<Invocation> {
        self.handed_over.borrow().clone()
    }

    /// What `spawn` and `hand_over` answer to `invocation`.
    fn answer(&self, invocation: &Invocation) -> io::Result<i32> {
        let key = (invocation.program.clone(), invocation.args.join(" "));
        match self.spawns.get(&key) {
            Some(answer) => answer.map_err(io::Error::from),
            None => Err(io::Error::from(io::ErrorKind::NotFound)),
        }
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
        if let Some(effect) = self.effects.get(&key) {
            effect();
        }
        self.executions
            .get(&key)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
        let key = (program.to_path_buf(), args.join(" "));
        self.runs
            .get(&key)
            .or_else(|| self.outputs.get(program))
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn spawn(&self, invocation: &Invocation) -> io::Result<i32> {
        self.spawned.borrow_mut().push(invocation.clone());
        self.answer(invocation)
    }

    /// Records the hand-over and answers as [`Self::with_spawn`] says.
    fn hand_over(&self, invocation: &Invocation) -> io::Result<i32> {
        self.handed_over.borrow_mut().push(invocation.clone());
        self.answer(invocation)
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
    fn fake_process_spawns_by_program_and_arguments_and_records_what_it_spawned() {
        let process = FakeProcess::default().with_spawn("/n/bin/node", "-e 1", 7);
        let known = Invocation::new("/n/bin/node").args(&["-e", "1"]);
        assert_eq!(process.spawn(&known).unwrap(), 7);
        let other = Invocation::new("/n/bin/node");
        let error = process.spawn(&other).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(process.spawned(), [known, other]);
        assert!(process.executed().is_empty());
    }

    #[test]
    fn fake_process_runs_the_effect_of_an_invocation() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran = Rc::new(Cell::new(0));
        let seen = Rc::clone(&ran);
        let process = FakeProcess::default()
            .with_success("make", "install", "")
            .with_effect("make", "install", move || seen.set(seen.get() + 1));
        process
            .execute(&Invocation::new("make").args(&["install"]))
            .unwrap();
        assert_eq!(ran.get(), 1);
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

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::ports::{Completed, Invocation, Process, ProcessOutput};
use crate::shell::DESCRIPTOR_VARIABLE;

const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Runs real programs, bounded so that a misbehaving one cannot hang or
/// flood `nvm`: stdin is closed, stderr is discarded, at most `output_cap`
/// bytes of stdout are kept, and a program still running after `timeout` is
/// killed, which fails the run with [`io::ErrorKind::TimedOut`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdProcess {
    pub timeout: Duration,
    pub output_cap: usize,
}

impl Default for StdProcess {
    /// Five seconds and 4096 bytes: plenty for `node --version`.
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            output_cap: 4096,
        }
    }
}

impl Process for StdProcess {
    /// No time limit and no cap: a build may take an hour and print a lot.
    /// Both streams are read at once, so a full pipe cannot stall the program.
    fn execute(&self, invocation: &Invocation) -> io::Result<Completed> {
        let mut command = Command::new(&invocation.program);
        for name in &invocation.env_remove {
            command.env_remove(name);
        }
        command
            .args(&invocation.args)
            .envs(invocation.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &invocation.dir {
            command.current_dir(dir);
        }
        if let Some(prefix) = &invocation.path_prefix {
            command.env("PATH", path_with_prefix(prefix)?);
        }
        let mut child = without_channel(&mut command).spawn()?;
        let stdout = collect(child.stdout.take());
        let stderr = collect(child.stderr.take());
        let status = child.wait()?;
        Ok(Completed {
            success: status.success(),
            code: status.code(),
            stdout: stdout.join().unwrap_or_default(),
            stderr: stderr.join().unwrap_or_default(),
        })
    }

    fn spawn(&self, invocation: &Invocation) -> io::Result<i32> {
        super::std_spawn::spawn_inherited(invocation)
    }

    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
        let deadline = Instant::now() + self.timeout;
        let mut child = without_channel(&mut Command::new(program))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("no stdout pipe"))?;
        let reader = read_in_background(stdout, self.output_cap);
        let status = wait_until(&mut child, deadline)?;
        let bytes = receive_until(&reader, deadline)??;
        Ok(ProcessOutput {
            success: status.success(),
            stdout: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }
}

/// `command` without the variable of the `nvm` function's channel, which is
/// for this process only (its descriptor is close-on-exec already).
pub(super) fn without_channel(command: &mut Command) -> &mut Command {
    command.env_remove(DESCRIPTOR_VARIABLE)
}

/// `prefix` in front of the `PATH` this process has.
fn path_with_prefix(prefix: &Path) -> io::Result<std::ffi::OsString> {
    let current = std::env::var_os("PATH").unwrap_or_default();
    let directories = std::iter::once(prefix.to_path_buf()).chain(std::env::split_paths(&current));
    std::env::join_paths(directories).map_err(io::Error::other)
}

/// Reads a pipe to its end on another thread.
fn collect<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Reads at most `cap` bytes on another thread. The thread is never joined:
/// a grandchild holding the pipe open must not hold `nvm` up with it.
fn read_in_background(stdout: ChildStdout, cap: usize) -> Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = stdout
            .take(u64::try_from(cap).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes);
        // The receiver is gone after a timeout; nobody needs the bytes then.
        sender.send(read.map(|_| bytes)).ok();
    });
    receiver
}

fn wait_until(child: &mut Child, deadline: Instant) -> io::Result<ExitStatus> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            // It may have exited since `try_wait`; `wait` reaps it either way.
            child.kill().ok();
            child.wait()?;
            return Err(timed_out());
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn receive_until<T>(receiver: &Receiver<T>, deadline: Instant) -> io::Result<T> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    receiver.recv_timeout(remaining).map_err(|_| timed_out())
}

fn timed_out() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "the program did not finish in time",
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn shell(process: &StdProcess, script: &str) -> io::Result<ProcessOutput> {
        process.run(Path::new("/bin/sh"), &["-c", script])
    }

    #[test]
    fn execute_takes_the_removed_variables_out() {
        let invocation = Invocation::new("/bin/sh")
            .args(&["-c", "echo \"${HOME-unset}\""])
            .env_remove("HOME");
        let done = StdProcess::default().execute(&invocation).unwrap();
        assert_eq!(done.stdout, "unset\n");
    }

    #[test]
    fn no_program_gets_the_variable_of_the_channel() {
        let show = "echo \"${NVMRC_SCRIPT_FD-none}\"";
        let invocation = Invocation::new("/bin/sh")
            .args(&["-c", show])
            .env("NVMRC_SCRIPT_FD", "3");
        let done = StdProcess::default().execute(&invocation).unwrap();
        assert_eq!(done.stdout, "none\n");
        assert_eq!(
            shell(&StdProcess::default(), show).unwrap().stdout,
            "none\n"
        );
    }

    #[test]
    fn a_program_that_prints_a_version_is_read() {
        let output = shell(&StdProcess::default(), "echo v22.1.0").unwrap();
        assert!(output.success);
        assert_eq!(output.stdout, "v22.1.0\n");
    }

    #[test]
    fn a_failing_program_is_not_a_success() {
        let output = shell(&StdProcess::default(), "echo v1.0.0; exit 3").unwrap();
        assert!(!output.success);
    }

    #[test]
    fn stdin_is_closed() {
        let output = shell(&StdProcess::default(), "cat").unwrap();
        assert_eq!(output.stdout, "");
    }

    #[test]
    fn a_program_that_runs_too_long_is_killed() {
        let process = StdProcess {
            timeout: Duration::from_millis(200),
            ..StdProcess::default()
        };
        let started = Instant::now();
        let error = shell(&process, "sleep 30").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn output_beyond_the_cap_is_dropped() {
        let process = StdProcess {
            output_cap: 16,
            ..StdProcess::default()
        };
        let output = shell(&process, "printf '%0100d' 0").unwrap();
        assert_eq!(output.stdout, "0".repeat(16));
    }

    #[test]
    fn a_missing_program_cannot_run() {
        let missing = Path::new("/nonexistent/node");
        assert!(StdProcess::default().run(missing, &[]).is_err());
    }

    fn execute(script: &str) -> Completed {
        let invocation = Invocation::new("/bin/sh").args(&["-c", script]);
        StdProcess::default().execute(&invocation).unwrap()
    }

    #[test]
    fn execute_returns_both_streams_and_the_outcome() {
        let done = execute("echo out; echo err >&2; exit 3");
        assert_eq!(
            (done.stdout.as_str(), done.stderr.as_str()),
            ("out\n", "err\n")
        );
        assert!(!done.success);
        assert!(execute("true").success);
    }

    #[test]
    fn execute_has_no_output_cap_and_no_deadlock_on_a_full_pipe() {
        let done = execute(
            "head -c 300000 /dev/zero | tr '\\0' a; head -c 300000 /dev/zero | tr '\\0' b >&2",
        );
        assert_eq!((done.stdout.len(), done.stderr.len()), (300_000, 300_000));
    }

    #[test]
    fn execute_runs_where_it_is_told_with_the_environment_it_is_given() {
        let root = tempfile::tempdir().unwrap();
        let invocation = Invocation::new("/bin/sh")
            .args(&["-c", "pwd; echo $GREETING"])
            .dir(root.path())
            .env("GREETING", "hello");
        let done = StdProcess::default().execute(&invocation).unwrap();
        let lines: Vec<&str> = done.stdout.lines().collect();
        let real = std::fs::canonicalize(root.path()).unwrap();
        assert_eq!(std::path::PathBuf::from(lines[0]), real);
        assert_eq!(lines[1], "hello");
    }

    #[test]
    fn execute_puts_the_prefix_first_on_the_path() {
        let root = tempfile::tempdir().unwrap();
        let tool = root.path().join("mytool");
        std::fs::write(&tool, "#!/bin/sh\necho from-prefix\n").unwrap();
        std::fs::set_permissions(&tool, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let invocation = Invocation::new("/bin/sh")
            .args(&["-c", "mytool"])
            .path_prefix(root.path());
        let done = StdProcess::default().execute(&invocation).unwrap();
        assert_eq!(done.stdout, "from-prefix\n");
    }

    #[test]
    fn execute_of_a_missing_program_is_an_error() {
        let invocation = Invocation::new("/nonexistent/npm");
        assert!(StdProcess::default().execute(&invocation).is_err());
    }
}

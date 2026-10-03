use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::ports::{Process, ProcessOutput};

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
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
        let deadline = Instant::now() + self.timeout;
        let mut child = Command::new(program)
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
}

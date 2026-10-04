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
    let done =
        execute("head -c 300000 /dev/zero | tr '\\0' a; head -c 300000 /dev/zero | tr '\\0' b >&2");
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
    std::fs::set_permissions(&tool, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
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

#[test]
fn execute_reports_128_plus_the_signal_that_killed_the_program() {
    let done = execute("kill -9 $$");
    assert_eq!((done.success, done.code), (false, Some(137)));
}

#[test]
fn run_keeps_what_the_program_said_on_stderr() {
    let output = shell(&StdProcess::default(), "echo oops >&2; exit 2").unwrap();
    assert_eq!((output.success, output.stderr.as_str()), (false, "oops\n"));
}

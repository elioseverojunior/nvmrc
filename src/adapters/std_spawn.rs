use std::io;

use super::std_process::without_channel;
use crate::ports::Invocation;

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// Runs `invocation` with inherited stdio and waits for it.
pub(super) fn spawn_inherited(invocation: &Invocation) -> io::Result<i32> {
    Ok(exit_code(command_for(invocation)?.status()?))
}

/// Replaces this process with `invocation`; returns only the error that
/// kept it from starting.
pub(super) fn exec_replacing(invocation: &Invocation) -> io::Result<i32> {
    use std::os::unix::process::CommandExt;
    Err(command_for(invocation)?.exec())
}

/// The command `invocation` describes, with stdio inherited.
fn command_for(invocation: &Invocation) -> io::Result<Command> {
    let mut command = Command::new(&invocation.program);
    for name in &invocation.env_remove {
        command.env_remove(name);
    }
    command
        .args(&invocation.args)
        .envs(invocation.env.iter().map(|(name, value)| (name, value)));
    if let Some(dir) = &invocation.dir {
        command.current_dir(dir);
    }
    if let Some(prefix) = &invocation.path_prefix {
        command.env("PATH", path_with_prefix(prefix, invocation)?);
    }
    without_channel(&mut command);
    Ok(command)
}

/// `prefix` in front of the `PATH` the child ends up with: the `env` entry
/// named `PATH` when there is one (the last wins), else this process's.
fn path_with_prefix(prefix: &Path, invocation: &Invocation) -> io::Result<OsString> {
    let replaced = invocation
        .env
        .iter()
        .rev()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| OsString::from(value));
    let current = replaced
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default();
    let directories = std::iter::once(prefix.to_path_buf()).chain(std::env::split_paths(&current));
    std::env::join_paths(directories).map_err(io::Error::other)
}

pub(super) fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(1)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::adapters::std_process::StdProcess;
    use crate::ports::Process;

    fn sh(script: &str) -> Invocation {
        Invocation::new("/bin/sh").args(&["-c", script])
    }

    #[test]
    fn the_exit_code_is_propagated() {
        assert_eq!(StdProcess::default().spawn(&sh("exit 7")).unwrap(), 7);
        assert_eq!(StdProcess::default().spawn(&sh("true")).unwrap(), 0);
    }

    #[test]
    fn a_signal_gives_128_plus_its_number() {
        let code = StdProcess::default().spawn(&sh("kill -9 $$")).unwrap();
        assert_eq!(code, 137);
    }

    #[test]
    fn a_missing_program_is_not_found() {
        let invocation = Invocation::new("/nonexistent/node");
        let error = StdProcess::default().spawn(&invocation).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn it_runs_where_told_with_the_given_environment() {
        let root = tempfile::tempdir().unwrap();
        let invocation = sh("echo \"$GREETING\" > out; pwd -P >> out")
            .dir(root.path())
            .env("GREETING", "hello");
        assert_eq!(StdProcess::default().spawn(&invocation).unwrap(), 0);
        let written = std::fs::read_to_string(root.path().join("out")).unwrap();
        let real = std::fs::canonicalize(root.path()).unwrap();
        assert_eq!(written, format!("hello\n{}\n", real.display()));
    }

    #[test]
    fn an_env_path_replaces_the_inherited_path() {
        let root = tempfile::tempdir().unwrap();
        let out = root.path().join("out");
        let script = format!("echo \"$PATH\" > {}", out.display());
        let invocation = sh(&script).env("PATH", "/only/here:/bin");
        // `echo` is a shell builtin, so the replaced PATH is enough to run.
        assert_eq!(StdProcess::default().spawn(&invocation).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(out).unwrap(), "/only/here:/bin\n");
    }

    #[test]
    fn a_bare_program_is_looked_up_on_the_env_path() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let program = bin.join("only-here-xyz");
        std::fs::write(&program, "#!/bin/sh\nexit 9\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!("{}:/usr/bin:/bin", bin.display());
        let invocation = Invocation::new("only-here-xyz").env("PATH", &path);
        assert_eq!(StdProcess::default().spawn(&invocation).unwrap(), 9);
    }

    #[test]
    fn removed_variables_are_not_inherited() {
        let root = tempfile::tempdir().unwrap();
        let out = root.path().join("out");
        let script = format!("echo \"${{HOME-unset}}\" > {}", out.display());
        let invocation = sh(&script).env_remove("HOME");
        assert_eq!(StdProcess::default().spawn(&invocation).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(out).unwrap(), "unset\n");
    }

    #[test]
    fn the_variable_of_the_channel_is_never_inherited() {
        let root = tempfile::tempdir().unwrap();
        let out = root.path().join("out");
        let script = format!("echo \"${{NVMRC_SCRIPT_FD-none}}\" > {}", out.display());
        let invocation = sh(&script).env("NVMRC_SCRIPT_FD", "3");
        assert_eq!(StdProcess::default().spawn(&invocation).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(out).unwrap(), "none\n");
    }

    #[test]
    fn the_prefix_goes_in_front_of_an_env_path() {
        let root = tempfile::tempdir().unwrap();
        let out = root.path().join("out");
        let script = format!("echo \"$PATH\" > {}", out.display());
        let invocation = sh(&script).env("PATH", "/a:/b").path_prefix("/prefix");
        assert_eq!(StdProcess::default().spawn(&invocation).unwrap(), 0);
        assert_eq!(std::fs::read_to_string(out).unwrap(), "/prefix:/a:/b\n");
    }

    #[test]
    fn the_prefix_goes_in_front_of_the_inherited_path_without_an_env_path() {
        let root = tempfile::tempdir().unwrap();
        let out = root.path().join("out");
        let script = format!("echo \"$PATH\" > {}", out.display());
        let invocation = sh(&script).path_prefix("/prefix");
        assert_eq!(StdProcess::default().spawn(&invocation).unwrap(), 0);
        let written = std::fs::read_to_string(out).unwrap();
        assert!(written.starts_with("/prefix:"));
    }
}

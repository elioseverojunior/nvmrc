use super::*;
use crate::shell::Script;

fn script_output(stdout: &str, stderr: &str) -> Output {
    let script = Script::new().unset("NVM_BIN").unwrap();
    Output::stdout(stdout)
        .with_stderr(stderr)
        .with_script(script)
}

fn deliver(output: &Output, fs: &FakeFileSystem, descriptor: Option<&str>) -> (u8, String, String) {
    let env = match descriptor {
        Some(value) => FakeEnv::default().with_var("NVMRC_SCRIPT_FD", value),
        None => FakeEnv::default(),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = finish(output, &Context::new(fs, &env), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn with_a_descriptor_the_code_is_written_to_it_and_the_streams_pass_through() {
    let fs = FakeFileSystem::default();
    let result = deliver(&script_output("o", "e"), &fs, Some("3"));
    assert_eq!(result, (0, "o\n".into(), "e\n".into()));
    let written = fs.read_to_string(Path::new("/dev/fd/3")).unwrap();
    assert_eq!(written, "unset NVM_BIN\n");
}

#[test]
fn without_a_descriptor_the_code_goes_to_stdout_and_the_text_to_stderr() {
    let fs = FakeFileSystem::default();
    let result = deliver(&script_output("o", "e"), &fs, None);
    assert_eq!(result, (0, "unset NVM_BIN\n".into(), "e\no\n".into()));
    assert!(!fs.is_file(Path::new("/dev/fd/3")));
}

#[test]
fn an_empty_script_changes_nothing_in_either_mode() {
    let fs = FakeFileSystem::default();
    for descriptor in [None, Some("3")] {
        let result = deliver(&Output::stdout("o").with_stderr("e"), &fs, descriptor);
        assert_eq!(result, (0, "o\n".into(), "e\n".into()));
    }
    assert!(!fs.is_file(Path::new("/dev/fd/3")));
}

#[test]
fn a_descriptor_that_is_not_decimal_digits_is_ignored() {
    let fs = FakeFileSystem::default();
    for bad in ["", "x", "3x", "-1", "../etc"] {
        let result = deliver(&script_output("o", ""), &fs, Some(bad));
        assert_eq!(result, (0, "unset NVM_BIN\n".into(), "o\n".into()), "{bad}");
    }
}

#[test]
fn a_failed_write_is_reported_and_makes_the_status_one() {
    let fs = FakeFileSystem::default().with_unwritable("/dev/fd/3");
    let (code, out, err) = deliver(&script_output("o", "e"), &fs, Some("3"));
    assert_eq!((code, out.as_str()), (1, "o\n"));
    assert!(
        err.starts_with("e\nnvm: cannot hand the shell code over"),
        "{err}"
    );
}

#[test]
fn a_failed_write_keeps_the_status_of_a_command_that_already_failed() {
    let fs = FakeFileSystem::default().with_unwritable("/dev/fd/3");
    let output = script_output("", "e").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(deliver(&output, &fs, Some("3")).0, 3);
}

#[test]
fn use_and_deactivate_are_commands_of_the_cli() {
    let (code, out, err) = run_cli(&["nvm", "use", "99"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.contains("99"), "{err}");
    let (code, out, _) = run_cli(&["nvm", "deactivate"]);
    assert_eq!(code, 0);
    assert!(out.contains("unset NVM_BIN") || out.is_empty(), "{out}");
}

#[test]
fn use_passes_hyphenated_arguments_through() {
    let (code, _, err) = run_cli(&["nvm", "use", "--silent", "--lts=nope", "-w", "99"]);
    assert_ne!(code, 127, "{err}");
}

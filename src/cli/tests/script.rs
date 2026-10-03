use super::*;
use crate::fakes::FakeScriptChannel;
use crate::shell::Script;

fn script_output(stdout: &str, stderr: &str) -> Output {
    let script = Script::new().unset("NVM_BIN").unwrap();
    Output::stdout(stdout)
        .with_stderr(stderr)
        .with_script(script)
}

fn deliver(output: &Output, channel: Option<&FakeScriptChannel>) -> (u8, String, String) {
    deliver_in(&FakeEnv::default(), output, channel)
}

fn deliver_in(
    env: &FakeEnv,
    output: &Output,
    channel: Option<&FakeScriptChannel>,
) -> (u8, String, String) {
    let fs = FakeFileSystem::default();
    let context = Context::new(&fs, env);
    let context = match channel {
        Some(channel) => context.with_script_channel(channel),
        None => context,
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = finish("nvm", output, &context, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn with_a_channel_the_code_is_sent_to_it_and_the_streams_pass_through() {
    let channel = FakeScriptChannel::default();
    let result = deliver(&script_output("o", "e"), Some(&channel));
    assert_eq!(result, (0, "o\n".into(), "e\n".into()));
    assert_eq!(channel.sent(), "unset NVM_BIN\n");
}

#[test]
fn without_a_channel_the_code_goes_to_stdout_and_the_text_to_stderr() {
    let result = deliver(&script_output("o", "e"), None);
    assert_eq!(result, (0, "unset NVM_BIN\n".into(), "e\no\n".into()));
}

#[test]
fn the_fish_function_gets_fish_code_on_its_channel() {
    let channel = FakeScriptChannel::default();
    let env = FakeEnv::default().with_var("NVMRC_SHELL_KIND", "fish");
    let result = deliver_in(&env, &script_output("o", "e"), Some(&channel));
    assert_eq!(result, (0, "o\n".into(), "e\n".into()));
    assert_eq!(channel.sent(), "set -e NVM_BIN\n");
}

#[test]
fn standalone_the_code_is_posix_unless_the_kind_says_fish() {
    let fish = FakeEnv::default().with_var("NVMRC_SHELL_KIND", "fish");
    let result = deliver_in(&fish, &script_output("o", ""), None);
    assert_eq!(result, (0, "set -e NVM_BIN\n".into(), "o\n".into()));
    let other = FakeEnv::default().with_var("NVMRC_SHELL_KIND", "zsh");
    let result = deliver_in(&other, &script_output("o", ""), None);
    assert_eq!(result.1, "unset NVM_BIN\n");
}

#[test]
fn an_empty_script_changes_nothing_in_either_mode() {
    let channel = FakeScriptChannel::default();
    for channel in [None, Some(&channel)] {
        let result = deliver(&Output::stdout("o").with_stderr("e"), channel);
        assert_eq!(result, (0, "o\n".into(), "e\n".into()));
    }
    assert_eq!(channel.sent(), "");
}

#[test]
fn the_variable_alone_opens_no_channel() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVMRC_SCRIPT_FD", "3");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let output = script_output("o", "");
    let code = finish("nvm", &output, &Context::new(&fs, &env), &mut out, &mut err);
    assert_eq!(
        (code, out, err),
        (0, b"unset NVM_BIN\n".to_vec(), b"o\n".to_vec())
    );
}

#[test]
fn a_failed_send_is_reported_and_makes_the_status_one() {
    let channel = FakeScriptChannel::broken();
    let (code, out, err) = deliver(&script_output("o", "e"), Some(&channel));
    assert_eq!((code, out.as_str()), (1, "o\n"));
    assert!(
        err.starts_with("e\nnvm: cannot hand the shell code over"),
        "{err}"
    );
}

#[test]
fn a_failed_send_keeps_the_status_of_a_command_that_already_failed() {
    let channel = FakeScriptChannel::broken();
    let output = script_output("", "e").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(deliver(&output, Some(&channel)).0, 3);
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

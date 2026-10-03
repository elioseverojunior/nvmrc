use super::*;
use crate::fakes::FakeEnv;
use std::process::Command;

#[test]
fn quote_table() {
    let cases = [
        ("", "''"),
        ("a b", "'a b'"),
        ("it's", r"'it'\''s'"),
        ("'", r"''\'''"),
        ("$HOME", "'$HOME'"),
        ("`x`", "'`x`'"),
        ("say \"hi\"", "'say \"hi\"'"),
        ("a\nb", "'a\nb'"),
        (r"a\b", r"'a\b'"),
        ("caf\u{e9} \u{1f600}", "'caf\u{e9} \u{1f600}'"),
    ];
    for (input, expected) in cases {
        assert_eq!(quote(input), expected, "input {input:?}");
    }
}

#[test]
fn render_sample_script() -> Result<(), ShellError> {
    let script = Script::new()
        .export("PATH", "/a b:/c")?
        .unset("NVM_BIN")?
        .hash_reset();
    assert_eq!(
        script.render(),
        "export PATH='/a b:/c'\nunset NVM_BIN\nhash -r 2>/dev/null || true\n"
    );
    Ok(())
}

#[test]
fn empty_script_renders_nothing() {
    assert!(Script::new().is_empty());
    assert_eq!(Script::new().render(), "");
}

#[test]
fn invalid_names_are_rejected() {
    for name in ["", "1A", "A-B", "A B", "A=B", "A;rm", "é", "A\n"] {
        assert_eq!(
            Script::new().export(name, "v"),
            Err(ShellError::InvalidName(name.to_string())),
            "export {name:?}"
        );
        assert_eq!(
            Script::new().unset(name),
            Err(ShellError::InvalidName(name.to_string())),
            "unset {name:?}"
        );
    }
}

#[test]
fn valid_names_are_accepted() {
    assert!(Script::new().export("_a1", "v").is_ok());
    assert!(Script::new().unset("NVM_BIN").is_ok());
}

#[test]
fn append_keeps_order_and_is_empty_tracks_content() -> Result<(), ShellError> {
    let first = Script::new().export("A", "1")?;
    let second = Script::new().unset("B")?;
    let joined = first.append(second);
    assert!(!joined.is_empty());
    assert_eq!(joined.render(), "export A='1'\nunset B\n");
    assert_eq!(Script::new().append(Script::new()), Script::new());
    Ok(())
}

const NASTY: &str = "a b $HOME `x` \"q\" 'single' \\ back\nline2 caf\u{e9} \u{1f600};&|<>*?";

fn round_trip(shell: &str) -> Result<(), ShellError> {
    let Ok(probe) = Command::new(shell).args(["-c", "exit 0"]).output() else {
        eprintln!("skipping: {shell} is not installed");
        return Ok(());
    };
    assert!(probe.status.success());
    let rendered = Script::new().export("NVMRC_TEST_VALUE", NASTY)?.render();
    let program = format!("{rendered}printf %s \"$NVMRC_TEST_VALUE\"");
    let output = Command::new(shell)
        .args(["-c", &program])
        .output()
        .expect("shell runs");
    assert!(output.status.success(), "{shell} failed");
    assert_eq!(output.stdout, NASTY.as_bytes(), "{shell} round trip");
    Ok(())
}

#[test]
fn round_trip_sh() -> Result<(), ShellError> {
    round_trip("sh")
}

#[test]
fn round_trip_bash() -> Result<(), ShellError> {
    round_trip("bash")
}

#[test]
fn round_trip_zsh() -> Result<(), ShellError> {
    round_trip("zsh")
}

#[test]
fn unset_and_hash_reset_run_in_a_shell() -> Result<(), ShellError> {
    let rendered = Script::new()
        .export("NVMRC_TEST_VALUE", "x")?
        .unset("NVMRC_TEST_VALUE")?
        .hash_reset()
        .render();
    let program = format!("{rendered}printf %s \"${{NVMRC_TEST_VALUE-gone}}\"");
    let output = Command::new("sh")
        .args(["-c", &program])
        .output()
        .expect("sh");
    assert_eq!(output.stdout, b"gone");
    Ok(())
}

#[test]
fn fish_quote_table() {
    let cases = [
        ("", "''"),
        ("a b", "'a b'"),
        ("it's", r"'it\'s'"),
        ("'", r"'\''"),
        (r"a\b", r"'a\\b'"),
        (r"\'", r"'\\\''"),
        ("$HOME", "'$HOME'"),
        ("(echo x)", "'(echo x)'"),
        ("say \"hi\"", "'say \"hi\"'"),
        ("a\nb", "'a\nb'"),
        ("caf\u{e9} \u{1f600}", "'caf\u{e9} \u{1f600}'"),
    ];
    for (input, expected) in cases {
        assert_eq!(fish::quote(input), expected, "input {input:?}");
    }
}

#[test]
fn a_use_script_renders_as_fish() -> Result<(), ShellError> {
    let script = Script::new()
        .export("MANPATH", "/v/share/man:/m a:")?
        .export("PATH", "/v/bin:/a b:/c")?
        .hash_reset()
        .export("NVM_BIN", "/v/bin")?;
    assert_eq!(
        script.render_in(Dialect::Fish),
        "set -gx MANPATH '/v/share/man:/m a:'\nset -gx PATH '/v/bin:/a b:/c'\n\
         set -gx NVM_BIN '/v/bin'\n"
    );
    assert_eq!(script.render_in(Dialect::Posix), script.render());
    Ok(())
}

#[test]
fn a_deactivate_script_renders_as_fish() -> Result<(), ShellError> {
    let script = Script::new()
        .export("PATH", "/usr/bin")?
        .unset("NVM_BIN")?
        .unset("NVM_INC")?;
    assert_eq!(
        script.render_in(Dialect::Fish),
        "set -gx PATH '/usr/bin'\nset -e NVM_BIN\nset -e NVM_INC\n"
    );
    Ok(())
}

#[test]
fn the_dialect_is_fish_only_when_the_function_says_so() {
    let kind =
        |value: &str| Dialect::from_env(&FakeEnv::default().with_var(DIALECT_VARIABLE, value));
    assert_eq!(DIALECT_VARIABLE, "NVMRC_SHELL_KIND");
    assert_eq!(kind("fish"), Dialect::Fish);
    for other in ["", "bash", "FISH", "fish "] {
        assert_eq!(kind(other), Dialect::Posix, "{other:?}");
    }
    assert_eq!(Dialect::from_env(&FakeEnv::default()), Dialect::Posix);
}

/// Runs `program` in a real fish (`None` when fish is not installed).
fn in_fish(program: &str) -> Option<Vec<u8>> {
    let output = Command::new("fish")
        .args(["--no-config", "-c", program])
        .output()
        .ok()?;
    assert!(output.status.success(), "fish failed on {program:?}");
    Some(output.stdout)
}

#[test]
fn round_trip_fish() -> Result<(), ShellError> {
    let script = Script::new()
        .export("NVMRC_TEST_VALUE", NASTY)?
        .export("NVMRC_TEST_EMPTY", "")?;
    let program = format!(
        "{}printf '%s|%s' \"$NVMRC_TEST_VALUE\" \"$NVMRC_TEST_EMPTY\"",
        script.render_in(Dialect::Fish)
    );
    match in_fish(&program) {
        Some(stdout) => assert_eq!(stdout, format!("{NASTY}|").as_bytes()),
        None => eprintln!("skipping: fish is not installed"),
    }
    Ok(())
}

#[test]
fn fish_path_variables_keep_empty_entries_spaces_and_a_trailing_colon() -> Result<(), ShellError> {
    let value = "/v/share/man:/m a::/n:";
    let script = Script::new()
        .export("MANPATH", value)?
        .export("NVM_BIN", "x")?
        .unset("NVM_BIN")?
        .hash_reset();
    let program = format!(
        "{}printf '%s|%s|' \"$MANPATH\" (count $MANPATH); env | string match 'MANPATH=*'; \
         set -q NVM_BIN; or echo gone",
        script.render_in(Dialect::Fish)
    );
    match in_fish(&program) {
        Some(stdout) => assert_eq!(
            stdout,
            format!("{value}|5|MANPATH={value}\ngone\n").as_bytes()
        ),
        None => eprintln!("skipping: fish is not installed"),
    }
    Ok(())
}

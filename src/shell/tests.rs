use super::*;
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

use super::*;
use crate::fakes::FakeScriptChannel;

const PLAIN: &str = "Setting colors to: r g b c m\n\
WARNING: Colors may not display because they are not supported in this shell.\n";
const INVALID: &str = "\x1b[1;37mPlease pass in five \x1b[1;31mvalid color codes\x1b[1;37m. \
Choose from: rRgGbBcCyYmMkKeW\x1b[0m\n";

fn in_the_function(args: &[&str]) -> (u8, String, String, String) {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default();
    let channel = FakeScriptChannel::default();
    let context = Context::new(&fs, &env).with_script_channel(&channel);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args, &context, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
        channel.sent(),
    )
}

#[test]
fn set_colors_hands_the_export_to_the_function_and_confirms_on_stdout() {
    let result = in_the_function(&["nvm", "set-colors", "rgbcm"]);
    let expected = (
        0,
        PLAIN.into(),
        String::new(),
        "export NVM_COLORS='rgbcm'\n".into(),
    );
    assert_eq!(result, expected);
}

#[test]
fn standalone_set_colors_prints_the_export_and_moves_the_text_to_stderr() {
    let result = run_cli(&["nvm", "set-colors", "rgbcm"]);
    assert_eq!(
        result,
        (0, "export NVM_COLORS='rgbcm'\n".into(), PLAIN.into())
    );
}

#[test]
fn an_invalid_set_colors_prints_a_blank_line_and_the_error_with_status_0() {
    for args in [
        &["nvm", "set-colors"][..],
        &["nvm", "set-colors", "rgb"],
        &["nvm", "set-colors", "--no-colors"],
    ] {
        let result = in_the_function(args);
        let expected = (0, "\n".into(), INVALID.into(), String::new());
        assert_eq!(result, expected, "{args:?}");
        assert_eq!(run_cli(args), (0, "\n".into(), INVALID.into()), "{args:?}");
    }
}

#[test]
fn a_double_dash_is_the_setting_as_in_nvm_sh_and_so_invalid() {
    let result = in_the_function(&["nvm", "set-colors", "--", "rgbcm"]);
    assert_eq!(result, (0, "\n".into(), INVALID.into(), String::new()));
}

#[test]
fn a_blank_stdout_line_is_printed_only_when_the_text_is_empty() {
    let blank = Output::default().with_blank_stdout();
    assert_eq!(deliver(&blank), (0, "\n".into(), String::new()));
    let text = Output::stdout("o").with_blank_stdout();
    assert_eq!(deliver(&text), (0, "o\n".into(), String::new()));
}

fn deliver(output: &Output) -> (u8, String, String) {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = finish("nvm", output, &Context::new(&fs, &env), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

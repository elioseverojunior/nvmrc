//! The colored version rows, against the golden `nvm ls` of the digest
//! (fixture: node v18.20.4, v20.11.1, v22.3.0, io.js v3.3.1, a system node
//! v16.0.0).

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
const IN_USE: &str = "/n/versions/node/v20.11.1/bin";

fn versions_only() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v18.20.4/bin/node", "")
        .with_file("/n/versions/node/v20.11.1/bin/node", "")
        .with_file("/n/versions/node/v22.3.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.3.1/bin/node", "")
        .with_file("/n/alias/default", "18")
        .with_file(TPUT, "")
}

fn fixture() -> FakeFileSystem {
    versions_only().with_file("/sys/node", "")
}

struct Setup<'a> {
    path: &'a str,
    term: &'a str,
    colors: Option<&'a str>,
    terminal: FakeTerminal,
}

impl<'a> Setup<'a> {
    fn tty() -> Self {
        Self {
            path: IN_USE,
            term: "xterm-256color",
            colors: None,
            terminal: FakeTerminal::terminal(),
        }
    }

    fn path(self, path: &'a str) -> Self {
        Self { path, ..self }
    }

    fn colors(self, colors: &'a str) -> Self {
        Self {
            colors: Some(colors),
            ..self
        }
    }

    fn run(&self, fs: &FakeFileSystem, args: &[&str]) -> Output {
        let process = FakeProcess::default()
            .with_output("/sys/node", "v16.0.0\n")
            .with_run(TPUT, "-T xterm-256color colors", true, "256\n")
            .with_run(TPUT, "-T dumb colors", true, "-1\n");
        let path = format!("{}:/sys:/usr/bin", self.path);
        let mut env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", &path)
            .with_var("TERM", self.term);
        if let Some(colors) = self.colors {
            env = env.with_var("NVM_COLORS", colors);
        }
        let context = Context::new(fs, &env)
            .with_process(&process)
            .with_terminal(&self.terminal);
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        run_command(&context, &args).unwrap()
    }
}

fn rows(lines: &[&str]) -> String {
    lines.join("\n")
}

const PLAIN: [&str; 5] = [
    "    iojs-v3.3.1 *",
    "       v18.20.4 *",
    "->     v20.11.1 *",
    "        v22.3.0 *",
    "         system * (-> v16.0.0)",
];

#[test]
fn the_default_palette_colors_every_version_row() {
    let output = Setup::tty().run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;34m       v18.20.4\x1b[0m",
        "\x1b[0;32m->     v20.11.1\x1b[0m",
        "\x1b[0;34m        v22.3.0\x1b[0m",
        "\x1b[0;33m         system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)",
    ]);
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn nvm_colors_picks_the_role_colors() {
    let output = Setup::tty()
        .colors("rgbcm")
        .run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;31m       v18.20.4\x1b[0m",
        "\x1b[0;34m->     v20.11.1\x1b[0m",
        "\x1b[0;31m        v22.3.0\x1b[0m",
        "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)",
    ]);
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn a_pipe_dumb_term_or_no_colors_flag_gives_the_plain_rows() {
    let pipe = Setup {
        terminal: FakeTerminal::pipe(),
        ..Setup::tty()
    };
    let dumb = Setup {
        term: "dumb",
        ..Setup::tty()
    };
    let expected = Output::stdout(rows(&PLAIN));
    assert_eq!(pipe.run(&fixture(), &["--no-alias"]), expected);
    assert_eq!(dumb.run(&fixture(), &["--no-alias"]), expected);
    let flag_last = Setup::tty().run(&fixture(), &["--no-alias", "--no-colors"]);
    assert_eq!(flag_last, expected);
    let flag_first = Setup::tty().run(&fixture(), &["--no-colors", "--no-alias"]);
    assert_eq!(flag_first, expected);
}

#[test]
fn nvm_no_colors_in_the_environment_is_ignored() {
    let fs = fixture();
    let process = FakeProcess::default()
        .with_output("/sys/node", "v16.0.0\n")
        .with_run(TPUT, "-T xterm-256color colors", true, "256\n");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin")
        .with_var("TERM", "xterm-256color")
        .with_var("NVM_NO_COLORS", "--no-colors");
    let terminal = FakeTerminal::terminal();
    let context = Context::new(&fs, &env)
        .with_process(&process)
        .with_terminal(&terminal);
    let output = run(&context, Some("18"), false).unwrap();
    assert_eq!(output, Output::stdout("\x1b[0;34m       v18.20.4\x1b[0m"));
}

#[test]
fn the_current_system_row_is_current_with_a_system_target() {
    let output = Setup::tty().path("/sys").run(&fixture(), &["--no-alias"]);
    let last = output.stdout.lines().last().unwrap();
    assert_eq!(
        last,
        "\x1b[0;32m->       system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)"
    );
    assert!(output.stdout.contains("\x1b[0;34m       v20.11.1\x1b[0m\n"));
}

#[test]
fn a_current_io_js_gets_the_current_color() {
    let in_use = "/n/versions/io.js/v3.3.1/bin";
    let output = Setup::tty().path(in_use).run(&fixture(), &["--no-alias"]);
    let first = output.stdout.lines().next().unwrap();
    assert_eq!(first, "\x1b[0;32m->  iojs-v3.3.1\x1b[0m");
}

#[test]
fn with_no_node_in_use_there_is_no_arrow_and_no_system_row() {
    let fs = versions_only();
    let output = Setup::tty().path("/nowhere").run(&fs, &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;34m       v18.20.4\x1b[0m",
        "\x1b[0;34m       v20.11.1\x1b[0m",
        "\x1b[0;34m        v22.3.0\x1b[0m",
    ]);
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn the_not_available_row_stays_plain_with_status_3() {
    let output = Setup::tty().run(&fixture(), &["99"]);
    let expected = Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(output, expected);
}

#[test]
fn a_short_setting_loses_the_current_arrow_and_warns_once() {
    let output = Setup::tty().colors("rg").run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;31m       v18.20.4\x1b[0m",
        "       v20.11.1",
        "\x1b[0;31m        v22.3.0\x1b[0m",
        "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)",
    ]);
    let warning = "Invalid color code: ";
    assert_eq!(output, Output::stdout(expected).with_stderr(warning));
}

#[test]
fn an_invalid_setting_gives_plain_rows_without_markers_and_warns_once() {
    let output = Setup::tty()
        .colors("zzzzz")
        .run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "    iojs-v3.3.1",
        "       v18.20.4",
        "       v20.11.1",
        "        v22.3.0",
        "         system (-> v16.0.0)",
    ]);
    let warning = "Invalid color code: z";
    assert_eq!(output, Output::stdout(expected).with_stderr(warning));
}

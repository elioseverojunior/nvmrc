//! The colored listing, against the golden `nvm ls-remote` blocks of the
//! digest (fixture: node v18.20.4, v20.11.1, v22.3.0 and io.js v3.3.1
//! installed, `default -> 18`, `myalias -> 20`, the digest mirror).

use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
const IN_USE: &str = "/n/versions/node/v20.11.1/bin";

fn mirror() -> FakeHttp {
    let node = index_text(&[
        ("v22.3.0", "-"),
        ("v22.2.0", "-"),
        ("v20.11.1", "Iron"),
        ("v20.11.0", "Iron"),
        ("v18.20.4", "Hydrogen"),
        ("v4.0.0", "-"),
        ("v0.12.18", "-"),
        ("v0.11.16", "-"),
    ]);
    let iojs = index_text(&[("v3.3.1", "-"), ("v3.3.0", "-")]);
    FakeHttp::default()
        .with_body("https://nodejs.org/dist/index.tab", &node)
        .with_body("https://iojs.org/dist/index.tab", &iojs)
}

fn fixture() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v18.20.4/bin/node", "")
        .with_file("/n/versions/node/v20.11.1/bin/node", "")
        .with_file("/n/versions/node/v22.3.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.3.1/bin/node", "")
        .with_file("/n/alias/default", "18\n")
        .with_file("/n/alias/myalias", "20\n")
        .with_file("/sys/node", "")
        .with_file(TPUT, "")
}

struct Setup<'a> {
    path: &'a str,
    term: &'a str,
    variables: Vec<(&'a str, &'a str)>,
    terminal: FakeTerminal,
}

impl<'a> Setup<'a> {
    fn tty() -> Self {
        Self {
            path: IN_USE,
            term: "xterm-256color",
            variables: Vec::new(),
            terminal: FakeTerminal::terminal(),
        }
    }

    fn var(mut self, name: &'a str, value: &'a str) -> Self {
        self.variables.push((name, value));
        self
    }

    fn process() -> FakeProcess {
        FakeProcess::default()
            .with_output("/sys/node", "v16.0.0\n")
            .with_run(TPUT, "-T xterm-256color colors", true, "256\n")
            .with_run(TPUT, "-T xterm-256color sitm", true, "")
            .with_run(TPUT, "-T screen colors", true, "8\n")
            .with_run(TPUT, "-T screen sitm", false, "")
    }

    fn run(&self, line: &str) -> Output {
        let (fs, http, process) = (fixture(), mirror(), Self::process());
        let path = format!("{}:/sys:/usr/bin", self.path);
        let mut env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", &path)
            .with_var("TERM", self.term);
        for (name, value) in &self.variables {
            env = env.with_var(name, value);
        }
        let context = Context::new(&fs, &env)
            .with_http(&http)
            .with_process(&process)
            .with_terminal(&self.terminal);
        let args: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        run(&context, &args).unwrap()
    }
}

fn joined(lines: &[&str]) -> String {
    lines.join("\n")
}

const GOLDEN_COLORED: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
    "         v4.0.0",
    "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m                   \x1b[0;32m (Aliases: \x1b[3mdefault\x1b[23m)\x1b[0m",
    "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
    "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m                       \x1b[0;32m (Aliases: \x1b[3mmyalias\x1b[23m)\x1b[0m",
    "        v22.2.0",
    "\x1b[0;34m        v22.3.0\x1b[0m                           \x1b[0;32m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m",
];

const GOLDEN_PLAIN: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "    iojs-v3.3.1 *",
    "         v4.0.0",
    "       v18.20.4 * (Latest LTS: Hydrogen)                    (Aliases: default)",
    "       v20.11.0   (LTS: Iron)",
    "->     v20.11.1 * (Latest LTS: Iron)                        (Aliases: myalias)",
    "        v22.2.0",
    "        v22.3.0 *                          (Latest: node)",
];

const LTS_COLORED: [&str; 3] = [
    "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m",
    "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
    "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m",
];

#[test]
fn a_terminal_with_italics_gets_the_golden_colored_listing() {
    let output = Setup::tty().run("");
    assert_eq!(output, Output::stdout(joined(&GOLDEN_COLORED)));
}

#[test]
fn a_terminal_without_italics_gets_no_italic_names() {
    let output = Setup {
        term: "screen",
        ..Setup::tty()
    }
    .run("");
    let expected = joined(&GOLDEN_COLORED)
        .replace("\x1b[3m", "")
        .replace("\x1b[23m", "");
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn a_pipe_or_the_no_colors_flag_anywhere_gives_the_plain_listing() {
    let plain = Output::stdout(joined(&GOLDEN_PLAIN));
    let pipe = Setup {
        terminal: FakeTerminal::pipe(),
        ..Setup::tty()
    };
    assert_eq!(pipe.run(""), plain);
    assert_eq!(Setup::tty().run("--no-colors"), plain);
    let lts = Setup::tty().run("--lts --no-colors");
    assert_eq!(
        lts.stdout,
        joined(&[
            "       v18.20.4 * (Latest LTS: Hydrogen)",
            "       v20.11.0   (LTS: Iron)",
            "->     v20.11.1 * (Latest LTS: Iron)",
        ])
    );
    let pattern = Setup::tty().run("--no-colors 18");
    assert_eq!(pattern.stdout, "       v18.20.4 * (Latest LTS: Hydrogen)");
}

#[test]
fn nvm_no_colors_in_the_environment_is_ignored() {
    let output = Setup::tty().var("NVM_NO_COLORS", "--no-colors").run("");
    assert_eq!(output, Output::stdout(joined(&GOLDEN_COLORED)));
}

#[test]
fn rgbcm_moves_every_role_of_the_listing() {
    let output = Setup::tty().var("NVM_COLORS", "rgbcm").run("--lts");
    let expected = joined(&LTS_COLORED)
        .replace("0;34m", "0;31m")
        .replace("0;32m", "0;34m")
        .replace("1;32m", "1;34m")
        .replace("0;37m", "0;35m");
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn lts_listings_are_colored_without_italics() {
    assert_eq!(
        Setup::tty().run("--lts"),
        Output::stdout(joined(&LTS_COLORED))
    );
    assert_eq!(
        Setup::tty().run("--lts=iron"),
        Output::stdout(joined(&LTS_COLORED[1..]))
    );
    assert_eq!(
        Setup::tty().run("lts/hydrogen"),
        Output::stdout(LTS_COLORED[0])
    );
}

#[test]
fn a_pattern_listing_is_colored_and_keeps_its_status() {
    let output = Setup::tty().run("20");
    assert_eq!(output.stdout, joined(&LTS_COLORED[1..]));
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn no_results_is_the_plain_n_a_row_with_status_3() {
    let output = Setup::tty().run("99");
    let expected = Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(output, expected);
}

#[test]
fn an_invalid_setting_is_plain_without_markers_and_warns_once() {
    let output = Setup::tty().var("NVM_COLORS", "zzzzz").run("--lts");
    let expected = joined(&[
        "       v18.20.4   (Latest LTS: Hydrogen)",
        "       v20.11.0   (LTS: Iron)",
        "       v20.11.1   (Latest LTS: Iron)",
    ]);
    let warning = "Invalid color code: z";
    assert_eq!(output, Output::stdout(expected).with_stderr(warning));
}

#[test]
fn the_invalid_setting_warning_is_printed_on_a_pipe_and_for_n_a() {
    let pipe = Setup {
        terminal: FakeTerminal::pipe(),
        ..Setup::tty()
    };
    let output = pipe.var("NVM_COLORS", "zzzzz").run("99");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.stderr, "Invalid color code: z");
}

#[test]
fn the_system_node_in_use_leaves_no_arrow() {
    let output = Setup {
        path: "/sys",
        ..Setup::tty()
    }
    .run("--lts");
    assert_eq!(
        output.stdout.lines().last(),
        Some("\x1b[0;34m       v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m")
    );
}

#[test]
fn an_io_js_in_use_gets_the_current_color() {
    let output = Setup {
        path: "/n/versions/io.js/v3.3.1/bin",
        ..Setup::tty()
    }
    .run("");
    assert!(
        output
            .stdout
            .contains("\n\x1b[0;32m->  iojs-v3.3.1\x1b[0m\n")
    );
    assert!(
        output
            .stdout
            .contains("\n\x1b[0;34m       v20.11.1\x1b[0m  ")
    );
}

#[test]
fn no_node_in_use_has_no_arrow() {
    let output = Setup {
        path: "/nowhere",
        ..Setup::tty()
    }
    .run("--lts");
    assert!(!output.stdout.contains("->"));
}

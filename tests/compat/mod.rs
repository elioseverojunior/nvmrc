//! The compatibility contract (spec 8.3): scenarios of nvm's own test suite
//! (`test/fast`, `test/sourcing`) re-expressed as commands of the `nvm`
//! function, each run in a real shell over a temporary tree (see
//! `world.rs`), with what `nvm.sh` printed for it. stdout, stderr and the
//! status are compared separately, the temporary root replaced by `<W>`.
//!
//! A scenario with a deviation (`D1` .. `D17`, `docs/deviations.md`) pins
//! nvmrc's own output instead of nvm.sh's.
//!
//! With `NVMRC_ORACLE_NVM_SH=/path/to/nvm.sh`, the ignored
//! `capture_from_nvm_sh` tests (`mise run compat:capture`) run the same
//! scenarios through `nvm.sh` and print, as literals to paste, every outcome
//! that differs from its table. Nothing is written. bash is the reference:
//! nvm.sh ignores `--no-use` when sourced by dash or sh and does not support
//! ksh, so those captures are informative only.

mod alias;
mod codes;
mod exec;
mod init;
mod install;
mod ls;
mod nvmrc;
mod use_version;
mod world;

use std::path::Path;

pub use world::Fixture;
use world::{Implementation, World};

use crate::init_lab::find_shell;

/// The shell nvm's own suite runs in.
pub const BASH: &[&str] = &["bash"];
/// Every POSIX shell `nvm init` supports (fish has its own syntax).
pub const POSIX: &[&str] = &["bash", "zsh", "sh", "dash", "ksh"];

/// One upstream test, re-expressed; built with [`Scenario::new`] and the
/// chained setters, which default to bash, no fixture, no output, status 0.
pub struct Scenario {
    name: &'static str,
    upstream: &'static str,
    fixture: &'static [Fixture],
    shells: &'static [&'static str],
    script: &'static str,
    stdout: &'static str,
    stderr: &'static str,
    status: i32,
    deviation: Option<&'static str>,
}

impl Scenario {
    /// `name` names it in reports; `upstream` is the file under nvm's
    /// `test/` it comes from (or where nvm.sh produces the behaviour).
    #[must_use]
    pub const fn new(name: &'static str, upstream: &'static str) -> Self {
        Self {
            name,
            upstream,
            fixture: &[],
            shells: BASH,
            script: "",
            stdout: "",
            stderr: "",
            status: 0,
            deviation: None,
        }
    }

    #[must_use]
    pub const fn fixture(mut self, fixture: &'static [Fixture]) -> Self {
        self.fixture = fixture;
        self
    }

    #[must_use]
    pub const fn shells(mut self, shells: &'static [&'static str]) -> Self {
        self.shells = shells;
        self
    }

    #[must_use]
    pub const fn script(mut self, script: &'static str) -> Self {
        self.script = script;
        self
    }

    #[must_use]
    pub const fn stdout(mut self, stdout: &'static str) -> Self {
        self.stdout = stdout;
        self
    }

    #[must_use]
    pub const fn stderr(mut self, stderr: &'static str) -> Self {
        self.stderr = stderr;
        self
    }

    #[must_use]
    pub const fn status(mut self, status: i32) -> Self {
        self.status = status;
        self
    }

    /// The expectation is nvmrc's own output: deviation `id`.
    #[must_use]
    pub const fn deviation(mut self, id: &'static str) -> Self {
        self.deviation = Some(id);
        self
    }

    fn expected(&self) -> Outcome {
        Outcome {
            stdout: self.stdout.to_owned(),
            stderr: self.stderr.to_owned(),
            status: self.status,
        }
    }
}

/// What a shell printed, the root replaced by `<W>`, and its status.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    stdout: String,
    stderr: String,
    status: i32,
}

/// Runs every scenario in each of its shells through nvmrc and fails with
/// every mismatch at once; a shell that is not installed is skipped.
pub fn check_all(scenarios: &[Scenario]) {
    assert_no_system_node();
    let failures: Vec<String> = scenarios
        .iter()
        .flat_map(|scenario| scenario.shells.iter().map(move |shell| (scenario, *shell)))
        .filter_map(|(scenario, shell)| {
            let outcome = run(scenario, shell, &Implementation::Rust)?;
            (outcome != scenario.expected()).then(|| report(scenario, shell, &outcome))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Runs every scenario through the `nvm.sh` that `NVMRC_ORACLE_NVM_SH`
/// names and prints how each outcome compares with its table.
pub fn capture_all(scenarios: &[Scenario]) {
    let nvm_sh = std::env::var_os("NVMRC_ORACLE_NVM_SH")
        .expect("set NVMRC_ORACLE_NVM_SH to the nvm.sh to capture from");
    let oracle = Implementation::Oracle(Path::new(&nvm_sh));
    for scenario in scenarios {
        for shell in scenario.shells {
            if let Some(outcome) = run(scenario, shell, &oracle) {
                print_capture(scenario, shell, &outcome);
            }
        }
    }
}

fn print_capture(scenario: &Scenario, shell: &str, outcome: &Outcome) {
    let label = format!("{} [{shell}]", scenario.name);
    let same = outcome == &scenario.expected();
    match (same, scenario.deviation) {
        (true, None) => println!("// {label}: same as the table"),
        (true, Some(id)) => println!("// {label}: nvm.sh now prints the table: is {id} gone?"),
        (false, deviation) => println!(
            "// {label}: {}\n.stdout({:?})\n.stderr({:?})\n.status({})",
            deviation.map_or("DIFFERS".to_owned(), |id| format!("differs, as {id} says")),
            outcome.stdout,
            outcome.stderr,
            outcome.status
        ),
    }
}

/// Runs `scenario` in `shell`; `None` when the shell is not installed.
fn run(scenario: &Scenario, shell: &str, implementation: &Implementation<'_>) -> Option<Outcome> {
    let Some(program) = find_shell(shell) else {
        eprintln!("skipped {} in {shell}: not installed", scenario.name);
        return None;
    };
    let world = World::new(scenario.fixture, implementation);
    let output = world
        .command(&program, shell, scenario.script, implementation)
        .output()
        .expect("run the shell");
    let root = world.root().display().to_string();
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).replace(&root, "<W>");
    Some(Outcome {
        stdout: text(&output.stdout),
        stderr: text(&output.stderr),
        status: output.status.code().unwrap_or(-1),
    })
}

fn report(scenario: &Scenario, shell: &str, outcome: &Outcome) -> String {
    let deviation = scenario
        .deviation
        .map_or(String::new(), |id| format!(", deviation {id}"));
    format!(
        "{} [{shell}] (nvm test: {}{deviation})\n  expected {:?}\n  actual   {:?}",
        scenario.name,
        scenario.upstream,
        scenario.expected(),
        outcome
    )
}

/// The scenarios assume no system node unless they add one: a `node` in
/// `/usr/bin` or `/bin`, which stay on `PATH`, would break that.
fn assert_no_system_node() {
    for directory in ["/usr/bin", "/bin"] {
        let node = Path::new(directory).join("node");
        assert!(
            !node.exists(),
            "the contract needs a machine without {}",
            node.display()
        );
    }
}

//! A vulnerability index that cannot be read while node rules already fail:
//! the known violations still decide the gate, and the index is a warning.

use crate::commands::policy::enforce::{Gate, Origin, Requested, enforce_node};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::policy::{DEFAULT_INDEX, PolicyFlags, extract};
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use std::ffi::OsString;

const BLOCK: [&str; 3] = [
    "nvm: node v18.19.0 (from the command line) is not allowed by policy:",
    "  - below the minimum v20.11.1 (from --min-node)",
    "Use an allowed version.",
];

fn min_flags() -> PolicyFlags {
    let args = ["nvm", "--min-node", "20.11.1"];
    extract(args.into_iter().map(OsString::from).collect())
        .unwrap()
        .0
}

fn requested(offline: bool) -> Requested {
    Requested {
        version: "v18.19.0".parse().unwrap(),
        origin: Origin::Argument,
        offline,
        pins: crate::commands::project_pins::ProjectPins::default(),
    }
}

/// The gate under `--min-node 20.11.1` with nothing served, so the index at
/// the default location cannot be read.
fn run(advisory: bool, offline: bool) -> (Result<Gate, CliError>, String, Vec<String>) {
    let (fs, env, http) = (
        FakeFileSystem::default(),
        FakeEnv::default().with_var("NVM_DIR", "/n"),
        FakeHttp::default(),
    );
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http)
        .with_policy_flags(min_flags());
    let context = if advisory {
        context.advisory()
    } else {
        context
    };
    let mut transcript = Transcript::default();
    let gate = enforce_node(&context, &requested(offline), &mut transcript);
    let stderr = transcript.finish(NvmExitCode::Success).stderr;
    (gate, stderr, http.requests())
}

fn last_line(stderr: &str) -> &str {
    stderr.lines().last().unwrap_or_default()
}

#[test]
fn a_node_violation_still_stops_with_7_when_the_index_is_unreachable() {
    let (gate, stderr, requests) = run(false, false);
    assert!(
        matches!(gate, Ok(Gate::Stop(NvmExitCode::BelowVersionFloor))),
        "{gate:?}"
    );
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines[..3], BLOCK, "{stderr}");
    assert_eq!(lines.len(), 4, "{stderr}");
    assert!(
        last_line(&stderr).starts_with(&format!(
            "nvm: warning: cannot check known vulnerabilities: {DEFAULT_INDEX} cannot be read: "
        )),
        "{stderr}"
    );
    assert_eq!(requests, [DEFAULT_INDEX]);
}

#[test]
fn a_node_violation_still_stops_with_7_offline_with_a_remote_index() {
    let (gate, stderr, requests) = run(false, true);
    assert!(
        matches!(gate, Ok(Gate::Stop(NvmExitCode::BelowVersionFloor))),
        "{gate:?}"
    );
    assert_eq!(
        stderr,
        [
            BLOCK.join("\n").as_str(),
            "nvm: warning: cannot check known vulnerabilities: \
             the release information is unavailable: offline",
        ]
        .join("\n")
    );
    assert!(requests.is_empty());
}

#[test]
fn an_advisory_context_prints_both_and_passes() {
    let (gate, stderr, _) = run(true, false);
    assert!(matches!(gate, Ok(Gate::Pass)), "{gate:?}");
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(
        lines[0],
        "nvm: warning: node v18.19.0 (from the command line) is not allowed by policy:"
    );
    assert_eq!(lines[1..3], BLOCK[1..], "{stderr}");
    assert!(
        last_line(&stderr).starts_with("nvm: warning: cannot check known vulnerabilities: "),
        "{stderr}"
    );
}

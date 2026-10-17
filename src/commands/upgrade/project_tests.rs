use std::path::PathBuf;

use super::*;
use crate::commands::Output;
use crate::commands::project::test_support::{PNPM_URL, file, project, served};
use crate::commands::upgrade::request::parse;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

const PINNED: &str = "v20.11.1\npnpm=9.1.0\n";
const PNPM_9: &str = "{ \"packageManager\": \"pnpm@9.1.0\" }";

/// Runs `nvm upgrade args`: what it printed, with its status, and what it did.
pub(super) fn upgrade_in(
    fs: &FakeFileSystem,
    env: &FakeEnv,
    http: &FakeHttp,
    args: &[&str],
) -> (Output, Option<Upgraded>) {
    let context = Context::new(fs, env)
        .with_http(http)
        .with_third_party_http(http, http);
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let request = parse(&args).unwrap();
    let mut transcript = Transcript::default();
    let (status, upgraded) = match upgrade(&context, &request, &mut transcript) {
        Ok(upgraded) => (NvmExitCode::Success, Some(upgraded)),
        Err(stop) => (stop.printed(&mut transcript), None),
    };
    (transcript.finish(status), upgraded)
}

#[test]
fn upgrades_every_pinned_tool_by_default() {
    let files = [("/p/.nvmrc", PINNED), ("/p/package.json", PNPM_9)];
    let (fs, env) = project(&files, "");
    let (output, upgraded) = upgrade_in(&fs, &env, &served(), &[]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(
        output.stdout,
        "node v20.11.1 -> v20.18.0\npnpm 9.1.0 -> 9.12.0"
    );
    let nvmrc = PathBuf::from("/p/.nvmrc");
    assert_eq!(upgraded, Some(Upgraded::Written { nvmrc }));
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v20.18.0\npnpm=9.12.0\n")
    );
    assert_eq!(
        file(&fs, "/p/package.json").as_deref(),
        Some("{ \"packageManager\": \"pnpm@9.12.0\" }")
    );
}

#[test]
fn a_tool_flag_upgrades_only_that_tool() {
    let files = [("/p/.nvmrc", PINNED), ("/p/package.json", PNPM_9)];
    let (fs, env) = project(&files, "");
    let (output, _) = upgrade_in(&fs, &env, &served(), &["--pnpm"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "pnpm 9.1.0 -> 9.12.0");
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v20.11.1\npnpm=9.12.0\n")
    );
}

#[test]
fn a_flag_for_an_unpinned_tool_is_a_usage_error() {
    let (fs, env) = project(&[("/p/.nvmrc", "v20.11.1\n")], "");
    let http = served();
    let (output, _) = upgrade_in(&fs, &env, &http, &["--pnpm"]);
    assert_eq!(output.status.code(), 6);
    assert_eq!(
        output.stderr,
        "nvm upgrade: /p/.nvmrc does not pin pnpm; use nvm init --pnpm to add it."
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[test]
fn a_partial_node_line_is_upgraded_within_its_major() {
    let (fs, env) = project(&[("/p/.nvmrc", "# app\n20\n")], "");
    let (output, _) = upgrade_in(&fs, &env, &served(), &[]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "node 20 -> v20.18.0");
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some("# app\nv20.18.0\n"));
}

#[test]
fn an_alias_node_line_is_refused() {
    for alias in ["lts/*", "node", "iojs-v3.3.1"] {
        let (fs, env) = project(&[("/p/.nvmrc", &format!("{alias}\n"))], "");
        let http = served();
        let (output, _) = upgrade_in(&fs, &env, &http, &[]);
        assert_eq!(output.status.code(), 3);
        assert_eq!(
            output.stderr,
            format!(
                "nvm upgrade: /p/.nvmrc names node as \"{alias}\", not a version; \
                 use nvm init --node to pin one."
            )
        );
        assert!(http.requests().is_empty(), "{:?}", http.requests());
    }
}

#[test]
fn no_nvmrc_is_refused() {
    let (fs, env) = project(&[], "");
    let (output, _) = upgrade_in(&fs, &env, &served(), &[]);
    assert_eq!(output.status.code(), 3);
    assert_eq!(
        output.stderr,
        "nvm upgrade: no .nvmrc found; run nvm init first."
    );
}

#[test]
fn a_malformed_pin_is_refused() {
    let (fs, env) = project(&[("/p/.nvmrc", "v20.11.1\npnpm=nine\n")], "");
    let (output, _) = upgrade_in(&fs, &env, &served(), &[]);
    assert_eq!(output.status.code(), 3, "{}", output.stderr);
    assert!(output.stderr.contains("/p/.nvmrc"), "{}", output.stderr);
}

#[test]
fn one_tool_without_a_target_writes_nothing() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimum": "10.0.0" } } }"#;
    let files = [("/p/.nvmrc", PINNED), ("/p/package.json", PNPM_9)];
    let (fs, env) = project(&files, policy);
    let pnpm = r#"{"versions": {"9.1.0": {}, "9.12.0": {}, "10.2.0": {}}}"#;
    let http = served().with_body(PNPM_URL, pnpm);
    let (output, _) = upgrade_in(&fs, &env, &http, &[]);
    assert_eq!(output.status.code(), 7);
    assert_eq!(output.stdout, "");
    assert_eq!(
        output.stderr,
        "nvm upgrade: no allowed pnpm release in major 9.\n\
         nvm: pnpm 9.12.0 (the newest in major 9) is not allowed by policy:\n  \
         - below the minimum 10.0.0 (from policy /policy.json)\n\
         The lowest major with an allowed release is 10. Run \
         `nvm upgrade --pnpm --major=10`, or `nvm upgrade --pnpm --major` for the newest."
    );
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some(PINNED));
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some(PNPM_9));
}

#[test]
fn no_allowed_release_in_any_major_says_so() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimum": "11.0.0" } } }"#;
    let (fs, env) = project(&[("/p/.nvmrc", "pnpm=9.1.0\n")], policy);
    let (output, _) = upgrade_in(&fs, &env, &served(), &["--major"]);
    assert_eq!(output.status.code(), 7);
    assert_eq!(
        output.stderr,
        "nvm upgrade: no allowed pnpm release.\n\
         nvm: pnpm 9.12.0 (the newest release) is not allowed by policy:\n  \
         - below the minimum 11.0.0 (from policy /policy.json)\n\
         No major has an allowed pnpm release."
    );
}

#[test]
fn everything_up_to_date_writes_nothing() {
    let nvmrc = "v20.18.0\npnpm=9.12.0\n";
    let (fs, env) = project(&[("/p/.nvmrc", nvmrc)], "");
    let (output, upgraded) = upgrade_in(&fs, &env, &served(), &[]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(
        output.stdout,
        "node v20.18.0 is up to date\npnpm 9.12.0 is up to date"
    );
    assert_eq!(upgraded, Some(Upgraded::Nothing));
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some(nvmrc));
    assert_eq!(file(&fs, "/p/package.json"), None);
}

#[test]
fn the_nvmrc_in_a_parent_directory_is_the_one_rewritten() {
    let files = [("/p/.nvmrc", PINNED), ("/p/package.json", PNPM_9)];
    let (fs, _) = project(&files, "");
    let fs = fs.with_dir("/p/app");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/p/app")
        .with_current_dir("/p/app");
    let (output, upgraded) = upgrade_in(&fs, &env, &served(), &[]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    let nvmrc = PathBuf::from("/p/.nvmrc");
    assert_eq!(upgraded, Some(Upgraded::Written { nvmrc }));
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v20.18.0\npnpm=9.12.0\n")
    );
    assert_eq!(file(&fs, "/p/app/.nvmrc"), None);
    assert_eq!(file(&fs, "/p/app/package.json"), None);
}

#[test]
fn major_n_with_two_pinned_tools_and_no_flag_is_a_usage_error() {
    let (fs, env) = project(&[("/p/.nvmrc", PINNED)], "");
    let http = served();
    let (output, _) = upgrade_in(&fs, &env, &http, &["--major=22"]);
    assert_eq!(output.status.code(), 6);
    assert_eq!(
        output.stderr,
        "nvm upgrade: --major=<N> applies to one tool; name exactly one."
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[test]
fn major_n_with_one_pinned_tool_and_no_flag_moves_it() {
    let (fs, env) = project(&[("/p/.nvmrc", "v20.11.1\n")], "");
    let (output, _) = upgrade_in(&fs, &env, &served(), &["--major=22"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "node v20.11.1 -> v22.11.0");
}

#[test]
fn a_failed_write_prints_no_change_line() {
    let files = [("/p/.nvmrc", PINNED), ("/p/package.json", PNPM_9)];
    let (fs, env) = project(&files, "");
    let fs = fs.with_write_failure("/p/.nvmrc", 0);
    let (output, upgraded) = upgrade_in(&fs, &env, &served(), &[]);
    assert_ne!(output.status.code(), 0);
    assert_eq!(output.stdout, "");
    assert_eq!(upgraded, None);
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some(PINNED));
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some(PNPM_9));
}

#[test]
fn an_alias_node_line_is_left_alone_when_node_is_not_selected() {
    let nvmrc = "lts/*\npnpm=9.1.0\n";
    let (fs, env) = project(&[("/p/.nvmrc", nvmrc), ("/p/package.json", PNPM_9)], "");
    let (output, _) = upgrade_in(&fs, &env, &served(), &["--pnpm"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "pnpm 9.1.0 -> 9.12.0");
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("lts/*\npnpm=9.12.0\n")
    );
}

#[test]
fn an_alias_node_line_is_refused_when_node_is_selected() {
    let nvmrc = "lts/*\npnpm=9.1.0\n";
    for args in [&[][..], &["--node"][..]] {
        let (fs, env) = project(&[("/p/.nvmrc", nvmrc)], "");
        let (output, _) = upgrade_in(&fs, &env, &served(), args);
        assert_eq!(output.status.code(), 3, "{args:?}");
        assert!(output.stderr.contains("names node as \"lts/*\""));
        assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some(nvmrc));
    }
}

#[test]
fn an_nvmrc_that_pins_nothing_is_refused_before_any_request() {
    let (fs, env) = project(&[("/p/.nvmrc", "# nothing yet\n\n")], "");
    let http = served();
    let (output, _) = upgrade_in(&fs, &env, &http, &[]);
    assert_eq!(output.status.code(), 3);
    assert_eq!(
        output.stderr,
        "nvm upgrade: /p/.nvmrc pins nothing; use nvm init to add a tool."
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

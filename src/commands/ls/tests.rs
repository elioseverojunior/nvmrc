use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

fn installed() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/versions/node/v18.9.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
        .with_file("/n/versions/io.js/v2.5.0/bin/node", "")
        .with_file("/n/alias/work", "v18")
        .with_file("/n/alias/loop", "loop")
        .with_file("/n/alias/default", "node")
        .with_file("/n/alias/lts/iron", "v20.10.0")
        .with_file("/n/alias/lts/gallium", "v16.20.2")
}

fn ls_on(fs: &FakeFileSystem, path: &str, pattern: Option<&str>) -> Output {
    let process = FakeProcess::default().with_output("/sys/node", "v22.1.0\n");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    let context = Context::new(fs, &env).with_process(&process);
    run(&context, pattern).unwrap()
}

fn ls(pattern: Option<&str>) -> Output {
    ls_on(&installed(), "/nonexistent", pattern)
}

fn rows(lines: &[&str]) -> String {
    lines.join("\n")
}

#[test]
fn lists_every_version_sorted_by_number_with_io_js_in_between() {
    let expected = rows(&[
        "    iojs-v2.5.0 *",
        "    iojs-v3.0.0 *",
        "        v18.9.0 *",
        "        v20.1.0 *",
        "       v20.10.0 *",
    ]);
    assert_eq!(ls(None), Output::stdout(expected));
}

#[test]
fn the_version_in_use_gets_an_arrow() {
    let output = ls_on(&installed(), "/n/versions/node/v18.9.0/bin", None);
    assert!(
        output.stdout.contains("\n->      v18.9.0 *\n"),
        "{}",
        output.stdout
    );
}

#[test]
fn the_system_node_is_listed_last_with_its_version() {
    let fs = installed().with_file("/sys/node", "");
    let output = ls_on(&fs, "/sys", None);
    let last = output.stdout.lines().last().unwrap();
    assert_eq!(last, "->       system * (-> v22.1.0)");
    let elsewhere = ls_on(&fs, "/n/versions/node/v18.9.0/bin:/sys", None);
    let last = elsewhere.stdout.lines().last().unwrap();
    assert_eq!(last, "         system * (-> v22.1.0)");
}

#[test]
fn a_pattern_lists_the_versions_it_matches() {
    let expected = rows(&["        v20.1.0 *", "       v20.10.0 *"]);
    assert_eq!(ls(Some("20")), Output::stdout(expected.clone()));
    assert_eq!(ls(Some("v20")), Output::stdout(expected));
    assert_eq!(ls(Some("20.1")), Output::stdout("        v20.1.0 *"));
    assert_eq!(ls(Some("v18.9.0")), Output::stdout("        v18.9.0 *"));
}

#[test]
fn a_plain_pattern_also_finds_io_js_like_nvm_sh() {
    assert_eq!(ls(Some("3")), Output::stdout("    iojs-v3.0.0 *"));
    assert_eq!(ls(Some("v3.0.0")), Output::stdout("    iojs-v3.0.0 *"));
}

#[test]
fn node_and_iojs_list_only_their_own_flavor() {
    let node = rows(&[
        "        v18.9.0 *",
        "        v20.1.0 *",
        "       v20.10.0 *",
    ]);
    assert_eq!(ls(Some("node")), Output::stdout(node));
    let iojs = rows(&["    iojs-v2.5.0 *", "    iojs-v3.0.0 *"]);
    assert_eq!(ls(Some("iojs")), Output::stdout(iojs));
}

#[test]
fn an_alias_lists_the_version_it_resolves_to() {
    assert_eq!(ls(Some("work")), Output::stdout("        v18.9.0 *"));
    assert_eq!(ls(Some("default")), Output::stdout("       v20.10.0 *"));
    assert_eq!(ls(Some("lts/iron")), Output::stdout("       v20.10.0 *"));
    assert_eq!(ls(Some("stable")), Output::stdout("       v20.10.0 *"));
}

#[test]
fn nothing_matching_prints_na_and_exits_3() {
    let expected = Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion);
    for pattern in ["99", "foo", "lts/gallium", "unstable", "system"] {
        assert_eq!(ls(Some(pattern)), expected, "{pattern}");
    }
}

#[test]
fn an_alias_loop_lists_infinity() {
    assert_eq!(ls(Some("loop")), Output::stdout("            ∞"));
}

#[test]
fn current_lists_the_active_version_or_none() {
    assert_eq!(ls(Some("current")), Output::stdout("->         none *"));
    let output = ls_on(
        &installed(),
        "/n/versions/node/v18.9.0/bin",
        Some("current"),
    );
    assert_eq!(output, Output::stdout("->      v18.9.0 *"));
}

#[test]
fn system_lists_the_system_node_when_there_is_one() {
    let fs = FakeFileSystem::default().with_file("/sys/node", "");
    let output = ls_on(&fs, "/sys", Some("system"));
    assert_eq!(output, Output::stdout("->       system * (-> v22.1.0)"));
}

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

#[test]
fn options_are_read_like_nvm_sh() {
    let parsed = parse_options(&words(&["--no-colors", "--", "20", "18"])).unwrap();
    assert_eq!(
        parsed,
        Options {
            pattern: Some("20".to_owned()),
            no_alias: false
        }
    );
    let no_alias = parse_options(&words(&["--no-alias"])).unwrap();
    assert!(no_alias.no_alias && no_alias.pattern.is_none());
    let empty_first = parse_options(&words(&["", "20"])).unwrap();
    assert_eq!(empty_first.pattern, Some("20".to_owned()));
}

#[test]
fn bad_options_are_exit_55() {
    let unknown = parse_options(&words(&["--bogus"])).unwrap_err();
    assert_eq!(unknown.to_string(), "Unsupported option \"--bogus\".");
    assert_eq!(unknown.exit_code(), NvmExitCode::UnsupportedOption);
    let both = parse_options(&words(&["20", "--no-alias"])).unwrap_err();
    assert_eq!(
        both.to_string(),
        "`--no-alias` is not supported when a pattern is provided."
    );
    assert_eq!(both.exit_code(), NvmExitCode::UnsupportedOption);
}

fn command(fs: &FakeFileSystem, args: &[&str]) -> Output {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/nonexistent");
    run_command(&Context::new(fs, &env), &words(args)).unwrap()
}

#[test]
fn the_aliases_follow_the_versions() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/alias/default", "node");
    let expected = [
        "       v20.10.0 *",
        "default -> node (-> v20.10.0 *)",
        "iojs -> N/A (default)",
        "node -> stable (-> v20.10.0 *) (default)",
        "stable -> 20.10 (-> v20.10.0 *) (default)",
        "unstable -> N/A (default)",
    ];
    assert_eq!(command(&fs, &[]), Output::stdout(expected.join("\n")));
    let versions_only = command(&fs, &["--no-alias"]);
    assert_eq!(versions_only, Output::stdout(expected[0]));
}

#[test]
fn a_pattern_lists_no_aliases_and_keeps_the_status() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/bin/node", "");
    let output = command(&fs, &["99"]);
    assert_eq!(
        output,
        Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion)
    );
}

#[test]
fn nothing_installed_still_lists_the_aliases_with_status_3() {
    let output = command(&FakeFileSystem::default(), &[]);
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stdout,
        "            N/A *\n\
         iojs -> N/A (default)\n\
         node -> stable (-> N/A) (default)\n\
         unstable -> N/A (default)"
    );
}

#[test]
fn nothing_installed_prints_na_as_if_installed_and_exits_3() {
    let fs = FakeFileSystem::default();
    let expected = Output::stdout("            N/A *").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(ls_on(&fs, "/nonexistent", None), expected);
}

#[test]
fn only_a_system_node_lists_just_that_row() {
    let fs = FakeFileSystem::default().with_file("/sys/node", "");
    let output = ls_on(&fs, "/sys", None);
    assert_eq!(output, Output::stdout("->       system * (-> v22.1.0)"));
}

#[test]
fn a_system_node_that_does_not_answer_is_listed_without_a_version() {
    let fs = FakeFileSystem::default().with_file("/other/node", "");
    let output = ls_on(&fs, "/other", None);
    assert_eq!(output, Output::stdout("->       system *"));
}

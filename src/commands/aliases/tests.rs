use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};

/// The fixture the golden outputs were captured from, with the real `nvm.sh`.
fn fixture() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/versions/node/v18.9.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
        .with_file("/n/versions/io.js/v2.5.0/bin/node", "")
        .with_file("/n/alias/default", "node\n")
        .with_file("/n/alias/work", "v18\n")
        .with_file("/n/alias/lts/*", "lts/iron\n")
        .with_file("/n/alias/lts/iron", "v20.10.0\n")
        .with_file("/n/alias/lts/gallium", "v16.20.2\n")
}

fn list_on(fs: &FakeFileSystem, path: &str, prefix: Option<&str>) -> Output {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    list(&Context::new(fs, &env), prefix).unwrap()
}

fn list_with(fs: &FakeFileSystem, prefix: Option<&str>) -> Output {
    list_on(fs, "/nonexistent", prefix)
}

fn lines(output: &Output) -> Vec<&str> {
    output.stdout.lines().collect()
}

#[test]
fn lists_the_files_then_the_implicit_aliases_then_the_lts_aliases() {
    let output = list_with(&fixture(), None);
    assert_eq!(
        lines(&output),
        [
            "default -> node (-> v20.10.0 *)",
            "work -> v18 (-> v18.9.0 *)",
            "iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)",
            "node -> stable (-> v20.10.0 *) (default)",
            "stable -> 20.10 (-> v20.10.0 *) (default)",
            "unstable -> N/A (default)",
            "lts/* -> lts/iron (-> v20.10.0 *)",
            "lts/gallium -> v16.20.2 (-> N/A)",
            "lts/iron -> v20.10.0 *",
        ]
    );
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn a_prefix_selects_alias_files_and_the_exactly_named_implicit_alias() {
    let fs = fixture();
    assert_eq!(
        lines(&list_with(&fs, Some("wor"))),
        ["work -> v18 (-> v18.9.0 *)"]
    );
    assert_eq!(
        lines(&list_with(&fs, Some("default"))),
        ["default -> node (-> v20.10.0 *)"]
    );
    assert_eq!(
        lines(&list_with(&fs, Some("iojs"))),
        ["iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)"]
    );
    assert_eq!(
        lines(&list_with(&fs, Some("stable"))),
        ["stable -> 20.10 (-> v20.10.0 *) (default)"]
    );
}

#[test]
fn a_prefix_also_selects_lts_aliases_by_name() {
    let output = list_with(&fixture(), Some("i"));
    assert_eq!(lines(&output), ["lts/iron -> v20.10.0 *"]);
}

#[test]
fn an_unknown_prefix_prints_nothing_and_succeeds() {
    let output = list_with(&fixture(), Some("nope"));
    assert_eq!(output, Output::default());
}

#[test]
fn an_lts_name_prints_the_alias_file_target_as_is() {
    let fs = fixture();
    assert_eq!(list_with(&fs, Some("lts/iron")), Output::stdout("v20.10.0"));
    assert_eq!(list_with(&fs, Some("lts/*")), Output::stdout("lts/iron"));
}

#[test]
fn a_missing_lts_alias_is_exit_2_with_a_message_on_stderr() {
    let fs = fixture();
    let expected = Output::default()
        .with_stderr("Alias does not exist.")
        .with_status(NvmExitCode::MissingTarget);
    assert_eq!(list_with(&fs, Some("lts/nope")), expected);
    assert_eq!(list_with(&fs, Some("lts/")), expected);
}

#[test]
fn an_alias_file_hides_the_implicit_alias_of_the_same_name() {
    let fs = fixture().with_file("/n/alias/node", "v18\n");
    let output = list_with(&fs, None);
    let node: Vec<&str> = lines(&output)
        .into_iter()
        .filter(|line| line.starts_with("node "))
        .collect();
    assert_eq!(node, ["node -> v18 (-> v18.9.0 *)"]);
}

#[test]
fn nothing_installed_lists_the_implicit_aliases_without_stable() {
    let output = list_with(&FakeFileSystem::default(), None);
    assert_eq!(
        lines(&output),
        [
            "iojs -> N/A (default)",
            "node -> stable (-> N/A) (default)",
            "unstable -> N/A (default)",
        ]
    );
}

#[test]
fn old_release_lines_make_stable_and_unstable_differ() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v0.10.48/bin/node", "")
        .with_file("/n/versions/node/v0.11.16/bin/node", "")
        .with_file("/n/versions/node/v0.12.18/bin/node", "")
        .with_file("/n/versions/node/v4.2.0/bin/node", "");
    let output = list_with(&fs, None);
    assert_eq!(
        lines(&output),
        [
            "iojs -> N/A (default)",
            "node -> stable (-> v4.2.0 *) (default)",
            "stable -> 4.2 (-> v4.2.0 *) (default)",
            "unstable -> 0.11 (-> v0.11.16 *) (default)",
        ]
    );
}

#[test]
fn loops_comments_blank_lines_and_system_targets_are_shown_like_nvm_sh() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v4.2.0/bin/node", "")
        .with_file("/n/versions/node/v0.10.48/bin/node", "")
        .with_file("/n/alias/a", "b\n")
        .with_file("/n/alias/b", "a\n")
        .with_file("/n/alias/loop", "loop\n")
        .with_file("/n/alias/commented", "v4 # four\n")
        .with_file("/n/alias/blankfirst", "\n\nv0.10\n")
        .with_file("/n/alias/sys", "system\n");
    let output = list_with(&fs, None);
    assert_eq!(
        &lines(&output)[..6],
        [
            "a -> b (-> ∞)",
            "b -> a (-> ∞)",
            "blankfirst -> v0.10 (-> v0.10.48 *)",
            "commented -> v4 (-> v4.2.0 *)",
            "loop -> loop (-> ∞)",
            "sys -> system (-> N/A)",
        ]
    );
}

#[test]
fn a_system_node_makes_a_system_alias_resolve() {
    let fs = FakeFileSystem::default()
        .with_file("/sys/node", "")
        .with_file("/n/alias/sys", "system\n");
    let output = list_on(&fs, "/sys", Some("sys"));
    assert_eq!(lines(&output), ["sys -> system *"]);
}

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

fn run_words(fs: &FakeFileSystem, args: &[&str]) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/nonexistent");
    run(&Context::new(fs, &env), &words(args))
}

#[test]
fn no_words_list_and_one_word_is_a_prefix() {
    let fs = fixture();
    assert_eq!(run_words(&fs, &[]).unwrap(), list_with(&fs, None));
    assert_eq!(
        run_words(&fs, &["--no-colors"]).unwrap(),
        list_with(&fs, None)
    );
    assert_eq!(
        run_words(&fs, &["--", "default"]).unwrap(),
        list_with(&fs, Some("default"))
    );
}

#[test]
fn two_words_create_the_alias_and_extra_words_are_ignored() {
    let fs = fixture();
    let output = run_words(&fs, &["fresh", "stable", "ignored"]).unwrap();
    assert_eq!(output, Output::stdout("fresh -> stable (-> v20.10.0 *)"));
    assert_eq!(
        list_with(&fs, Some("fresh")),
        Output::stdout("fresh -> stable (-> v20.10.0 *)")
    );
}

#[test]
fn an_explicitly_empty_target_deletes_the_alias() {
    let fs = fixture();
    let output = run_words(&fs, &["work", ""]).unwrap();
    assert!(output.stdout.starts_with("Deleted alias work"));
}

#[test]
fn unknown_options_are_exit_55() {
    let error = run_words(&fixture(), &["--bogus"]).unwrap_err();
    assert_eq!(error.to_string(), "Unsupported option \"--bogus\".");
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
}

#[test]
fn a_comment_delimiter_in_the_name_is_rejected_even_without_a_target() {
    let error = run_words(&fixture(), &["a#b"]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Aliases with a comment delimiter (#) are not supported."
    );
    assert_eq!(error.exit_code(), NvmExitCode::Failure);
}

#[test]
fn hidden_files_and_directories_are_not_aliases() {
    let fs = fixture()
        .with_file("/n/alias/.DS_Store", "v20\n")
        .with_file("/n/alias/other/inner", "v20\n");
    let output = list_with(&fs, None);
    assert!(lines(&output).iter().all(|line| !line.contains("DS_Store")));
    assert!(lines(&output).iter().all(|line| !line.starts_with("other")));
}

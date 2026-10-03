//! The `.nvmrc` matrix of digest 3.5: `nvm use` from `/proj/sub/deep` with
//! the file at `/proj/.nvmrc`.

use super::*;

const NVMRC_HINT: &str =
    "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.";
const PLEASE_SEE: &str =
    "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.";
const INVALID: &str = "invalid .nvmrc!\n\
all non-commented content (anything after # is a comment) must be either:\n  \
- a single bare nvm-recognized version-ish\n  \
- or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n\
\n\
additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n\
\n\
non-commented content parsed:";

fn project(content: &str) -> FakeFileSystem {
    lab().with_file("/proj/.nvmrc", content)
}

fn deep_on(path: &str) -> FakeEnv {
    env_on(path).with_var("PWD", "/proj/sub/deep")
}

fn use_from(content: &str, args: &[&str]) -> (Result<Target, Halt>, Output) {
    run_in(&project(content), &deep_on(BASE_PATH), args)
}

fn found(version: &str) -> String {
    format!("Found '/proj/.nvmrc' with version <{version}>")
}

fn assert_uses_from_nvmrc(content: &str, version: &str, expected: &str) {
    let (target, output) = use_from(content, &[]);
    let Ok(Target::Installed {
        version: chosen,
        from_nvmrc,
        provided,
        ..
    }) = target
    else {
        panic!("{content:?} gave {target:?} and {output:?}");
    };
    assert_eq!(chosen.to_string(), expected, "{content:?}");
    assert!(from_nvmrc, "{content:?}");
    assert_eq!(provided.as_deref(), Some(version), "{content:?}");
    assert_eq!(output.stdout, found(version), "{content:?}");
    assert_eq!(output.stderr, "", "{content:?}");
}

#[test]
fn a_usable_nvmrc_is_announced_and_resolved() {
    assert_uses_from_nvmrc("18\n", "18", "v18.20.4");
    assert_uses_from_nvmrc("  v20.11  \r\n", "v20.11", "v20.11.1");
    assert_uses_from_nvmrc("# comment\n\n  18 # trailing\n", "18", "v18.20.4");
    assert_uses_from_nvmrc("lts/*", "lts/*", "v20.11.1");
    assert_uses_from_nvmrc("lts/hydrogen", "lts/hydrogen", "v18.20.4");
    assert_uses_from_nvmrc("node", "node", "v22.3.0");
    assert_uses_from_nvmrc("iojs", "iojs", "iojs-v3.3.1");
    assert_uses_from_nvmrc("18\nfoo=bar\n baz = qux \n", "18", "v18.20.4");
}

#[test]
fn silent_hides_the_found_line() {
    let (target, output) = use_from("18\n", &["--silent"]);
    assert!(matches!(
        target,
        Ok(Target::Installed {
            from_nvmrc: true,
            ..
        })
    ));
    assert_eq!(output, Output::default());
}

fn assert_invalid(content: &str, parsed: &str) {
    let (target, output) = use_from(content, &[]);
    assert_eq!(
        target,
        Err(Halt {
            status: NvmExitCode::NotFound
        }),
        "{content:?}"
    );
    assert_eq!(output.stdout, "", "{content:?}");
    assert_eq!(
        output.stderr,
        format!("{INVALID}{parsed}\n{PLEASE_SEE}"),
        "{content:?}"
    );
}

#[test]
fn an_invalid_nvmrc_is_explained_then_status_127() {
    for empty in ["", "# only comment\n", "  \n\t\n"] {
        assert_invalid(empty, "");
    }
    assert_invalid("18\n20\n", "\n18\n20");
    assert_invalid("node=18", "\nnode=18");
    assert_invalid("a=1\na=2\n18", "\na=1\na=2\n18");
    assert_invalid("foo=bar", "\nfoo=bar");
}

#[test]
fn the_invalid_message_is_printed_even_when_silent() {
    let (_, output) = use_from("", &["--silent"]);
    assert_eq!(output.stderr, format!("{INVALID}\n{PLEASE_SEE}"));
}

fn assert_nvmrc_fails(content: &str, status: NvmExitCode, stderr: &str) {
    let (target, output) = use_from(content, &[]);
    assert_eq!(target, Err(Halt { status }), "{content:?}");
    assert_eq!(output.stdout, found(content), "{content:?}");
    assert_eq!(output.stderr, stderr, "{content:?}");
}

#[test]
fn a_version_that_is_not_installed_points_at_plain_nvm_install() {
    let rows = [
        ("=x", "=x"),
        ("99", "v99"),
        ("missing", "missing -> v99"),
        ("garbage!", "garbage!"),
    ];
    for (content, shown) in rows {
        let message = format!("N/A: version \"{shown}\" is not yet installed.\n\n{NVMRC_HINT}");
        assert_nvmrc_fails(content, NvmExitCode::InvalidVersion, &message);
    }
}

#[test]
fn silent_hides_both_the_found_line_and_the_not_installed_message() {
    let (target, output) = use_from("99", &["--silent"]);
    assert_eq!(
        target,
        Err(Halt {
            status: NvmExitCode::InvalidVersion
        })
    );
    assert_eq!(
        output,
        Output {
            status: NvmExitCode::InvalidVersion,
            ..Output::default()
        }
    );
}

#[test]
fn an_alias_loop_in_nvmrc_is_status_8() {
    let message = "The alias \"loopa\" leads to an infinite loop. Aborting.";
    assert_nvmrc_fails("loopa", NvmExitCode::AliasLoop, message);
}

#[test]
fn system_in_nvmrc_without_a_system_node_is_status_3() {
    let message = "N/A: no system version of node/io.js is installed.";
    assert_nvmrc_fails("system", NvmExitCode::InvalidVersion, message);
}

#[test]
fn system_in_nvmrc_with_a_system_node_is_the_system_node() {
    let fs = project("system").with_executable("/sys/bin/node", "");
    let (target, output) = run_in(&fs, &deep_on("/sys/bin:/usr/bin"), &[]);
    let expected = SystemNode {
        flavor: SystemFlavor::Node,
        binary: PathBuf::from("/sys/bin/node"),
    };
    assert_eq!(target, Ok(Target::System(expected)));
    assert_eq!(
        (output.stdout, output.stderr),
        (found("system"), String::new())
    );
}

/// DELIBERATE DEVIATION, as for `nvm use current`: nvm.sh returns 0.
#[test]
fn a_version_directory_without_its_node_is_status_3_with_the_nvmrc_hint() {
    let fs = project("24").with_dir("/n/versions/node/v24.0.0");
    let (target, output) = run_in(&fs, &deep_on(BASE_PATH), &[]);
    assert_eq!(
        target,
        Err(Halt {
            status: NvmExitCode::InvalidVersion
        })
    );
    assert_eq!(output.stdout, found("24"));
    let message = format!("N/A: version \"v24.0.0\" is not yet installed.\n\n{NVMRC_HINT}");
    assert_eq!(output.stderr, message);
}

#[test]
fn a_version_or_lts_flag_ignores_the_nvmrc() {
    for args in [&["--lts"][..], &["20"]] {
        let (target, output) = use_from("18", args);
        let Ok(Target::Installed {
            version: chosen,
            from_nvmrc,
            ..
        }) = target
        else {
            panic!("{args:?} gave {target:?}");
        };
        assert_eq!((chosen, from_nvmrc), (version("v20.11.1"), false));
        assert_eq!(output, Output::default());
    }
}

//! End-to-end: `ls` and `alias` listings of the real binaries, compared with
//! what the real `nvm.sh` printed for the same `$NVM_DIR` (captured with its
//! stdout piped, so without colors).

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(binary: &str, nvm_dir: &Path, path: &OsStr, args: &[&str]) -> Output {
    Command::new(binary)
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PATH", path)
        .output()
        .expect("run the binary")
}

fn nvm(nvm_dir: &Path, args: &[&str]) -> Output {
    let path = OsStr::new("/nonexistent");
    run(env!("CARGO_BIN_EXE_nvm"), nvm_dir, path, args)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn install(nvm_dir: &Path, directory: &str) {
    let bin = nvm_dir.join(directory).join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("node"), "").unwrap();
}

fn alias(nvm_dir: &Path, name: &str, target: &str) {
    let file = nvm_dir.join("alias").join(name);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, format!("{target}\n")).unwrap();
}

/// The fixture the golden outputs below were captured from.
fn golden_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for version in ["v20.1.0", "v20.10.0", "v18.9.0"] {
        install(dir.path(), &format!("versions/node/{version}"));
    }
    for version in ["v3.0.0", "v2.5.0"] {
        install(dir.path(), &format!("versions/io.js/{version}"));
    }
    alias(dir.path(), "default", "node");
    alias(dir.path(), "work", "v18");
    alias(dir.path(), "lts/*", "lts/iron");
    alias(dir.path(), "lts/iron", "v20.10.0");
    alias(dir.path(), "lts/gallium", "v16.20.2");
    dir
}

const GOLDEN_VERSIONS: &str = "    iojs-v2.5.0 *
    iojs-v3.0.0 *
        v18.9.0 *
        v20.1.0 *
       v20.10.0 *
";

const GOLDEN_ALIASES: &str = "default -> node (-> v20.10.0 *)
work -> v18 (-> v18.9.0 *)
iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)
node -> stable (-> v20.10.0 *) (default)
stable -> 20.10 (-> v20.10.0 *) (default)
unstable -> N/A (default)
lts/* -> lts/iron (-> v20.10.0 *)
lts/gallium -> v16.20.2 (-> N/A)
lts/iron -> v20.10.0 *
";

#[test]
fn ls_matches_nvm_sh() {
    let dir = golden_dir();
    let output = nvm(dir.path(), &["ls"]);
    assert_eq!(
        stdout(&output),
        format!("{GOLDEN_VERSIONS}{GOLDEN_ALIASES}")
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&nvm(dir.path(), &["list"])), stdout(&output));
}

#[test]
fn ls_options_and_patterns_match_nvm_sh() {
    let dir = golden_dir();
    let no_alias = nvm(dir.path(), &["ls", "--no-colors", "--no-alias"]);
    assert_eq!(stdout(&no_alias), GOLDEN_VERSIONS);
    let twenty = nvm(dir.path(), &["ls", "20"]);
    assert_eq!(stdout(&twenty), "        v20.1.0 *\n       v20.10.0 *\n");
    let iojs = nvm(dir.path(), &["ls", "3"]);
    assert_eq!(stdout(&iojs), "    iojs-v3.0.0 *\n");
    let alias = nvm(dir.path(), &["ls", "work"]);
    assert_eq!(stdout(&alias), "        v18.9.0 *\n");
    let missing = nvm(dir.path(), &["ls", "99"]);
    assert_eq!(stdout(&missing), "            N/A\n");
    assert_eq!(missing.status.code(), Some(3));
}

#[test]
fn unsupported_ls_options_exit_55() {
    let dir = golden_dir();
    let bogus = nvm(dir.path(), &["ls", "--bogus"]);
    assert_eq!(bogus.status.code(), Some(55));
    assert_eq!(stderr(&bogus), "Unsupported option \"--bogus\".\n");
    let both = nvm(dir.path(), &["ls", "20", "--no-alias"]);
    assert_eq!(both.status.code(), Some(55));
    assert_eq!(
        stderr(&both),
        "`--no-alias` is not supported when a pattern is provided.\n"
    );
}

#[test]
fn alias_listings_match_nvm_sh() {
    let dir = golden_dir();
    assert_eq!(stdout(&nvm(dir.path(), &["alias"])), GOLDEN_ALIASES);
    let work = nvm(dir.path(), &["alias", "wor"]);
    assert_eq!(stdout(&work), "work -> v18 (-> v18.9.0 *)\n");
    let iojs = nvm(dir.path(), &["alias", "iojs"]);
    assert_eq!(
        stdout(&iojs),
        "iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)\n"
    );
    assert_eq!(
        stdout(&nvm(dir.path(), &["alias", "lts/iron"])),
        "v20.10.0\n"
    );
    let nothing = nvm(dir.path(), &["alias", "nope"]);
    assert!(nothing.stdout.is_empty() && nothing.status.success());
    let missing = nvm(dir.path(), &["alias", "lts/nope"]);
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(stderr(&missing), "Alias does not exist.\n");
    assert_eq!(
        nvm(dir.path(), &["alias", "--bogus"]).status.code(),
        Some(55)
    );
}

#[test]
fn an_empty_nvm_dir_matches_nvm_sh() {
    let dir = tempfile::tempdir().unwrap();
    let output = nvm(dir.path(), &["ls"]);
    assert_eq!(
        stdout(&output),
        "            N/A *
iojs -> N/A (default)
node -> stable (-> N/A) (default)
unstable -> N/A (default)
"
    );
    assert_eq!(output.status.code(), Some(3));
    for pattern in ["20", "node", "iojs", "system", "foo"] {
        let listed = nvm(dir.path(), &["ls", pattern]);
        assert_eq!(stdout(&listed), "            N/A *\n", "{pattern}");
        assert_eq!(listed.status.code(), Some(3));
    }
}

#[test]
fn old_release_lines_match_nvm_sh() {
    let dir = tempfile::tempdir().unwrap();
    for version in ["v0.10.48", "v0.11.16", "v0.12.18", "v4.2.0"] {
        install(dir.path(), &format!("versions/node/{version}"));
    }
    fs::create_dir_all(dir.path().join("alias")).unwrap();
    fs::write(dir.path().join("alias/commented"), "v4 # four\n").unwrap();
    fs::write(dir.path().join("alias/blankfirst"), "\n\nv0.10\n").unwrap();
    fs::write(dir.path().join("alias/a"), "b\n").unwrap();
    fs::write(dir.path().join("alias/b"), "a\n").unwrap();
    let output = nvm(dir.path(), &["alias"]);
    assert_eq!(
        stdout(&output),
        "a -> b (-> ∞)
b -> a (-> ∞)
blankfirst -> v0.10 (-> v0.10.48 *)
commented -> v4 (-> v4.2.0 *)
iojs -> N/A (default)
node -> stable (-> v4.2.0 *) (default)
stable -> 4.2 (-> v4.2.0 *) (default)
unstable -> 0.11 (-> v0.11.16 *) (default)
"
    );
    assert_eq!(
        stdout(&nvm(dir.path(), &["version", "unstable"])),
        "v0.11.16\n"
    );
}

#[cfg(unix)]
#[test]
fn a_real_system_node_is_listed_with_the_version_it_prints() {
    use std::os::unix::fs::PermissionsExt;

    let dir = golden_dir();
    let system = tempfile::tempdir().unwrap();
    let node = system.path().join("node");
    fs::write(&node, "#!/bin/sh\necho v22.1.0\n").unwrap();
    fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths([system.path()]).unwrap();
    let bin = env!("CARGO_BIN_EXE_nvm");

    let listed = run(bin, dir.path(), &path, &["ls", "--no-alias"]);
    let expected = format!("{GOLDEN_VERSIONS}->       system * (-> v22.1.0)\n");
    assert_eq!(stdout(&listed), expected);
    let only = run(bin, dir.path(), &path, &["ls", "system"]);
    assert_eq!(stdout(&only), "->       system * (-> v22.1.0)\n");
    let sys_alias = run(bin, dir.path(), &path, &["alias", "default", "system"]);
    assert_eq!(stdout(&sys_alias), "default -> system *\n");
}

#[test]
fn a_failed_alias_write_names_the_path() {
    let dir = golden_dir();
    let output = nvm(dir.path(), &["alias", "lts", "20"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    let expected_path = dir.path().join("alias/lts");
    assert!(
        message.starts_with(&expected_path.display().to_string()),
        "{message}"
    );
}

#[test]
fn both_binaries_list_the_same() {
    let dir = golden_dir();
    let path = OsStr::new("/nonexistent");
    let from_nvm = run(env!("CARGO_BIN_EXE_nvm"), dir.path(), path, &["ls"]);
    let from_nvmrc = run(env!("CARGO_BIN_EXE_nvmrc"), dir.path(), path, &["ls"]);
    assert_eq!(from_nvm.stdout, from_nvmrc.stdout);
}

/// `nvm.sh` never reads the alias files of the bare names `node` and `iojs`,
/// but follows a `stable` file wherever `node` stands for `stable`.
#[test]
fn alias_files_named_like_built_ins_match_nvm_sh() {
    let dir = golden_dir();
    let binary = |version: &str| format!("{}/bin/node\n", dir.path().join(version).display());
    alias(dir.path(), "node", "18");
    assert_eq!(stdout(&nvm(dir.path(), &["version", "node"])), "v20.10.0\n");
    let which = nvm(dir.path(), &["which", "node"]);
    assert_eq!(stdout(&which), binary("versions/node/v20.10.0"));
    let listed = nvm(dir.path(), &["alias", "default"]);
    assert_eq!(stdout(&listed), "default -> node (-> v20.10.0 *)\n");
    assert_eq!(
        stdout(&nvm(dir.path(), &["version", "default"])),
        "v18.9.0\n"
    );

    fs::remove_file(dir.path().join("alias/node")).unwrap();
    alias(dir.path(), "stable", "18");
    assert_eq!(stdout(&nvm(dir.path(), &["version", "node"])), "v18.9.0\n");
    assert_eq!(
        stdout(&nvm(dir.path(), &["version", "default"])),
        "v18.9.0\n"
    );
    assert_eq!(
        stdout(&nvm(dir.path(), &["ls", "default"])),
        "        v18.9.0 *\n"
    );
    let which = nvm(dir.path(), &["which", "default"]);
    assert_eq!(stdout(&which), binary("versions/node/v18.9.0"));
    let listed = nvm(dir.path(), &["alias", "default"]);
    assert_eq!(stdout(&listed), "default -> node (-> v18.9.0 *)\n");

    alias(dir.path(), "iojs", "iojs");
    let iojs = nvm(dir.path(), &["version", "iojs"]);
    assert_eq!(
        (stdout(&iojs), iojs.status.code()),
        ("iojs-v3.0.0\n".into(), Some(0))
    );
    let listed = nvm(dir.path(), &["alias", "iojs"]);
    assert_eq!(stdout(&listed), "iojs -> iojs (-> iojs-v3.0.0 *)\n");
}

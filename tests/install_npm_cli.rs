//! End-to-end: what `install` does with `npm` once a version is in place
//! (`--latest-npm`, `default-packages`, `--reinstall-packages-from`) and the
//! commands `install-latest-npm` and `reinstall-packages`, against a mirror
//! whose archives hold a small shell script for `npm`.
#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::{Mirror, nvm_with, stderr, stdout};

/// An `npm` that remembers its version and every `install` it was asked for,
/// both next to itself, and lists the packages in `../packages.txt`.
const FAKE_NPM: &str = r#"#!/bin/sh
HERE=$(cd "$(dirname "$0")" && pwd)
STATE="$HERE/../npm-state"
mkdir -p "$STATE"
case "$1" in
  --version) cat "$STATE/version" 2>/dev/null || echo 10.2.3 ;;
  install)
    shift; shift
    echo "$*" >> "$STATE/installs"
    case "$1" in npm@*) echo "${1#npm@}.99.0" > "$STATE/version" ;; esac ;;
  list) if [ -f "$HERE/../packages.txt" ]; then cat "$HERE/../packages.txt"; else echo "$HERE/../lib"; fi ;;
  root) echo "$HERE/../lib/node_modules" ;;
  link) echo "linked $PWD" >> "$STATE/links" ;;
esac
"#;

fn mirror() -> Option<String> {
    let extra = [("bin/npm", FAKE_NPM, 0o755)];
    Some(Mirror::new().binary("v20.10.0", &extra, None)?.serve())
}

fn installs(home: &Path, version: &str) -> String {
    let file = home
        .join("versions/node")
        .join(version)
        .join("npm-state/installs");
    fs::read_to_string(file).unwrap_or_default()
}

fn path_of(home: &Path, version: &str) -> String {
    let bin = home.join("versions/node").join(version).join("bin");
    format!("{}:/usr/bin:/bin", bin.display())
}

#[test]
fn latest_npm_upgrades_the_npm_of_the_new_version() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm_with(
        home.path(),
        &mirror,
        &["install", "--latest-npm", "20"],
        "/usr/bin:/bin",
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Attempting to upgrade to the latest working version of npm..."));
    assert!(out.contains("* `npm` `v10.x` is the last version that works on `node` `< v20.17`"));
    assert!(out.contains("* npm upgraded to: v10.99.0"));
    assert_eq!(installs(home.path(), "v20.10.0"), "npm@10\n");
}

#[test]
fn the_default_packages_are_installed_in_one_command() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("default-packages"),
        "# mine\nyarn\n\n@scope/tool\n",
    )
    .unwrap();
    let output = nvm_with(
        home.path(),
        &mirror,
        &["install", "20"],
        "/usr/bin:/bin",
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("npm install -g --quiet  yarn @scope/tool"));
    assert_eq!(
        installs(home.path(), "v20.10.0"),
        "--quiet yarn @scope/tool\n"
    );
}

#[test]
fn skip_default_packages_does_not_install_them() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("default-packages"), "yarn\n").unwrap();
    let args = ["install", "--skip-default-packages", "20"];
    nvm_with(home.path(), &mirror, &args, "/usr/bin:/bin", &[]);
    assert_eq!(installs(home.path(), "v20.10.0"), "");
}

#[test]
fn reinstall_packages_from_another_version_installs_and_links_them() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    let old = home.path().join("versions/node/v18.19.0");
    fs::create_dir_all(old.join("bin")).unwrap();
    fs::write(old.join("bin/node"), "#!/bin/sh\necho v18.19.0\n").unwrap();
    fs::write(old.join("bin/npm"), FAKE_NPM).unwrap();
    for name in ["node", "npm"] {
        let path = old.join("bin").join(name);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let link_target = home.path().join("src/mylink");
    fs::create_dir_all(&link_target).unwrap();
    let listing = format!(
        "/x/lib\n├── yarn@1.22.19\n├── mylink@1.0.0 -> {}\n",
        link_target.display()
    );
    fs::write(old.join("packages.txt"), listing).unwrap();
    let args = ["install", "--reinstall-packages-from=18", "20"];
    let output = nvm_with(home.path(), &mirror, &args, "/usr/bin:/bin", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("Reinstalling global packages from v18.19.0..."));
    assert_eq!(installs(home.path(), "v20.10.0"), "--quiet yarn@1.22.19\n");
    let links =
        fs::read_to_string(home.path().join("versions/node/v20.10.0/npm-state/links")).unwrap();
    assert!(links.starts_with("linked "));
}

#[test]
fn install_latest_npm_upgrades_the_npm_in_use() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    nvm_with(
        home.path(),
        &mirror,
        &["install", "20"],
        "/usr/bin:/bin",
        &[],
    );
    let path = path_of(home.path(), "v20.10.0");
    let output = nvm_with(home.path(), &mirror, &["install-latest-npm"], &path, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("* npm upgraded to: v10.99.0"));
}

#[test]
fn a_version_without_npm_skips_the_npm_steps_with_a_warning() {
    let Some(mirror) = common::mirror(None) else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let args = ["install", "--latest-npm", "20"];
    let output = nvm_with(home.path(), &mirror, &args, "/usr/bin:/bin", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stderr(&output).contains("npm was not found in v20.10.0; skipping the npm upgrade."));
}

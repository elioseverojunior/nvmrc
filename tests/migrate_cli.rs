//! End-to-end: `nvm migrate` with the real binary against a temporary
//! `$HOME` (no real dotfile is touched), the real file system and the real
//! `bash -n`.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const PROFILE: &str = "export PATH=\"$HOME/bin:$PATH\"\n\
export NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n\
[ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm\n\
[ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion\n\
alias ll='ls -l'\n";

/// `nvm <args>` with `HOME` set to `home`, bash as the login shell, no
/// terminal on stdin and nothing else from this environment.
fn nvm(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nvm"))
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("SHELL", "/bin/bash")
        .env("PATH", "/usr/bin:/bin")
        .env("NVM_DIR", home.join(".nvm"))
        .current_dir(home)
        .stdin(Stdio::null())
        .output()
        .expect("run the binary")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Every entry under `directory`: file contents, or the target of a link.
fn snapshot(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).unwrap();
            entries.insert(path, target.into_os_string().into_encoded_bytes());
        } else if metadata.is_dir() {
            entries.extend(snapshot(&path));
        } else {
            let contents = fs::read(&path).unwrap();
            entries.insert(path, contents);
        }
    }
    entries
}

/// The backups of `name` in `directory`.
fn backups(directory: &Path, name: &str) -> Vec<PathBuf> {
    let prefix = format!("{name}.nvmrc-backup-");
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|file| file.to_string_lossy().starts_with(&prefix))
        })
        .collect()
}

fn bash_accepts(path: &Path) -> bool {
    Command::new("bash")
        .arg("-n")
        .arg(path)
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn migrate_rewrites_the_profile_and_undo_restores_it_byte_for_byte() {
    let home = TempDir::new().unwrap();
    let profile = home.path().join(".bash_profile");
    fs::write(&profile, PROFILE).unwrap();
    let migrated = nvm(home.path(), &["migrate", "--yes"]);
    assert_eq!(
        migrated.status.code(),
        Some(0),
        "{}",
        text(&migrated.stderr)
    );
    let new = fs::read_to_string(&profile).unwrap();
    assert!(new.contains("eval \"$(nvmrc init bash)\"\n"), "{new}");
    assert!(new.contains("# [nvmrc-migrated] [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ]"));
    assert!(bash_accepts(&profile));
    let saved = backups(home.path(), ".bash_profile");
    assert_eq!(saved.len(), 1);
    assert_eq!(fs::read_to_string(&saved[0]).unwrap(), PROFILE);
    let doctor = nvm(home.path(), &["doctor"]);
    assert_eq!(doctor.status.code(), Some(0), "{}", text(&doctor.stdout));
    let undone = nvm(home.path(), &["migrate", "--undo", "--yes"]);
    assert_eq!(undone.status.code(), Some(0), "{}", text(&undone.stderr));
    assert_eq!(fs::read(&profile).unwrap(), PROFILE.as_bytes());
    assert_eq!(backups(home.path(), ".bash_profile").len(), 1);
}

/// A stow-like `~/.bash_profile` linked to `dotfiles/bash/.bash_profile`
/// (mode 0640): the repository directory, the target and the link.
fn stowed_profile(home: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let repository = home.join("dotfiles/bash");
    fs::create_dir_all(&repository).unwrap();
    let target = repository.join(".bash_profile");
    fs::write(&target, PROFILE).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    let link = home.join(".bash_profile");
    symlink("dotfiles/bash/.bash_profile", &link).unwrap();
    (repository, target, link)
}

#[test]
fn a_symlinked_profile_keeps_its_link_and_permissions() {
    let home = TempDir::new().unwrap();
    let (repository, target, link) = stowed_profile(home.path());
    let migrated = nvm(home.path(), &["migrate", "--yes"]);
    assert_eq!(
        migrated.status.code(),
        Some(0),
        "{}",
        text(&migrated.stderr)
    );
    let kept = |path: &Path| {
        let link_kind = fs::symlink_metadata(path).unwrap().file_type();
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        (link_kind.is_symlink(), mode)
    };
    assert_eq!(kept(&link), (true, 0o640));
    assert!(
        fs::read_to_string(&target)
            .unwrap()
            .contains("nvmrc init bash")
    );
    let saved = backups(home.path(), ".bash_profile");
    assert_eq!(saved.len(), 1);
    assert!(fs::symlink_metadata(&saved[0]).unwrap().is_file());
    assert!(backups(&repository, ".bash_profile").is_empty());
    let undone = nvm(home.path(), &["migrate", "--undo", "--yes"]);
    assert_eq!(undone.status.code(), Some(0), "{}", text(&undone.stderr));
    assert_eq!(fs::read(&target).unwrap(), PROFILE.as_bytes());
    assert_eq!(kept(&link), (true, 0o640));
}

#[test]
fn dry_run_changes_nothing() {
    let home = TempDir::new().unwrap();
    fs::write(home.path().join(".bash_profile"), PROFILE).unwrap();
    let before = snapshot(home.path());
    let output = nvm(home.path(), &["migrate", "--dry-run"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("+eval \"$(nvmrc init bash)\""));
    assert_eq!(snapshot(home.path()), before);
}

#[test]
fn an_if_whose_body_would_be_emptied_is_refused() {
    let home = TempDir::new().unwrap();
    let bashrc = home.path().join(".bashrc");
    // The block goes before the first loader, so the `if` below it is left
    // with an empty body once its line is commented out.
    let original = "[ -s \"$HOME/.nvm/nvm.sh\" ] && \\. \"$HOME/.nvm/nvm.sh\"\n\
if [ -d \"$HOME/.nvm\" ]; then\n\
[ -s \"$HOME/.nvm/bash_completion\" ] && \\. \"$HOME/.nvm/bash_completion\"\n\
fi\n";
    fs::write(&bashrc, original).unwrap();
    let output = nvm(home.path(), &["migrate", "--yes"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("would not pass `bash -n`; left unchanged"),
        "{}",
        text(&output.stderr)
    );
    assert_eq!(fs::read(&bashrc).unwrap(), original.as_bytes());
}

#[test]
fn without_yes_and_without_a_terminal_nothing_changes() {
    let home = TempDir::new().unwrap();
    fs::write(home.path().join(".bash_profile"), PROFILE).unwrap();
    let before = snapshot(home.path());
    let output = nvm(home.path(), &["migrate"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stderr),
        "nvm migrate: confirmation needs a terminal: pass --yes\n"
    );
    assert_eq!(snapshot(home.path()), before);
}

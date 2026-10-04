use std::sync::Mutex;

use super::*;

fn names_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn accept(_temporary: &Path) -> io::Result<()> {
    Ok(())
}

#[test]
fn replace_file_replaces_a_plain_file_and_verifies_the_new_contents_first() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".bashrc");
    fs::write(&file, "old").unwrap();
    let seen = Mutex::new(Vec::new());
    let verify = |temporary: &Path| {
        let contents = fs::read_to_string(temporary)?;
        seen.lock()
            .unwrap()
            .push((temporary.to_path_buf(), contents));
        Ok(())
    };
    StdFileSystem.replace_file(&file, "new", &verify).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "new");
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0.parent(), Some(root.path()));
    assert!(
        seen[0]
            .0
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".nvmrc-tmp-")
    );
    assert_eq!(seen[0].1, "new");
    assert_eq!(names_in(root.path()), [".bashrc"]);
}

#[cfg(unix)]
#[test]
fn replace_file_creates_a_missing_file_with_mode_644() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".kshrc");
    StdFileSystem.replace_file(&file, "x", &accept).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "x");
    let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644);
}

/// A stow-style `home/.zshrc -> ../dotfiles/zsh/.zshrc`, the target in mode
/// 0o640: returns the home and dotfiles directories.
#[cfg(unix)]
fn stowed_zshrc(root: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let (home, dotfiles) = (root.join("home"), root.join("dotfiles/zsh"));
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&dotfiles).unwrap();
    let target = dotfiles.join(".zshrc");
    fs::write(&target, "old").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    std::os::unix::fs::symlink("../dotfiles/zsh/.zshrc", home.join(".zshrc")).unwrap();
    (home, dotfiles)
}

#[cfg(unix)]
#[test]
fn replace_file_writes_through_a_relative_symlink_keeping_the_link_and_the_mode() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let (home, dotfiles) = stowed_zshrc(root.path());
    let (link, target) = (home.join(".zshrc"), dotfiles.join(".zshrc"));
    let target_directory = fs::canonicalize(&dotfiles).unwrap();
    let verify = |temporary: &Path| {
        assert_eq!(temporary.parent(), Some(target_directory.as_path()));
        Ok(())
    };
    StdFileSystem.replace_file(&link, "new", &verify).unwrap();
    // `read_link` fails on anything but a symbolic link: the link survived.
    let kept = fs::read_link(&link).unwrap();
    assert_eq!(kept, Path::new("../dotfiles/zsh/.zshrc"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640);
    assert_eq!(names_in(&dotfiles), [".zshrc"]);
    assert_eq!(names_in(&home), [".zshrc"]);
}

#[test]
fn replace_file_leaves_the_target_untouched_when_verify_fails() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".profile");
    fs::write(&file, "old").unwrap();
    let refuse = |_temporary: &Path| Err(io::Error::other("syntax error"));
    let error = StdFileSystem
        .replace_file(&file, "new", &refuse)
        .unwrap_err();
    assert_eq!(error.to_string(), "syntax error");
    assert_eq!(fs::read_to_string(&file).unwrap(), "old");
    assert_eq!(names_in(root.path()), [".profile"]);
}

#[cfg(unix)]
#[test]
fn replace_file_refuses_a_dangling_symlink() {
    let root = tempfile::tempdir().unwrap();
    let link = root.path().join(".zshrc");
    std::os::unix::fs::symlink("missing/.zshrc", &link).unwrap();
    let error = StdFileSystem
        .replace_file(&link, "new", &accept)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(names_in(root.path()), [".zshrc"]);
}

#[test]
fn replace_file_gives_concurrent_writers_unique_temporary_files() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".bashrc");
    fs::write(&file, "old").unwrap();
    let temporaries = Mutex::new(Vec::new());
    let record = |temporary: &Path| {
        temporaries.lock().unwrap().push(temporary.to_path_buf());
        Ok(())
    };
    std::thread::scope(|scope| {
        for writer in 0..8 {
            let (file, record) = (&file, &record);
            scope.spawn(move || {
                let contents = format!("writer {writer}");
                StdFileSystem.replace_file(file, &contents, record).unwrap();
            });
        }
    });
    let mut temporaries = temporaries.into_inner().unwrap();
    temporaries.sort();
    temporaries.dedup();
    assert_eq!(temporaries.len(), 8);
    assert!(fs::read_to_string(&file).unwrap().starts_with("writer "));
    assert_eq!(names_in(root.path()), [".bashrc"]);
}

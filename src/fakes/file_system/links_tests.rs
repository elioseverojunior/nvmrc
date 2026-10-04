use std::cell::RefCell;

use super::*;

fn path(text: &str) -> PathBuf {
    PathBuf::from(text)
}

fn stowed() -> FakeFileSystem {
    let fs = FakeFileSystem::default()
        .with_file("/dotfiles/zsh/.zshrc", "old")
        .with_file("/real/sub/f", "x")
        .with_dir("/home");
    fs.symlink(
        Path::new("../dotfiles/zsh/.zshrc"),
        Path::new("/home/.zshrc"),
    )
    .unwrap();
    fs
}

#[test]
fn canonicalize_resolves_relative_absolute_chained_and_directory_links() {
    let fs = stowed();
    fs.symlink(Path::new("/real"), Path::new("/dir-link"))
        .unwrap();
    fs.symlink(Path::new("/home/.zshrc"), Path::new("/chain"))
        .unwrap();
    let canonical = |text: &str| fs.canonicalize(Path::new(text)).unwrap();
    assert_eq!(canonical("/home/.zshrc"), path("/dotfiles/zsh/.zshrc"));
    assert_eq!(canonical("/chain"), path("/dotfiles/zsh/.zshrc"));
    assert_eq!(canonical("/dir-link/sub/f"), path("/real/sub/f"));
    assert_eq!(canonical("/real/sub/f"), path("/real/sub/f"));
    assert_eq!(canonical("/real/sub"), path("/real/sub"));
}

#[test]
fn canonicalize_is_not_found_for_a_missing_path_or_a_dangling_link() {
    let fs = stowed();
    fs.symlink(Path::new("gone"), Path::new("/home/dangling"))
        .unwrap();
    fs.symlink(Path::new("/loop-b"), Path::new("/loop-a"))
        .unwrap();
    fs.symlink(Path::new("/loop-a"), Path::new("/loop-b"))
        .unwrap();
    for missing in ["/home/dangling", "/missing", "/loop-a"] {
        let error = fs.canonicalize(Path::new(missing)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound, "{missing}");
    }
}

#[test]
fn reading_through_a_link_reads_its_target() {
    let fs = stowed();
    assert_eq!(fs.read_to_string(Path::new("/home/.zshrc")).unwrap(), "old");
    assert!(fs.is_file(Path::new("/home/.zshrc")));
}

#[test]
fn replace_file_writes_through_the_link_and_keeps_it() {
    let fs = stowed();
    let seen = RefCell::new(Vec::new());
    let verify = |temporary: &Path| {
        seen.borrow_mut()
            .push((temporary.to_path_buf(), fs.read_to_string(temporary)?));
        Ok(())
    };
    fs.replace_file(Path::new("/home/.zshrc"), "new", &verify)
        .unwrap();
    let link = fs.read_link(Path::new("/home/.zshrc")).unwrap();
    assert_eq!(link, path("../dotfiles/zsh/.zshrc"));
    assert_eq!(
        fs.read_to_string(Path::new("/dotfiles/zsh/.zshrc"))
            .unwrap(),
        "new"
    );
    let seen = seen.into_inner();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0.parent(), Some(Path::new("/dotfiles/zsh")));
    assert_eq!(seen[0].1, "new");
    let listed = fs.read_dir(Path::new("/dotfiles/zsh")).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(fs.replaced(), [path("/home/.zshrc")]);
}

#[test]
fn replace_file_keeps_the_target_and_drops_the_temporary_when_verify_fails() {
    let fs = stowed();
    let refuse = |_temporary: &Path| Err(io::Error::other("syntax error"));
    let error = fs
        .replace_file(Path::new("/home/.zshrc"), "new", &refuse)
        .unwrap_err();
    assert_eq!(error.to_string(), "syntax error");
    assert_eq!(
        fs.read_to_string(Path::new("/dotfiles/zsh/.zshrc"))
            .unwrap(),
        "old"
    );
    assert_eq!(fs.read_dir(Path::new("/dotfiles/zsh")).unwrap().len(), 1);
    assert!(fs.replaced().is_empty());
}

#[test]
fn replace_file_refuses_a_dangling_link_and_creates_a_plain_missing_file() {
    let fs = stowed();
    fs.symlink(Path::new("gone"), Path::new("/home/dangling"))
        .unwrap();
    let accept = |_temporary: &Path| Ok(());
    let error = fs
        .replace_file(Path::new("/home/dangling"), "x", &accept)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(!fs.is_file(Path::new("/home/gone")));
    fs.replace_file(Path::new("/home/.kshrc"), "y", &accept)
        .unwrap();
    assert_eq!(fs.read_to_string(Path::new("/home/.kshrc")).unwrap(), "y");
    // The fake lists files and directories, not links: no temporary is left.
    let listed = fs.read_dir(Path::new("/home")).unwrap();
    let names: Vec<&str> = listed.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".kshrc"]);
}

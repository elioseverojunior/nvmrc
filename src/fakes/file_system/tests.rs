use super::*;

fn entry(name: &str, is_dir: bool) -> DirEntry {
    DirEntry {
        name: name.to_owned(),
        is_dir,
    }
}

#[test]
fn fake_file_system_lists_direct_children_with_their_kind() {
    let fs = FakeFileSystem::default()
        .with_file("/d/a/x", "1")
        .with_file("/d/a/y", "2")
        .with_file("/d/b", "3");
    let root = fs.read_dir(Path::new("/d")).unwrap();
    assert_eq!(root, [entry("a", true), entry("b", false)]);
    let nested = fs.read_dir(Path::new("/d/a")).unwrap();
    assert_eq!(nested, [entry("x", false), entry("y", false)]);
}

#[test]
fn fake_file_system_removes_a_whole_tree_and_not_its_siblings() {
    let fs = FakeFileSystem::default()
        .with_file("/d/a/x", "1")
        .with_dir("/d/a/empty")
        .with_file("/d/ab", "2");
    fs.remove_dir_all(Path::new("/d/a")).unwrap();
    assert_eq!(fs.read_dir(Path::new("/d")).unwrap(), [entry("ab", false)]);
    fs.remove_dir_all(Path::new("/d/missing")).unwrap();
}

#[test]
fn fake_file_system_reports_what_a_path_is() {
    let fs = FakeFileSystem::default()
        .with_executable("/d/bin/node", "abc")
        .with_file("/d/plain", "")
        .with_dir("/d/empty");
    let node = fs.file_info(Path::new("/d/bin/node")).unwrap();
    assert_eq!((node.is_dir, node.len, node.executable), (false, 3, true));
    assert!(!fs.file_info(Path::new("/d/plain")).unwrap().executable);
    assert!(fs.file_info(Path::new("/d/bin")).unwrap().is_dir);
    assert!(fs.file_info(Path::new("/d/empty")).unwrap().is_dir);
    assert!(fs.file_info(Path::new("/nope")).is_err());
}

#[test]
fn fake_file_system_reports_when_a_path_was_changed() {
    let time = SystemTime::UNIX_EPOCH;
    let fs = FakeFileSystem::default()
        .with_file("/d/f", "")
        .with_modified("/d/f", time);
    assert_eq!(
        fs.file_info(Path::new("/d/f")).unwrap().modified,
        Some(time)
    );
}

#[test]
fn fake_file_system_renames_a_tree_and_creates_a_directory_once() {
    let fs = FakeFileSystem::default()
        .with_file("/a/x/y", "1")
        .with_executable("/a/bin/tool", "t")
        .with_dir("/a/empty");
    fs.rename(Path::new("/a"), Path::new("/b")).unwrap();
    assert_eq!(fs.read_to_string(Path::new("/b/x/y")).unwrap(), "1");
    assert!(fs.file_info(Path::new("/b/empty")).unwrap().is_dir);
    assert!(!fs.file_info(Path::new("/b/x/y")).unwrap().executable);
    assert!(fs.file_info(Path::new("/b/bin/tool")).unwrap().executable);
    assert!(fs.file_info(Path::new("/a")).is_err());
    assert!(fs.rename(Path::new("/a"), Path::new("/c")).is_err());
    fs.create_dir(Path::new("/lock")).unwrap();
    let again = fs.create_dir(Path::new("/lock")).unwrap_err();
    assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
}

#[test]
fn fake_file_system_keeps_bytes_and_rejects_non_text_as_a_string() {
    let fs = FakeFileSystem::default();
    fs.write_bytes(Path::new("/blob"), &[0, 255]).unwrap();
    let error = fs.read_to_string(Path::new("/blob")).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn fake_file_system_models_empty_directories() {
    let fs = FakeFileSystem::default().with_dir("/d/empty");
    assert_eq!(fs.read_dir(Path::new("/d/empty")).unwrap(), []);
    assert_eq!(
        fs.read_dir(Path::new("/d")).unwrap(),
        [entry("empty", true)]
    );
}

#[test]
fn fake_file_system_fails_on_missing_paths_and_on_files() {
    let fs = FakeFileSystem::default().with_file("/d/f", "x");
    assert!(fs.read_dir(Path::new("/missing")).is_err());
    assert!(fs.read_dir(Path::new("/d/f")).is_err());
}

#[test]
fn fake_file_system_reads_files() {
    let fs = FakeFileSystem::default().with_file("/f", "hi");
    assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "hi");
    assert!(fs.read_to_string(Path::new("/g")).is_err());
    assert!(fs.is_file(Path::new("/f")));
    assert!(!fs.is_file(Path::new("/g")));
}

#[test]
fn fake_file_system_removes_files() {
    let fs = FakeFileSystem::default().with_file("/f", "hi");
    fs.remove_file(Path::new("/f")).unwrap();
    assert!(!fs.is_file(Path::new("/f")));
    assert!(fs.remove_file(Path::new("/f")).is_err());
}

#[test]
fn fake_file_system_writes_and_overwrites_files() {
    let fs = FakeFileSystem::default();
    fs.write_file(Path::new("/f"), "one").unwrap();
    fs.write_file(Path::new("/f"), "two").unwrap();
    assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "two");
}

#[test]
fn fake_file_system_creates_directories() {
    let fs = FakeFileSystem::default();
    fs.create_dir_all(Path::new("/d/e")).unwrap();
    fs.create_dir_all(Path::new("/d/e")).unwrap();
    assert_eq!(fs.read_dir(Path::new("/d/e")).unwrap(), []);
}

#[test]
fn fake_file_system_remembers_symlinks_and_refuses_to_overwrite() {
    let fs = FakeFileSystem::default().with_file("/d/file", "x");
    fs.symlink(Path::new("/v/1"), Path::new("/d/current"))
        .unwrap();
    let link = fs.read_link(Path::new("/d/current")).unwrap();
    assert_eq!(link, Path::new("/v/1"));
    for existing in ["/d/current", "/d/file"] {
        let error = fs
            .symlink(Path::new("/v/2"), Path::new(existing))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    }
    let missing = fs.read_link(Path::new("/d/file")).unwrap_err();
    assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    fs.remove_file(Path::new("/d/current")).unwrap();
    assert!(fs.read_link(Path::new("/d/current")).is_err());
    fs.symlink(Path::new("/v/2"), Path::new("/d/current"))
        .unwrap();
}

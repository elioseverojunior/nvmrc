use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{DirEntry, FileInfo, FileSystem};

pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        fs::read_dir(path)?
            .map(|entry| {
                let entry = entry?;
                Ok(DirEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    is_dir: entry.path().is_dir(),
                })
            })
            .collect()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn file_info(&self, path: &Path) -> io::Result<FileInfo> {
        let metadata = fs::metadata(path)?;
        Ok(FileInfo {
            is_dir: metadata.is_dir(),
            len: metadata.len(),
            executable: is_executable(&metadata),
            modified: metadata.modified().ok(),
        })
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    #[cfg(unix)]
    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(not(unix))]
    fn symlink(&self, _target: &Path, _link: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    #[cfg(unix)]
    fn read_link(&self, link: &Path) -> io::Result<PathBuf> {
        fs::read_link(link)
    }

    #[cfg(not(unix))]
    fn read_link(&self, _link: &Path) -> io::Result<PathBuf> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    #[cfg(unix)]
    fn same_directory(&self, a: &Path, b: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(a), fs::metadata(b)) {
            (Ok(a), Ok(b)) => a.is_dir() && a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }

    #[cfg(not(unix))]
    fn same_directory(&self, a: &Path, b: &Path) -> bool {
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b && a.is_dir(),
            _ => false,
        }
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_dir_all_removes_a_tree_a_file_and_tolerates_a_missing_path() {
        let root = tempfile::tempdir().unwrap();
        let tree = root.path().join("tree");
        fs::create_dir_all(tree.join("a/b")).unwrap();
        fs::write(tree.join("a/b/file"), "x").unwrap();
        let file = root.path().join("file");
        fs::write(&file, "x").unwrap();
        for path in [&tree, &file, &root.path().join("missing")] {
            StdFileSystem.remove_dir_all(path).unwrap();
            assert!(!path.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn remove_dir_all_removes_a_symlink_and_leaves_its_target() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        StdFileSystem.remove_dir_all(&link).unwrap();
        assert!(!link.exists() && target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn same_directory_follows_symlinks_and_tells_directories_apart() {
        let root = tempfile::tempdir().unwrap();
        let (real, other) = (root.path().join("real"), root.path().join("other"));
        fs::create_dir(&real).unwrap();
        fs::create_dir(&other).unwrap();
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(StdFileSystem.same_directory(&link, &real));
        assert!(!StdFileSystem.same_directory(&other, &real));
        assert!(!StdFileSystem.same_directory(&root.path().join("gone"), &real));
    }

    #[test]
    fn file_info_tells_size_kind_and_executability() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("tool");
        fs::write(&file, "abc").unwrap();
        let info = StdFileSystem.file_info(&file).unwrap();
        assert_eq!((info.is_dir, info.len), (false, 3));
        assert!(info.modified.is_some());
        assert!(StdFileSystem.file_info(root.path()).unwrap().is_dir);
        assert!(
            StdFileSystem
                .file_info(&root.path().join("missing"))
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_info_sees_the_execute_bit() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("tool");
        fs::write(&file, "x").unwrap();
        assert!(!StdFileSystem.file_info(&file).unwrap().executable);
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(StdFileSystem.file_info(&file).unwrap().executable);
    }

    #[test]
    fn create_dir_fails_when_the_directory_exists_and_rename_moves_a_tree() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("lock");
        StdFileSystem.create_dir(&first).unwrap();
        let again = StdFileSystem.create_dir(&first).unwrap_err();
        assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
        fs::write(first.join("f"), "x").unwrap();
        let moved = root.path().join("moved");
        StdFileSystem.rename(&first, &moved).unwrap();
        assert!(moved.join("f").is_file() && !first.exists());
    }

    #[test]
    fn write_bytes_stores_what_is_not_text() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("blob");
        StdFileSystem.write_bytes(&file, &[0, 255, 1]).unwrap();
        assert_eq!(fs::read(&file).unwrap(), [0, 255, 1]);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_created_read_back_and_refuses_an_existing_path() {
        let root = tempfile::tempdir().unwrap();
        let link = root.path().join("current");
        StdFileSystem.symlink(Path::new("/v/1"), &link).unwrap();
        assert_eq!(StdFileSystem.read_link(&link).unwrap(), Path::new("/v/1"));
        let again = StdFileSystem.symlink(Path::new("/v/2"), &link).unwrap_err();
        assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
        StdFileSystem.remove_file(&link).unwrap();
        let gone = StdFileSystem.read_link(&link).unwrap_err();
        assert_eq!(gone.kind(), io::ErrorKind::NotFound);
    }
}

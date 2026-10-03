use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::fakes::FakeFileSystem;
use crate::ports::{Archive, FileSystem};

/// One file of a fake archive.
struct Entry {
    path: &'static str,
    contents: &'static str,
    executable: bool,
}

/// Archives by path: unpacking one writes its files into a [`FakeFileSystem`]
/// under the destination; any other path is not found.
pub struct FakeArchive<'a> {
    fs: &'a FakeFileSystem,
    archives: BTreeMap<PathBuf, Vec<Entry>>,
}

impl<'a> FakeArchive<'a> {
    #[must_use]
    pub fn new(fs: &'a FakeFileSystem) -> Self {
        Self {
            fs,
            archives: BTreeMap::new(),
        }
    }

    /// An archive holding `(path, contents, executable)` files.
    #[must_use]
    pub fn with_archive(
        mut self,
        path: &str,
        files: &[(&'static str, &'static str, bool)],
    ) -> Self {
        let entries = files
            .iter()
            .map(|&(path, contents, executable)| Entry {
                path,
                contents,
                executable,
            })
            .collect();
        self.archives.insert(PathBuf::from(path), entries);
        self
    }
}

impl Archive for FakeArchive<'_> {
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()> {
        let entries = self
            .archives
            .get(archive)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        for entry in entries {
            let target = destination.join(entry.path);
            self.fs.write_file(&target, entry.contents)?;
            if entry.executable {
                self.fs.set_executable(&target);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_its_files_under_the_destination() {
        let fs = FakeFileSystem::default();
        let archive = FakeArchive::new(&fs).with_archive("/a.tgz", &[("top/bin/node", "x", true)]);
        archive
            .extract(Path::new("/a.tgz"), Path::new("/out"))
            .unwrap();
        let info = fs.file_info(Path::new("/out/top/bin/node")).unwrap();
        assert!(info.executable);
        assert!(
            archive
                .extract(Path::new("/b.tgz"), Path::new("/out"))
                .is_err()
        );
    }
}

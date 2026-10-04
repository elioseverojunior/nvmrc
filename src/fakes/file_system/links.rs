//! Symbolic links in the fake: resolving them and writing through them.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::FakeFileSystem;
use crate::ports::FileSystem;

/// How many links one path may go through, as the kernel's `ELOOP` limit.
const LINK_LIMIT: usize = 40;

/// `path` with `.` and `..` taken out, without looking at links.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    normal
}

fn not_found(path: &Path, why: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: {why}", path.display()),
    )
}

impl FakeFileSystem {
    /// `path` with every symbolic link it goes through replaced by its target
    /// (a relative target is relative to the link's directory); `None` for a
    /// loop.
    pub(super) fn resolve(&self, path: &Path) -> Option<PathBuf> {
        let mut current = normalize(path);
        for _ in 0..LINK_LIMIT {
            match self.follow_one_link(&current) {
                Some(next) => current = next,
                None => return Some(current),
            }
        }
        None
    }

    fn follow_one_link(&self, path: &Path) -> Option<PathBuf> {
        let links = self.links.borrow();
        links.iter().find_map(|(link, target)| {
            let rest = path.strip_prefix(link).ok()?;
            let directory = link.parent().unwrap_or_else(|| Path::new("/"));
            Some(normalize(&directory.join(target).join(rest)))
        })
    }

    pub(super) fn canonical(&self, path: &Path) -> io::Result<PathBuf> {
        let resolved = self
            .resolve(path)
            .ok_or_else(|| not_found(path, "link loop"))?;
        match self.file_info(&resolved) {
            Ok(_) => Ok(resolved),
            Err(_) => Err(not_found(path, "no such file")),
        }
    }

    /// [`FileSystem::replace_file`] on the maps: the same observable contract
    /// as the real one, plus a record of the paths replaced.
    pub(super) fn replace_through_links(
        &self,
        path: &Path,
        contents: &str,
        verify: &dyn Fn(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        let target = self.replace_target(path)?;
        let temporary = self.temporary_beside(&target);
        self.write_file(&temporary, contents)?;
        let result = verify(&temporary).and_then(|()| self.rename(&temporary, &target));
        match result {
            Ok(()) => self.replaced.borrow_mut().push(path.to_path_buf()),
            Err(_) => {
                self.files.borrow_mut().remove(&temporary);
            }
        }
        result
    }

    /// The end of the link `path`, or the file `path` names (maybe missing).
    fn replace_target(&self, path: &Path) -> io::Result<PathBuf> {
        if self.read_link(path).is_ok() {
            return self
                .canonical(path)
                .map_err(|_| not_found(path, "dangling symbolic link"));
        }
        self.resolve(path)
            .ok_or_else(|| not_found(path, "link loop"))
    }

    fn temporary_beside(&self, target: &Path) -> PathBuf {
        let counter = self.temporaries.get();
        self.temporaries.set(counter + 1);
        let name = format!(".nvmrc-tmp-{}-{counter}", std::process::id());
        target.parent().unwrap_or_else(|| Path::new("/")).join(name)
    }
}

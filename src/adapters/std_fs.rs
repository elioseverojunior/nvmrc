use std::fs;
use std::io;
use std::path::Path;

use crate::ports::FileSystem;

pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>> {
        fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}

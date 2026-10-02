//! `.nvmrc` files: parsing, and the upward search from a directory.

use std::path::{Path, PathBuf};

use crate::ports::FileSystem;

pub const NVMRC_FILE_NAME: &str = ".nvmrc";

/// The version requested by a `.nvmrc`: its first line, without CR or spaces.
#[must_use]
pub fn parse(contents: &str) -> Option<String> {
    let first_line = contents.split('\n').next()?.replace('\r', "");
    let trimmed = first_line.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The nearest `.nvmrc`, looking in `start` and then each parent directory.
#[must_use]
pub fn find_up(fs: &dyn FileSystem, start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|directory| directory.join(NVMRC_FILE_NAME))
        .find(|candidate| fs.is_file(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeFileSystem;

    #[test]
    fn parse_takes_the_first_line() {
        assert_eq!(parse("v20.0.0\nother"), Some("v20.0.0".to_owned()));
    }

    #[test]
    fn parse_drops_carriage_returns_and_spaces() {
        assert_eq!(parse("v20.0.0\r\n"), Some("v20.0.0".to_owned()));
        assert_eq!(parse("  lts/iron \n"), Some("lts/iron".to_owned()));
    }

    #[test]
    fn parse_of_an_empty_file_is_none() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("\r\n"), None);
    }

    #[test]
    fn find_up_walks_to_parent_directories() {
        let fs = FakeFileSystem::default().with_file("/work/proj/.nvmrc", "20");
        let found = find_up(&fs, Path::new("/work/proj/src/deep"));
        assert_eq!(found, Some(PathBuf::from("/work/proj/.nvmrc")));
    }

    #[test]
    fn find_up_prefers_the_nearest_file() {
        let fs = FakeFileSystem::default()
            .with_file("/work/.nvmrc", "18")
            .with_file("/work/proj/.nvmrc", "20");
        let found = find_up(&fs, Path::new("/work/proj"));
        assert_eq!(found, Some(PathBuf::from("/work/proj/.nvmrc")));
    }

    #[test]
    fn find_up_returns_none_when_absent() {
        let fs = FakeFileSystem::default();
        assert_eq!(find_up(&fs, Path::new("/work")), None);
    }
}

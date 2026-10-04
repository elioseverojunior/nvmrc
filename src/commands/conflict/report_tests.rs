use std::path::PathBuf;

use super::{FileFindings, Report};
use crate::domain::conflict::Hit;
use crate::domain::conflict::Kind::{
    self, Completion, HelperCall, LazyStub, Loader, NotFollowed, NvmDirExport, OmzPlugin,
};

fn file(path: &str, depth: usize, kinds: &[Kind]) -> FileFindings {
    let hits = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| Hit {
            line: index + 1,
            kind: *kind,
            text: format!("line {}", index + 1),
        })
        .collect();
    FileFindings {
        path: PathBuf::from(path),
        canonical: PathBuf::from(path),
        depth,
        hits,
    }
}

fn report(files: Vec<FileFindings>) -> Report {
    Report {
        files,
        notes: Vec::new(),
    }
}

fn kinds<'a>(found: impl Iterator<Item = (&'a FileFindings, &'a Hit)>) -> Vec<(String, Kind)> {
    found
        .map(|(file, hit)| (file.path.display().to_string(), hit.kind))
        .collect()
}

#[test]
fn info_and_hints_are_not_conflicts() {
    let quiet = report(vec![file(
        "/h/.zshrc",
        0,
        &[NvmDirExport, HelperCall, OmzPlugin, NotFollowed],
    )]);
    assert!(!quiet.has_conflicts());
    assert_eq!(quiet.conflicts().count(), 0);
    assert!(!Report::default().has_conflicts());
}

#[test]
fn conflicts_are_every_conflicting_hit_of_every_file_in_order() {
    let found = report(vec![
        file("/h/.zshrc", 0, &[NvmDirExport, Loader]),
        file("/h/lazy.zsh", 1, &[LazyStub, HelperCall]),
    ]);
    assert!(found.has_conflicts());
    let expected = [
        ("/h/.zshrc".to_owned(), Loader),
        ("/h/lazy.zsh".to_owned(), LazyStub),
    ];
    assert_eq!(kinds(found.conflicts()), expected);
}

#[test]
fn only_loaders_and_completions_of_roots_are_auto_migratable() {
    let found = report(vec![
        file(
            "/h/.bashrc",
            0,
            &[Loader, NvmDirExport, Completion, LazyStub],
        ),
        file("/h/extra.sh", 1, &[Loader]),
    ]);
    let expected = [
        ("/h/.bashrc".to_owned(), Loader),
        ("/h/.bashrc".to_owned(), Completion),
    ];
    assert_eq!(kinds(found.auto_migratable()), expected);
}

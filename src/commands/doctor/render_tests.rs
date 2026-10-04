use std::path::PathBuf;

use super::render::render;
use crate::commands::conflict::{FileFindings, Note, NoteReason, Report};
use crate::domain::conflict::Hit;
use crate::domain::conflict::Kind;

fn file(path: &str, canonical: &str, depth: usize, hits: &[(usize, Kind, &str)]) -> FileFindings {
    FileFindings {
        path: PathBuf::from(path),
        canonical: PathBuf::from(canonical),
        depth,
        hits: hits
            .iter()
            .map(|(line, kind, text)| Hit {
                line: *line,
                kind: *kind,
                text: (*text).to_owned(),
            })
            .collect(),
    }
}

#[test]
fn a_clean_scan_says_no_conflicts_and_has_no_info() {
    let report = Report {
        files: vec![file("/h/.bashrc", "/h/.bashrc", 0, &[])],
        notes: Vec::new(),
    };
    assert_eq!(
        render(&report),
        "nvm doctor: scanned 1 file(s)\n\nResult: no conflicts"
    );
}

#[test]
fn a_hint_alone_is_shown_but_is_not_a_conflict() {
    let report = Report {
        files: vec![file(
            "/h/.zshrc",
            "/h/.zshrc",
            0,
            &[(3, Kind::OmzPlugin, "    nvm")],
        )],
        notes: Vec::new(),
    };
    let text = render(&report);
    assert!(text.contains("  3: oh-my-zsh nvm plugin  nvm\n      fix: remove `nvm` from"));
    assert!(text.ends_with("Result: no conflicts"));
}

#[test]
fn a_manual_loader_gets_a_patch_line_and_no_migrate_count() {
    let report = Report {
        files: vec![file(
            "/h/lazy.zsh",
            "/h/lazy.zsh",
            1,
            &[(3, Kind::LazyLoader, "  . \"$P/nvm.sh\"")],
        )],
        notes: Vec::new(),
    };
    let text = render(&report);
    assert!(text.contains("      patch: replace the line with: eval \"$(nvmrc init zsh)\""));
    assert!(text.ends_with("Result: 1 conflict(s)"));
}

/// An `NVM_DIR` export, a path that cannot be followed, a file not read.
fn exports_and_notes() -> Report {
    Report {
        files: vec![file(
            "/h/.zshenv",
            "/h/.zshenv",
            0,
            &[
                (1, Kind::NvmDirExport, "export NVM_DIR=x"),
                (2, Kind::NotFollowed, "source $X"),
            ],
        )],
        notes: vec![
            Note {
                path: PathBuf::from("/h/.zshenv"),
                line: Some(2),
                text: "source $X".to_owned(),
                reason: NoteReason::Unresolvable("unset $X".to_owned()),
            },
            Note {
                path: PathBuf::from("/h/gone"),
                line: None,
                text: String::new(),
                reason: NoteReason::Unreadable("denied".to_owned()),
            },
        ],
    }
}

#[test]
fn info_lists_the_kept_exports_then_the_notes() {
    assert_eq!(
        render(&exports_and_notes()),
        "nvm doctor: scanned 1 file(s)\n\n\
Info:\n  /h/.zshenv:1: NVM_DIR export: kept by `nvm migrate`\n  \
/h/.zshenv:2: not followed: unset $X\n  /h/gone: not read: denied\n\n\
Result: no conflicts"
    );
}

#[test]
fn note_reasons_have_their_text() {
    let path = PathBuf::from("/h/x");
    assert_eq!(
        NoteReason::Relative(path.clone()).to_string(),
        "relative path resolved from the file's directory: /h/x"
    );
    assert_eq!(
        NoteReason::Missing(path.clone()).to_string(),
        "file not found: /h/x"
    );
    assert_eq!(
        NoteReason::TooDeep(path).to_string(),
        "not scanned: deeper than 2 levels"
    );
}

//! The text of `nvm doctor`: one block per file, the Info section, the verdict.

use std::fmt::Write as _;

use super::fix::{fix_text, suggested_patch};
use crate::commands::conflict::{FileFindings, Note, NoteReason, Report};
use crate::domain::conflict::{Hit, Kind, Severity};

/// Kinds that are told in the Info section, not in a file's block.
fn is_info(kind: Kind) -> bool {
    kind.severity() == Severity::Info
}

/// The whole report as text, without a trailing newline.
pub fn render(report: &Report) -> String {
    let mut sections = vec![format!(
        "nvm doctor: scanned {} file(s)",
        report.files.len()
    )];
    sections.extend(report.files.iter().filter_map(file_block));
    sections.extend(info_section(report));
    sections.extend(warning_section(report));
    sections.push(verdict(report));
    sections.join("\n\n")
}

fn file_block(file: &FileFindings) -> Option<String> {
    let hits: Vec<&Hit> = file.hits.iter().filter(|hit| !is_info(hit.kind)).collect();
    if hits.is_empty() {
        return None;
    }
    let mut block = file.path.display().to_string();
    if file.canonical != file.path {
        let _ = write!(block, " -> {}", file.canonical.display());
    }
    for hit in hits {
        block.push('\n');
        block.push_str(&hit_lines(file, hit));
    }
    Some(block)
}

fn hit_lines(file: &FileFindings, hit: &Hit) -> String {
    let mut lines = format!(
        "  {}: {}  {}\n      fix: {}",
        hit.line,
        hit.kind,
        hit.text.trim(),
        fix_text(hit.kind, file.depth)
    );
    if let Some(patch) = suggested_patch(hit.kind, file.depth, &file.path, &hit.text) {
        let _ = write!(lines, "\n      patch: {patch}");
    }
    lines
}

fn info_section(report: &Report) -> Option<String> {
    let kept = report.files.iter().flat_map(|file| {
        file.hits
            .iter()
            .filter(|hit| hit.kind == Kind::NvmDirExport)
            .map(move |hit| {
                let text = format!("{}: {}", hit.kind, fix_text(hit.kind, file.depth));
                format!("  {}:{}: {text}", file.path.display(), hit.line)
            })
    });
    let notes = report
        .notes
        .iter()
        .filter(|note| !is_unread(note))
        .map(|note| match note.line {
            Some(line) => format!("  {}:{line}: {}", note.path.display(), note.reason),
            None => format!("  {}: {}", note.path.display(), note.reason),
        });
    let lines: Vec<String> = kept.chain(notes).collect();
    (!lines.is_empty()).then(|| format!("Info:\n{}", lines.join("\n")))
}

/// Whether `note` is about a file that could not be read (told apart, in
/// the warning section).
fn is_unread(note: &Note) -> bool {
    matches!(note.reason, NoteReason::Unreadable(_))
}

/// The files that could not be read: unknown, so never clean.
fn warning_section(report: &Report) -> Option<String> {
    let lines: Vec<String> = report
        .notes
        .iter()
        .filter(|note| is_unread(note))
        .map(|note| format!("  {}: {}", note.path.display(), note.reason))
        .collect();
    (!lines.is_empty()).then(|| {
        format!(
            "Warning: not scanned, so they may still load nvm.sh:\n{}",
            lines.join("\n")
        )
    })
}

fn verdict(report: &Report) -> String {
    let conflicts = report.conflicts().count();
    let unread = report.unreadable().count();
    let fixable = report.auto_migratable().count();
    let mut verdict = match (conflicts, fixable) {
        (0, _) if unread > 0 => "Result: no conflicts found".to_owned(),
        (0, _) => "Result: no conflicts".to_owned(),
        (_, 0) => format!("Result: {conflicts} conflict(s)"),
        _ => format!("Result: {conflicts} conflict(s), {fixable} can be fixed with `nvm migrate`"),
    };
    if unread > 0 {
        let _ = write!(verdict, "; {unread} file(s) not read");
    }
    verdict
}

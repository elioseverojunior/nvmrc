use std::path::PathBuf;

use super::fixtures::{
    HOME, hits_of, home_env, link, scan_with, scanned, users_dotfiles, users_env,
};
use super::{Note, NoteReason, roots, scan};
use crate::context::Context;
use crate::domain::conflict::Kind::{LazyLoader, LazyStub, Loader, NvmDirExport, OmzPlugin, Unset};
use crate::fakes::FakeFileSystem;
use crate::ports::FileSystem;
use crate::shell::init::Shell;

fn note(path: &str, line: usize, text: &str, reason: NoteReason) -> Note {
    Note {
        path: PathBuf::from(path),
        line: Some(line),
        text: text.to_owned(),
        reason,
    }
}

fn owned(pairs: &[(&str, usize)]) -> Vec<(String, usize)> {
    pairs
        .iter()
        .map(|(path, depth)| ((*path).to_owned(), *depth))
        .collect()
}

#[test]
fn the_users_dotfiles_are_scanned_roots_first_then_what_they_source() {
    let (fs, env) = (users_dotfiles(), users_env());
    let context = Context::new(&fs, &env);
    let report = scan(&context, &roots(&context, Some(Shell::Zsh)));
    let expected = [
        ("/Users/u/.zshenv", 0),
        ("/Users/u/.zprofile", 0),
        ("/Users/u/.zshrc", 0),
        ("/Users/u/.swiftly/env.sh", 1),
        ("/Users/u/.oh-my-zsh/oh-my-zsh.sh", 1),
        ("/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh", 1),
        ("/Users/u/dotfiles/zsh/scripts/environment.zsh", 1),
    ];
    assert_eq!(scanned(&report), owned(&expected));
    let zshrc = &report.files[2];
    assert_eq!(
        zshrc.canonical,
        PathBuf::from("/Users/u/dotfiles/zsh/.zshrc")
    );
}

#[test]
fn the_users_conflict_is_the_lazy_loader_at_depth_one() {
    let (fs, env) = (users_dotfiles(), users_env());
    let report = scan_with(
        &fs,
        &env,
        &["/Users/u/.zshenv", "/Users/u/.zprofile", "/Users/u/.zshrc"],
    );
    assert_eq!(hits_of(&report, "/Users/u/.zshenv"), [(3, NvmDirExport)]);
    assert_eq!(hits_of(&report, "/Users/u/.zshrc"), [(3, OmzPlugin)]);
    let lazy = "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh";
    let expected = [(1, LazyStub), (2, Unset), (3, LazyLoader), (6, LazyStub)];
    assert_eq!(hits_of(&report, lazy), expected);
    assert!(report.has_conflicts());
    assert_eq!(report.auto_migratable().count(), 0);
    let omz_line = "  source \"$ZSH/plugins/$plugin/$plugin.plugin.zsh\"";
    let stub_line = "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"";
    let unset = |name: &str| NoteReason::Unresolvable(format!("unset ${name}"));
    let omz = "/Users/u/.oh-my-zsh/oh-my-zsh.sh";
    assert_eq!(
        report.notes,
        [
            note(omz, 2, omz_line, unset("plugin")),
            note(lazy, 3, stub_line, unset("nvm_prefix")),
        ]
    );
}

fn chain() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", "source ~/a.sh\n")
        .with_file("/Users/u/a.sh", "source ~/b.sh\n")
        .with_file("/Users/u/b.sh", "source ~/c.sh\n")
        .with_file("/Users/u/c.sh", ". \"$HOME/.nvm/nvm.sh\"\n")
}

#[test]
fn depth_three_is_not_scanned_but_noted() {
    let report = scan_with(&chain(), &home_env(), &["/Users/u/.bashrc"]);
    let expected = [
        ("/Users/u/.bashrc", 0),
        ("/Users/u/a.sh", 1),
        ("/Users/u/b.sh", 2),
    ];
    assert_eq!(scanned(&report), owned(&expected));
    let too_deep = NoteReason::TooDeep(PathBuf::from("/Users/u/c.sh"));
    assert_eq!(
        report.notes,
        [note("/Users/u/b.sh", 1, "source ~/c.sh", too_deep)]
    );
    assert!(!report.has_conflicts());
}

#[test]
fn a_cycle_scans_each_file_once() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", "source ~/a.sh\nsource ~/.bashrc\n")
        .with_file("/Users/u/a.sh", "source ~/b.sh\n")
        .with_file("/Users/u/b.sh", "source ~/a.sh\n. ~/.bashrc\n");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [
        ("/Users/u/.bashrc", 0),
        ("/Users/u/a.sh", 1),
        ("/Users/u/b.sh", 2),
    ];
    assert_eq!(scanned(&report), owned(&expected));
    assert!(report.notes.is_empty());
}

#[test]
fn a_file_sourced_twice_or_through_a_link_is_kept_under_its_first_name() {
    let fs = FakeFileSystem::default()
        .with_file(
            "/Users/u/.bashrc",
            "source ~/a.sh\n. ~/alias.sh\nsource $HOME/a.sh\n",
        )
        .with_file("/Users/u/a.sh", "");
    link(&fs, "a.sh", "/Users/u/alias.sh");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [("/Users/u/.bashrc", 0), ("/Users/u/a.sh", 1)];
    assert_eq!(scanned(&report), owned(&expected));
}

#[test]
fn a_root_sourced_by_an_earlier_root_stays_a_root() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bash_profile", "source ~/.profile\n")
        .with_file("/Users/u/.profile", ". /opt/nvm/nvm.sh\n");
    let roots = ["/Users/u/.bash_profile", "/Users/u/.profile"];
    let report = scan_with(&fs, &home_env(), &roots);
    let expected = [("/Users/u/.bash_profile", 0), ("/Users/u/.profile", 0)];
    assert_eq!(scanned(&report), owned(&expected));
    assert_eq!(report.auto_migratable().count(), 1);
}

#[test]
fn a_command_substitution_is_noted_and_the_loader_still_found() {
    let line = "source $(brew --prefix nvm)/nvm.sh";
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", &format!("{line}\n"));
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    assert_eq!(hits_of(&report, "/Users/u/.bashrc"), [(1, Loader)]);
    let why = NoteReason::Unresolvable("command substitution $(brew --prefix nvm)".to_owned());
    assert_eq!(report.notes, [note("/Users/u/.bashrc", 1, line, why)]);
    assert!(report.has_conflicts());
    assert_eq!(report.auto_migratable().count(), 1);
}

#[test]
fn a_relative_path_is_resolved_from_the_sourcing_file_and_noted() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", "source lib/extra.sh\n")
        .with_file("/Users/u/lib/extra.sh", ". /opt/nvm/nvm.sh\n");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [("/Users/u/.bashrc", 0), ("/Users/u/lib/extra.sh", 1)];
    assert_eq!(scanned(&report), owned(&expected));
    let relative = NoteReason::Relative(PathBuf::from("/Users/u/lib/extra.sh"));
    let missing = NoteReason::Missing(PathBuf::from("/opt/nvm/nvm.sh"));
    let extra = ". /opt/nvm/nvm.sh";
    assert_eq!(
        report.notes,
        [
            note("/Users/u/.bashrc", 1, "source lib/extra.sh", relative),
            note("/Users/u/lib/extra.sh", 1, extra, missing),
        ]
    );
    assert_eq!(hits_of(&report, "/Users/u/lib/extra.sh"), [(1, Loader)]);
    assert!(report.has_conflicts());
    assert_eq!(report.auto_migratable().count(), 0, "not in a root");
}

#[test]
fn a_missing_file_is_noted() {
    let text = "[ -f \"$HOME/.missing.sh\" ] && . \"$HOME/.missing.sh\"";
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", &format!("{text}\n"));
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let missing = NoteReason::Missing(PathBuf::from(format!("{HOME}/.missing.sh")));
    assert_eq!(report.notes, [note("/Users/u/.bashrc", 1, text, missing)]);
    assert_eq!(report.files.len(), 1);
}

fn unreadable_note(report_notes: &[Note], path: &str) -> bool {
    report_notes.iter().any(|note| {
        note.path == std::path::Path::new(path)
            && note.line.is_none()
            && note.text.is_empty()
            && matches!(note.reason, NoteReason::Unreadable(_))
    })
}

#[test]
fn an_unreadable_root_or_sourced_file_is_noted_not_scanned() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", "source ~/bin.sh\n");
    let written = fs.write_bytes(std::path::Path::new("/Users/u/bin.sh"), b"\xff\xfe");
    assert!(written.is_ok());
    let written = fs.write_bytes(std::path::Path::new("/Users/u/.bashrc"), b"\xff");
    assert!(written.is_ok());
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc", "/Users/u/.profile"]);
    assert_eq!(scanned(&report), owned(&[("/Users/u/.profile", 0)]));
    assert_eq!(report.notes.len(), 2);
    assert!(unreadable_note(&report.notes, "/Users/u/.bashrc"));
    assert!(unreadable_note(&report.notes, "/Users/u/bin.sh"));
}

#[test]
fn a_clean_setup_has_no_hits_and_comments_are_not_followed() {
    let fs = FakeFileSystem::default()
        .with_file(
            "/Users/u/.bashrc",
            "export PATH=\"$HOME/bin:$PATH\"\nsource ~/.aliases\n# source ~/.secret.sh\n",
        )
        .with_file("/Users/u/.aliases", "alias ll='ls -l'\n")
        .with_file("/Users/u/.secret.sh", ". /opt/nvm/nvm.sh\n");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [("/Users/u/.bashrc", 0), ("/Users/u/.aliases", 1)];
    assert_eq!(scanned(&report), owned(&expected));
    assert!(report.files.iter().all(|file| file.hits.is_empty()));
    assert!(report.notes.is_empty());
    assert!(!report.has_conflicts());
}

#[test]
fn no_roots_give_an_empty_report() {
    let report = scan_with(&FakeFileSystem::default(), &home_env(), &[]);
    assert!(report.files.is_empty() && report.notes.is_empty());
}

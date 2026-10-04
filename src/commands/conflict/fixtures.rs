//! Startup files for the scanner's tests.

use std::path::{Path, PathBuf};

use super::{Report, scan};
use crate::context::Context;
use crate::domain::conflict::Kind;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

pub const HOME: &str = "/Users/u";

/// The user's zsh setup of digest section 3.4: stow links in `$HOME` to
/// `dotfiles/zsh`, scripts sourced through `$ZSH_CONFIG_SCRIPTS`.
pub fn users_dotfiles() -> FakeFileSystem {
    let fs = users_scripts(users_profiles());
    for name in [".zshenv", ".zprofile", ".zshrc"] {
        link(
            &fs,
            &format!("dotfiles/zsh/{name}"),
            &format!("{HOME}/{name}"),
        );
    }
    fs
}

/// The files of `dotfiles/zsh` that stow links into `$HOME`.
fn users_profiles() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file(
            "/Users/u/dotfiles/zsh/.zshenv",
            "export DOTFILES_HOME=\"${HOME}/dotfiles\"\n\
export ZSH_CONFIG_SCRIPTS=\"${DOTFILES_HOME}/zsh/scripts\"\n\
export NVM_DIR=\"${HOME}/.nvm\"\n",
        )
        .with_file(
            "/Users/u/dotfiles/zsh/.zprofile",
            &lines(&[
                "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
                "if [ -f \"$HOME/.swiftly/env.sh\" ]; then",
                "  . \"$HOME/.swiftly/env.sh\"",
                "fi",
            ]),
        )
        .with_file(
            "/Users/u/dotfiles/zsh/.zshrc",
            "plugins=(\n    git\n    nvm\n)\n\
source $ZSH/oh-my-zsh.sh\n\
source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh\n\
source ${ZSH_CONFIG_SCRIPTS}/environment.zsh\n",
        )
}

/// The files `.zprofile` and `.zshrc` source.
fn users_scripts(fs: FakeFileSystem) -> FakeFileSystem {
    fs.with_file("/Users/u/.swiftly/env.sh", "export SWIFTLY_HOME=x\n")
        .with_file(
            "/Users/u/.oh-my-zsh/oh-my-zsh.sh",
            "for plugin ($plugins); do\n  source \"$ZSH/plugins/$plugin/$plugin.plugin.zsh\"\ndone\n",
        )
        .with_file(
            "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh",
            &lines(&[
                "nvm() {",
                "  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
                "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
                "  nvm \"$@\"",
                "}",
                "npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
            ]),
        )
        .with_file(
            "/Users/u/dotfiles/zsh/scripts/environment.zsh",
            "export EDITOR=vim\n",
        )
}

/// The environment the user's `.zshenv` exports.
pub fn users_env() -> FakeEnv {
    FakeEnv::default()
        .with_var("HOME", HOME)
        .with_var("ZSH", "/Users/u/.oh-my-zsh")
        .with_var("ZSH_CONFIG_SCRIPTS", "/Users/u/dotfiles/zsh/scripts")
}

/// `text` as a file: every line newline-terminated (indentation kept).
fn lines(text: &[&str]) -> String {
    text.iter().map(|line| format!("{line}\n")).collect()
}

pub fn home_env() -> FakeEnv {
    FakeEnv::default().with_var("HOME", HOME)
}

pub fn link(fs: &FakeFileSystem, target: &str, link: &str) {
    let made = fs.symlink(Path::new(target), Path::new(link));
    assert!(made.is_ok(), "symlink {link}");
}

pub fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

pub fn scan_with(fs: &FakeFileSystem, env: &FakeEnv, roots: &[&str]) -> Report {
    scan(&Context::new(fs, env), &paths(roots))
}

/// `(path, depth)` of every scanned file.
pub fn scanned(report: &Report) -> Vec<(String, usize)> {
    report
        .files
        .iter()
        .map(|file| (file.path.display().to_string(), file.depth))
        .collect()
}

/// `(line, kind)` of the hits of the file named `path`.
pub fn hits_of(report: &Report, path: &str) -> Vec<(usize, Kind)> {
    let file = report
        .files
        .iter()
        .find(|file| file.path == Path::new(path));
    let file = file.unwrap_or_else(|| panic!("{path} was not scanned"));
    file.hits.iter().map(|hit| (hit.line, hit.kind)).collect()
}

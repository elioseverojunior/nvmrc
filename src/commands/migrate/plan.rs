//! Which startup files `migrate` edits, and how.

use std::path::PathBuf;

use crate::commands::conflict::Report;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::migration::{migrate_text, unified_diff};
use crate::ports::Env;
use crate::shell::init::{SHELLS, Shell};

/// The planned new content of one file, named as the scan named it (a
/// symbolic link stays a link).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub old: String,
    pub new: String,
}

impl Change {
    /// The unified diff from the current content to the new one.
    pub fn diff(&self) -> String {
        unified_diff(&self.path.display().to_string(), &self.old, &self.new)
    }
}

/// The edits of the root files (depth 0) of `report` that change: the ones
/// with top-level loaders or completions, or with an outdated init block.
/// The block is for `shell`, else the shell of a current block already in
/// the file, else [`shell_of`] the file. A root that cannot be read is
/// reported on `transcript` and makes the second value `true`.
pub fn planned(
    context: &Context<'_>,
    report: &Report,
    shell: Option<Shell>,
    transcript: &mut Transcript,
) -> (Vec<Change>, bool) {
    let mut changes = Vec::new();
    let mut failed = false;
    for file in report.files.iter().filter(|file| file.depth == 0) {
        match context.fs.read_to_string(&file.path) {
            Ok(old) => changes.extend(change_of(context, file.path.clone(), old, shell)),
            Err(error) => {
                transcript.err(format!("nvm migrate: {}: {error}", file.path.display()));
                failed = true;
            }
        }
    }
    (changes, failed)
}

/// The edit of the file `path` whose content is `old`, when it changes.
fn change_of(
    context: &Context<'_>,
    path: PathBuf,
    old: String,
    shell: Option<Shell>,
) -> Option<Change> {
    let shell = shell
        .or_else(|| block_shell(&old))
        .unwrap_or_else(|| shell_of(&path.display().to_string(), context.env));
    let new = migrate_text(&old, shell).text;
    (new != old).then_some(Change { path, old, new })
}

/// The shell of a current init line already in `text`, so that a block the
/// user chose stays as it is.
fn block_shell(text: &str) -> Option<Shell> {
    text.lines().find_map(|line| {
        SHELLS
            .into_iter()
            .find(|shell| line.trim() == shell.load_line())
    })
}

/// The shell whose init line goes in the startup file `path`, by its name:
/// zsh's files (`zsh` in the name, `.zprofile`, `.zlogin`, `.zlogout`) zsh,
/// `.bash*` bash, `.kshrc` ksh, `*.fish` fish; any other file (`.profile`,
/// an `$ENV` file) the login shell of `$SHELL` when it is a supported one,
/// else bash.
#[must_use]
pub fn shell_of(path: &str, env: &dyn Env) -> Shell {
    const ZSH_FILES: [&str; 3] = [".zprofile", ".zlogin", ".zlogout"];
    let name = path.rsplit('/').next().unwrap_or_default();
    if name.contains("zsh") || ZSH_FILES.contains(&name) {
        Shell::Zsh
    } else if name.starts_with(".bash") {
        Shell::Bash
    } else if name == ".kshrc" {
        Shell::Ksh
    } else if name.ends_with(".fish") {
        Shell::Fish
    } else {
        login_shell(env).unwrap_or(Shell::Bash)
    }
}

/// The supported shell `$SHELL` names (its basename).
fn login_shell(env: &dyn Env) -> Option<Shell> {
    let shell = env.var("SHELL")?;
    shell.rsplit('/').next()?.parse().ok()
}

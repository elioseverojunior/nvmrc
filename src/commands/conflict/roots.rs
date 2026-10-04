//! The startup files each shell reads (digest section 5).
//!
//! `sh` and `dash` share one list: as a profile reader they are the same
//! POSIX shell (`~/.profile` for a login shell, `$ENV` for an interactive
//! one); the separate names only matter to `nvm init`.

use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::domain::conflict::{Expansion, expand_path};
use crate::shell::init::{SHELLS, Shell};

const BASH_FILES: [&str; 4] = [".bashrc", ".bash_profile", ".bash_login", ".profile"];
const ZSH_FILES: [&str; 4] = [".zshenv", ".zprofile", ".zshrc", ".zlogin"];

/// The startup files of `shell` that exist (links followed), in the order the
/// shell reads them; `None` for those of every shell, each file once:
///
/// - bash: `~/.bashrc`, `~/.bash_profile`, `~/.bash_login`, `~/.profile`;
/// - zsh: `.zshenv`, `.zprofile`, `.zshrc`, `.zlogin` in `$ZDOTDIR` (else
///   `$HOME`), after `~/.zshenv` when `ZDOTDIR` is elsewhere (that file is
///   read first, and is where `ZDOTDIR` is usually set);
/// - sh and dash: `~/.profile` and `$ENV`;
/// - ksh: `~/.profile` and `$ENV`, which defaults to `~/.kshrc`;
/// - fish: `conf.d/*.fish` by name, then `config.fish`, in
///   `${XDG_CONFIG_HOME:-~/.config}/fish`.
///
/// `$ENV` is expanded like a sourced path; one that cannot be is skipped.
#[must_use]
pub fn roots(context: &Context<'_>, shell: Option<Shell>) -> Vec<PathBuf> {
    let shells = shell.map_or(SHELLS.to_vec(), |shell| vec![shell]);
    let mut found: Vec<PathBuf> = Vec::new();
    for candidate in shells
        .into_iter()
        .flat_map(|shell| candidates(context, shell))
    {
        if !found.contains(&candidate) && context.fs.is_file(&candidate) {
            found.push(candidate);
        }
    }
    found
}

/// Every file `shell` may read, present or not.
fn candidates(context: &Context<'_>, shell: Shell) -> Vec<PathBuf> {
    let home = directory_variable(context, "HOME");
    match shell {
        Shell::Bash => in_directory(home.as_deref(), &BASH_FILES),
        Shell::Zsh => zsh_files(context, home.as_deref()),
        Shell::Sh | Shell::Dash => profile_and_env(context, home.as_deref(), None),
        Shell::Ksh => profile_and_env(context, home.as_deref(), Some(".kshrc")),
        Shell::Fish => fish_files(context, home.as_deref()),
    }
}

/// A variable naming a directory; `None` when unset or empty.
fn directory_variable(context: &Context<'_>, name: &str) -> Option<PathBuf> {
    let value = context.env.var_os(name).filter(|value| !value.is_empty());
    value.map(PathBuf::from)
}

fn in_directory(directory: Option<&Path>, names: &[&str]) -> Vec<PathBuf> {
    directory.map_or_else(Vec::new, |directory| {
        names.iter().map(|name| directory.join(name)).collect()
    })
}

fn zsh_files(context: &Context<'_>, home: Option<&Path>) -> Vec<PathBuf> {
    let zdotdir = directory_variable(context, "ZDOTDIR");
    let mut files = Vec::new();
    if let (Some(zdotdir), Some(home)) = (zdotdir.as_deref(), home) {
        if zdotdir != home {
            files.push(home.join(".zshenv"));
        }
    }
    files.extend(in_directory(zdotdir.as_deref().or(home), &ZSH_FILES));
    files
}

/// `~/.profile`, then `$ENV` (or `~/<default_env>` when `ENV` is unset).
fn profile_and_env(
    context: &Context<'_>,
    home: Option<&Path>,
    default_env: Option<&str>,
) -> Vec<PathBuf> {
    let mut files = in_directory(home, &[".profile"]);
    let env_file = match context.env.var("ENV").filter(|value| !value.is_empty()) {
        Some(value) => absolute_expansion(context, &value),
        None => default_env.and_then(|name| home.map(|home| home.join(name))),
    };
    files.extend(env_file);
    files
}

/// `value` expanded as a sourced path, when that gives an absolute path.
fn absolute_expansion(context: &Context<'_>, value: &str) -> Option<PathBuf> {
    let lookup = |name: &str| context.env.var(name);
    match expand_path(value, &lookup) {
        Expansion::Resolved(path) => Some(PathBuf::from(path)).filter(|path| path.is_absolute()),
        Expansion::Unresolvable(_) => None,
    }
}

fn fish_files(context: &Context<'_>, home: Option<&Path>) -> Vec<PathBuf> {
    let config_home = directory_variable(context, "XDG_CONFIG_HOME")
        .or_else(|| home.map(|home| home.join(".config")));
    let Some(fish) = config_home.map(|config| config.join("fish")) else {
        return Vec::new();
    };
    let conf_d = fish.join("conf.d");
    let mut names: Vec<String> = context
        .fs
        .read_dir(&conf_d)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.is_dir && entry.name.ends_with(".fish"))
        .map(|entry| entry.name)
        .collect();
    names.sort();
    let mut files: Vec<PathBuf> = names.iter().map(|name| conf_d.join(name)).collect();
    files.push(fish.join("config.fish"));
    files
}

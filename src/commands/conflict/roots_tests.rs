use super::fixtures::{home_env, link, paths};
use super::roots;
use crate::context::Context;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::shell::init::Shell;

fn with_files(names: &[&str]) -> FakeFileSystem {
    names
        .iter()
        .fold(FakeFileSystem::default(), |fs, name| fs.with_file(name, ""))
}

fn roots_of(fs: &FakeFileSystem, env: &FakeEnv, shell: Option<Shell>) -> Vec<std::path::PathBuf> {
    roots(&Context::new(fs, env), shell)
}

const BASH: [&str; 4] = [
    "/Users/u/.bashrc",
    "/Users/u/.bash_profile",
    "/Users/u/.bash_login",
    "/Users/u/.profile",
];

#[test]
fn bash_reads_bashrc_then_the_login_files_in_order() {
    let fs = with_files(&BASH);
    assert_eq!(roots_of(&fs, &home_env(), Some(Shell::Bash)), paths(&BASH));
}

#[test]
fn only_the_files_that_exist_are_roots() {
    let fs = with_files(&["/Users/u/.bash_profile", "/Users/u/.zshrc"]);
    let found = roots_of(&fs, &home_env(), Some(Shell::Bash));
    assert_eq!(found, paths(&["/Users/u/.bash_profile"]));
}

const ZSH: [&str; 4] = [
    "/Users/u/.zshenv",
    "/Users/u/.zprofile",
    "/Users/u/.zshrc",
    "/Users/u/.zlogin",
];

#[test]
fn zsh_reads_its_four_files_from_home_without_zdotdir() {
    let fs = with_files(&ZSH);
    assert_eq!(roots_of(&fs, &home_env(), Some(Shell::Zsh)), paths(&ZSH));
    let empty = home_env().with_var("ZDOTDIR", "");
    assert_eq!(roots_of(&fs, &empty, Some(Shell::Zsh)), paths(&ZSH));
}

#[test]
fn zsh_reads_the_zdotdir_files_after_the_home_zshenv_that_sets_it() {
    let fs = with_files(&ZSH)
        .with_file("/z/.zshenv", "")
        .with_file("/z/.zshrc", "");
    let env = home_env().with_var("ZDOTDIR", "/z");
    let found = roots_of(&fs, &env, Some(Shell::Zsh));
    assert_eq!(
        found,
        paths(&["/Users/u/.zshenv", "/z/.zshenv", "/z/.zshrc"])
    );
}

#[test]
fn sh_and_dash_read_profile_and_the_file_named_by_env() {
    let fs = with_files(&["/Users/u/.profile", "/Users/u/.shinit"]);
    let env = home_env().with_var("ENV", "$HOME/.shinit");
    for shell in [Shell::Sh, Shell::Dash] {
        let found = roots_of(&fs, &env, Some(shell));
        assert_eq!(found, paths(&["/Users/u/.profile", "/Users/u/.shinit"]));
        let without_env = roots_of(&fs, &home_env(), Some(shell));
        assert_eq!(without_env, paths(&["/Users/u/.profile"]));
    }
}

#[test]
fn ksh_reads_profile_and_env_which_defaults_to_kshrc() {
    let fs = with_files(&["/Users/u/.profile", "/Users/u/.kshrc", "/etc/kshenv"]);
    let found = roots_of(&fs, &home_env(), Some(Shell::Ksh));
    assert_eq!(found, paths(&["/Users/u/.profile", "/Users/u/.kshrc"]));
    let env = home_env().with_var("ENV", "/etc/kshenv");
    let found = roots_of(&fs, &env, Some(Shell::Ksh));
    assert_eq!(found, paths(&["/Users/u/.profile", "/etc/kshenv"]));
}

#[test]
fn an_env_that_cannot_be_expanded_is_ignored() {
    let fs = with_files(&["/Users/u/.profile"]);
    let env = home_env().with_var("ENV", "$(echo /x)");
    let found = roots_of(&fs, &env, Some(Shell::Sh));
    assert_eq!(found, paths(&["/Users/u/.profile"]));
}

#[test]
fn fish_reads_conf_d_sorted_then_config_fish() {
    let fs = with_files(&[
        "/Users/u/.config/fish/config.fish",
        "/Users/u/.config/fish/conf.d/b.fish",
        "/Users/u/.config/fish/conf.d/a.fish",
        "/Users/u/.config/fish/conf.d/notes.txt",
        "/Users/u/.config/fish/conf.d/dir.fish/inner.fish",
    ]);
    let found = roots_of(&fs, &home_env(), Some(Shell::Fish));
    let expected = [
        "/Users/u/.config/fish/conf.d/a.fish",
        "/Users/u/.config/fish/conf.d/b.fish",
        "/Users/u/.config/fish/config.fish",
    ];
    assert_eq!(found, paths(&expected));
}

#[test]
fn fish_follows_xdg_config_home() {
    let fs = with_files(&["/x/fish/config.fish", "/x/fish/conf.d/n.fish"]);
    let env = home_env().with_var("XDG_CONFIG_HOME", "/x");
    let found = roots_of(&fs, &env, Some(Shell::Fish));
    assert_eq!(
        found,
        paths(&["/x/fish/conf.d/n.fish", "/x/fish/config.fish"])
    );
}

#[test]
fn every_shell_without_a_name_once_each_in_shell_order() {
    let fs = with_files(&[
        "/Users/u/.profile",
        "/Users/u/.zshrc",
        "/Users/u/.bashrc",
        "/Users/u/.kshrc",
        "/Users/u/.config/fish/config.fish",
    ]);
    let found = roots_of(&fs, &home_env(), None);
    let expected = [
        "/Users/u/.bashrc",
        "/Users/u/.profile",
        "/Users/u/.zshrc",
        "/Users/u/.kshrc",
        "/Users/u/.config/fish/config.fish",
    ];
    assert_eq!(found, paths(&expected));
}

#[test]
fn a_linked_root_counts_and_so_does_a_dangling_link_for_the_scan_to_report() {
    let fs = with_files(&["/Users/u/dotfiles/zsh/.zshrc"]);
    link(&fs, "dotfiles/zsh/.zshrc", "/Users/u/.zshrc");
    link(&fs, "dotfiles/zsh/.zprofile", "/Users/u/.zprofile");
    let found = roots_of(&fs, &home_env(), Some(Shell::Zsh));
    assert_eq!(found, paths(&["/Users/u/.zprofile", "/Users/u/.zshrc"]));
}

#[test]
fn without_home_only_absolute_variables_name_roots() {
    let fs = with_files(&["/etc/shinit", "/z/.zshrc", "/.profile"]);
    let env = FakeEnv::default().with_var("ENV", "/etc/shinit");
    assert_eq!(
        roots_of(&fs, &env, Some(Shell::Sh)),
        paths(&["/etc/shinit"])
    );
    let env = FakeEnv::default().with_var("ZDOTDIR", "/z");
    assert_eq!(roots_of(&fs, &env, Some(Shell::Zsh)), paths(&["/z/.zshrc"]));
}

#[test]
fn a_link_that_leads_nowhere_is_still_a_root() {
    let fs = FakeFileSystem::default();
    link(&fs, "/Users/u/.bashrc", "/Users/u/.bashrc");
    let found = roots_of(&fs, &home_env(), Some(Shell::Bash));
    assert_eq!(found, paths(&["/Users/u/.bashrc"]));
}

use super::expand::{Expansion, expand_path};

fn environment(name: &str) -> Option<String> {
    let value = match name {
        "HOME" => "/home/me",
        "ZSH" => "/home/me/.oh-my-zsh",
        "ZSH_CONFIG_SCRIPTS" => "/home/me/dotfiles/zsh/scripts",
        "NVM_DIR" => "/home/me/.nvm",
        "EMPTY" => "",
        _ => return None,
    };
    Some(value.to_string())
}

fn with_zdotdir(name: &str) -> Option<String> {
    match name {
        "ZDOTDIR" => Some("/home/me/.config/zsh".to_string()),
        _ => environment(name),
    }
}

fn without_home(name: &str) -> Option<String> {
    (name != "HOME").then(|| environment(name)).flatten()
}

fn resolved(path: &str) -> Expansion {
    Expansion::Resolved(path.to_string())
}

fn unresolvable(reason: &str) -> Expansion {
    Expansion::Unresolvable(reason.to_string())
}

#[test]
fn resolves_home_tilde_zdotdir_and_environment_variables() {
    let rows = [
        ("~", "/home/me"),
        ("~/.nvm/nvm.sh", "/home/me/.nvm/nvm.sh"),
        ("$HOME/.bun/_bun", "/home/me/.bun/_bun"),
        ("${HOME}/.bun/_bun", "/home/me/.bun/_bun"),
        ("${ZDOTDIR:-$HOME}/.zshenv", "/home/me/.zshenv"),
        ("${ZDOTDIR:-${HOME}}/.zshenv", "/home/me/.zshenv"),
        ("$ZSH/oh-my-zsh.sh", "/home/me/.oh-my-zsh/oh-my-zsh.sh"),
        (
            "${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
            "/home/me/dotfiles/zsh/scripts/lazy-functions.zsh",
        ),
        ("$NVM_DIR/nvm.sh", "/home/me/.nvm/nvm.sh"),
        ("/etc/profile", "/etc/profile"),
        ("relative/file.sh", "relative/file.sh"),
        ("$EMPTY/x", "/x"),
        ("a$HOME", "a/home/me"),
    ];
    for (token, expected) in rows {
        assert_eq!(
            expand_path(token, &environment),
            resolved(expected),
            "{token:?}"
        );
    }
}

#[test]
fn zdotdir_wins_over_home_when_set() {
    assert_eq!(
        expand_path("${ZDOTDIR:-$HOME}/.zshrc", &with_zdotdir),
        resolved("/home/me/.config/zsh/.zshrc")
    );
    assert_eq!(
        expand_path("${ZDOTDIR:-${HOME}}/.zshrc", &with_zdotdir),
        resolved("/home/me/.config/zsh/.zshrc")
    );
}

#[test]
fn anything_left_unexpanded_is_unresolvable_with_a_reason() {
    let rows = [
        (
            "$(brew --prefix nvm)/nvm.sh",
            "command substitution $(brew --prefix nvm)",
        ),
        (
            "`brew --prefix nvm`/nvm.sh",
            "command substitution `brew --prefix nvm`/nvm.sh",
        ),
        ("${0:A:h}/x.zsh", "unsupported ${0:A:h}"),
        ("$f", "unset $f"),
        ("$FOO/x", "unset $FOO"),
        ("${FOO}/x", "unset $FOO"),
        (
            "${NVM_DIR:-$HOME/.nvm}/nvm.sh",
            "unsupported ${NVM_DIR:-$HOME/.nvm}",
        ),
        ("${#HOME}", "unsupported ${#HOME}"),
        ("${list[@]}", "unsupported ${list[@]}"),
        ("$1/x", "unsupported $1/x"),
        ("$@", "unsupported $@"),
        ("x$", "unsupported $"),
        ("${HOME", "unsupported ${HOME"),
        ("~user/x", "unsupported ~user/x"),
        ("$HOME/conf.d/*.zsh", "glob /home/me/conf.d/*.zsh"),
        ("$HOME/file?.sh", "glob /home/me/file?.sh"),
        ("$HOME/[ab].sh", "glob /home/me/[ab].sh"),
    ];
    for (token, reason) in rows {
        assert_eq!(
            expand_path(token, &environment),
            unresolvable(reason),
            "{token:?}"
        );
    }
}

#[test]
fn home_must_be_set_to_expand_it() {
    assert_eq!(
        expand_path("~/x", &without_home),
        unresolvable("unset $HOME")
    );
    assert_eq!(
        expand_path("${ZDOTDIR:-$HOME}/.zshrc", &without_home),
        unresolvable("unset $HOME")
    );
}

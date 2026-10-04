use super::file_shell;
use crate::shell::init::Shell;

#[test]
fn shell_specific_names_name_their_shell() {
    let cases = [
        ("/home/u/.zshrc", Shell::Zsh),
        ("/home/u/.zprofile", Shell::Zsh),
        ("/home/u/.zlogin", Shell::Zsh),
        ("/home/u/.zshenv", Shell::Zsh),
        ("/home/u/.bash_login", Shell::Bash),
        ("/home/u/.bash_profile", Shell::Bash),
        ("/home/u/.config/fish/conf.d/nvm.fish", Shell::Fish),
        ("/home/u/.kshrc", Shell::Ksh),
        ("/home/u/.mkshrc", Shell::Ksh),
    ];
    for (path, shell) in cases {
        assert_eq!(file_shell(path), Some(shell), "{path}");
    }
}

#[test]
fn the_profile_and_other_names_are_posix_files() {
    for path in [
        "/home/u/.profile",
        "/home/u/zsh/.profile",
        "/home/u/.shinit",
        "/h/nvm.sh.d/x.sh",
        "",
    ] {
        assert_eq!(file_shell(path), None, "{path}");
    }
}

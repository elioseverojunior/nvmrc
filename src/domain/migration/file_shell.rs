//! The shell a startup file is written for, from its name alone: the one
//! source of the init line `migrate` writes there, the program that checks
//! the result, and the shell of the patch `doctor` suggests.

use crate::shell::init::Shell;

/// The files of zsh whose names do not say `zsh`.
const ZSH_FILES: [&str; 3] = [".zprofile", ".zlogin", ".zlogout"];

/// The shell the startup file `path` is written for, by its file name: `zsh`
/// in the name (or `.zprofile`, `.zlogin`, `.zlogout`) zsh, `bash` bash,
/// `.fish` fish, `ksh` (`.kshrc`, `.mkshrc`) ksh. `None` for any other name
/// (`.profile`, an `$ENV` file, a sourced `*.sh`): a POSIX file, which
/// `/bin/sh` may read too (display managers, `su -`, ssh), so it only ever
/// gets POSIX `sh` code, whatever `$SHELL` is.
#[must_use]
pub fn file_shell(path: &str) -> Option<Shell> {
    let name = path.rsplit('/').next().unwrap_or_default();
    if name.contains("zsh") || ZSH_FILES.contains(&name) {
        Some(Shell::Zsh)
    } else if name.contains("bash") {
        Some(Shell::Bash)
    } else if name.ends_with(".fish") {
        Some(Shell::Fish)
    } else if name.contains("ksh") {
        Some(Shell::Ksh)
    } else {
        None
    }
}

/// The program (and its arguments) that syntax-checks a candidate for the
/// startup file `path`: the [`file_shell`] of the file with `-n` (fish with
/// `--no-execute`), `sh -n` for a POSIX file. `None` when `path` names no
/// file. The caller appends the path of the temp file to the arguments.
#[must_use]
pub fn syntax_check(path: &str) -> Option<(&'static str, Vec<&'static str>)> {
    if path.rsplit('/').next().unwrap_or_default().is_empty() {
        return None;
    }
    let checker = match file_shell(path) {
        Some(Shell::Zsh) => ("zsh", "-n"),
        Some(Shell::Bash) => ("bash", "-n"),
        Some(Shell::Fish) => ("fish", "--no-execute"),
        Some(Shell::Ksh) => ("ksh", "-n"),
        Some(Shell::Sh | Shell::Dash) | None => ("sh", "-n"),
    };
    Some((checker.0, vec![checker.1]))
}

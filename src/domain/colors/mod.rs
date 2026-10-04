//! The color palette of nvm.sh: letter table, the five `NVM_COLORS` roles and
//! the colors derived from them. Pure: no terminal detection happens here.

#[cfg(test)]
mod tests;

/// `\e[0m`, the reset that closes every colored span.
pub const RESET: &str = "\x1b[0m";
/// `\e[0;90m`, the fixed arrow color of the alias rows.
pub const ARROW: &str = "\x1b[0;90m";
/// `\e[3m`, italics on (ls-remote annotations).
pub const ITALIC_ON: &str = "\x1b[3m";
/// `\e[23m`, italics off.
pub const ITALIC_OFF: &str = "\x1b[23m";

/// The letters accepted by `nvm_set_colors`, in the order of its regex.
const SETTABLE_LETTERS: &str = "rRgGbBcCyYmMkKeW";
/// The setting used when `NVM_COLORS` is unset or empty.
const DEFAULT_SETTING: &str = "bygre";

/// Mirrors `nvm_print_color_code`: the SGR code of a letter, without the
/// escape prefix but with the trailing `m` (`r` gives `0;31m`).
///
/// `0` (the undocumented "no color") and every unknown letter give `None`.
#[must_use]
pub fn sgr(letter: char) -> Option<&'static str> {
    match letter {
        'r' => Some("0;31m"),
        'R' => Some("1;31m"),
        'g' => Some("0;32m"),
        'G' => Some("1;32m"),
        'b' => Some("0;34m"),
        'B' => Some("1;34m"),
        'c' => Some("0;36m"),
        'C' => Some("1;36m"),
        'm' => Some("0;35m"),
        'M' => Some("1;35m"),
        'y' => Some("0;33m"),
        'Y' => Some("1;33m"),
        'k' => Some("0;30m"),
        'K' => Some("1;30m"),
        'e' => Some("0;37m"),
        'W' => Some("1;37m"),
        _ => None,
    }
}

/// The five roles of `NVM_COLORS`, in the order of its positions 1 to 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Position 1: installed versions.
    Installed,
    /// Position 2: the system node (and, derived, the LTS aliases).
    System,
    /// Position 3: the current version.
    Current,
    /// Position 4: not installed (`N/A`, infinite loops).
    NotInstalled,
    /// Position 5: the `(default)` marker and old LTS annotations.
    Default,
}

impl Role {
    const fn index(self) -> usize {
        match self {
            Self::Installed => 0,
            Self::System => 1,
            Self::Current => 2,
            Self::NotInstalled => 3,
            Self::Default => 4,
        }
    }
}

/// The resolved colors of the five roles. A role that is missing or invalid
/// in the setting has no color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    codes: [Option<&'static str>; 5],
}

impl Palette {
    /// Mirrors `nvm_get_colors` over `${NVM_COLORS:-bygre}`: unset or empty
    /// means `bygre`, extra characters are ignored.
    ///
    /// Quirk fixed on purpose: nvm.sh warns on every lookup and splices
    /// malformed escapes; here an invalid or missing role is just uncolored
    /// and reported once in the second value, for the caller's single
    /// warning: `Some(letter)` for an invalid letter, `None` for a missing
    /// position (nvm.sh prints `Invalid color code: ` with nothing after it).
    /// The letter `0` means "no color" and is not reported. Positions are
    /// counted in characters.
    #[must_use]
    pub fn from_setting(setting: Option<&str>) -> (Self, Vec<Option<char>>) {
        let text = setting
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_SETTING);
        let mut letters = text.chars();
        let mut codes = [None; 5];
        let mut offending = Vec::new();
        for code in &mut codes {
            let letter = letters.next();
            *code = letter.and_then(sgr);
            if code.is_none() && letter != Some('0') {
                offending.push(letter);
            }
        }
        (Self { codes }, offending)
    }

    /// The code of a role (`nvm_get_colors` 1 to 5), `None` when it has none.
    #[must_use]
    pub fn code(&self, role: Role) -> Option<&str> {
        self.codes[role.index()]
    }

    /// The LTS color (`nvm_get_colors 6`): the system color piped through
    /// `tr '0;' '1;'`, so EVERY `0` becomes `1` (quirk kept: `k` gives
    /// `1;31m`).
    #[must_use]
    pub fn lts(&self) -> Option<String> {
        self.code(Role::System).map(|code| code.replace('0', "1"))
    }

    /// The ls-remote "Latest LTS" color: the current color with the FIRST
    /// `0;` turned into `1;` (awk `sub(/0;/,"1;")`).
    #[must_use]
    pub fn latest_lts(&self) -> Option<String> {
        self.code(Role::Current)
            .map(|code| code.replacen("0;", "1;", 1))
    }

    /// The ls-remote alias/latest annotation color: the current color with a
    /// LEADING `1;` turned into `0;`.
    #[must_use]
    pub fn annotation(&self) -> Option<String> {
        self.code(Role::Current)
            .map(|code| match code.strip_prefix("1;") {
                Some(rest) => format!("0;{rest}"),
                None => code.to_owned(),
            })
    }

    /// The ls-remote old `(LTS: x)` color: the default role.
    #[must_use]
    pub fn old_lts(&self) -> Option<&str> {
        self.code(Role::Default)
    }
}

/// Mirrors `nvm_wrap_with_color_code` once colors are known to be on:
/// `\e[<code><text>\e[0m`, or the plain text when the code is `None`.
#[must_use]
pub fn wrap(code: Option<&str>, text: &str) -> String {
    match code {
        Some(code) => format!("\x1b[{code}{text}{RESET}"),
        None => text.to_owned(),
    }
}

/// Mirrors the check of `nvm_set_colors`: exactly five characters, each one
/// of `rRgGbBcCyYmMkKeW` (`0` is not accepted here).
#[must_use]
pub fn is_valid_setting(setting: &str) -> bool {
    setting.chars().count() == 5
        && setting
            .chars()
            .all(|letter| SETTABLE_LETTERS.contains(letter))
}

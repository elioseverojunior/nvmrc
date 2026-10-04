//! Profile text made safe to print: a startup file can hold raw escape
//! sequences (a `PS1` with a literal ESC, a BEL), which would drive the
//! terminal if `doctor` or a diff echoed them.

/// `text` with every control character but tab and newline written in caret
/// notation, as `cat -v` does (ESC is `^[`, BEL `^G`, DEL `^?`). A carriage
/// return right before a newline (a CRLF line ending) is kept.
#[must_use]
pub fn printable(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        let line_ending = character == '\r' && characters.peek() == Some(&'\n');
        if is_kept(character) || line_ending {
            shown.push(character);
        } else {
            shown.push('^');
            shown.push(caret(character));
        }
    }
    shown
}

fn is_kept(character: char) -> bool {
    !character.is_ascii_control() || matches!(character, '\t' | '\n')
}

/// The letter of `^X` for the ASCII control `character`.
fn caret(character: char) -> char {
    char::from((character as u8) ^ 0x40)
}

#[cfg(test)]
mod tests {
    use super::printable;

    #[test]
    fn controls_but_tab_newline_and_crlf_become_caret_notation() {
        assert_eq!(
            printable("PS1=\"\x1b[31m\\u\x1b[0m\x07\"\tx\r\ny\rz\x7f\n"),
            "PS1=\"^[[31m\\u^[[0m^G\"\tx\r\ny^Mz^?\n"
        );
    }

    #[test]
    fn plain_text_and_other_unicode_are_unchanged() {
        assert_eq!(printable("café ✓ \u{85}"), "café ✓ \u{85}");
        assert_eq!(printable(""), "");
    }
}

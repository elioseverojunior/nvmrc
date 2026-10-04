//! Lines `migrate` must leave alone, and where the block goes around them.

use super::{BlockChange, migrate_text};
use crate::shell::init::Shell;

const LOADER: &str = "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"";
const COMPLETION: &str = "[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"";
const BASH_BLOCK: &str =
    "# >>> nvmrc init >>>\neval \"$(nvmrc init bash)\"\n# <<< nvmrc init <<<\n";

fn migrated(line: &str) -> String {
    format!("# [nvmrc-migrated] {line}")
}

#[test]
fn a_loader_sharing_its_line_with_other_commands_is_left_alone() {
    let text = format!(
        "export NVM_DIR=\"$HOME/.config/nvm\"; {LOADER}\n\
export PATH=\"$HOME/bin:$PATH\"; source ~/.nvm/nvm.sh\n"
    );
    let result = migrate_text(&text, Shell::Bash);
    assert_eq!(result.text, text);
    assert_eq!(result.block, BlockChange::Unchanged);
}

#[test]
fn a_continued_loader_is_left_alone_and_the_block_goes_before_a_top_level_line() {
    let text = format!("[ -s \"$NVM_DIR/nvm.sh\" ] && \\\n  . \"$NVM_DIR/nvm.sh\"\n{COMPLETION}\n");
    let result = migrate_text(&text, Shell::Bash);
    assert_eq!(
        result.text,
        format!(
            "[ -s \"$NVM_DIR/nvm.sh\" ] && \\\n  . \"$NVM_DIR/nvm.sh\"\n{BASH_BLOCK}{}\n",
            migrated(COMPLETION)
        )
    );
    assert_eq!(result.block, BlockChange::Inserted { before_line: 3 });
}

#[test]
fn a_loader_in_an_if_body_at_the_ifs_indentation_is_left_alone() {
    let text = format!("if [ -s \"$NVM_DIR/nvm.sh\" ]; then\n{LOADER}\nfi\n");
    let result = migrate_text(&text, Shell::Bash);
    assert_eq!(result.text, text);
    assert_eq!(result.block, BlockChange::Unchanged);
}

#[test]
fn an_unterminated_begin_marker_is_no_block_and_hides_no_loader() {
    let text = format!("# >>> nvmrc init >>>\n{LOADER}\n");
    let result = migrate_text(&text, Shell::Bash);
    assert_eq!(
        result.text,
        format!("# >>> nvmrc init >>>\n{BASH_BLOCK}{}\n", migrated(LOADER))
    );
    assert_eq!(result.block, BlockChange::Inserted { before_line: 2 });
    let again = migrate_text(&result.text, Shell::Bash);
    assert_eq!(again.text, result.text);
    assert_eq!(again.block, BlockChange::Unchanged);
}

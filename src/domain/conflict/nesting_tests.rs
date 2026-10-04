use super::lexer::{strip_comment, tokens};
use super::nesting::BlockTracker;

/// Whether each line of `text` sits inside a block or opens or closes one
/// (commenting it out could break the file), as the scanner asks.
fn nested_flags(text: &str) -> Vec<bool> {
    let mut tracker = BlockTracker::default();
    text.lines()
        .map(|line| tracker.observe(&tokens(strip_comment(line))))
        .collect()
}

#[test]
fn install_sh_one_liners_are_top_level() {
    let text = "\nexport NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion\n";
    assert_eq!(nested_flags(text), [false, false, false, false]);
}

#[test]
fn indentation_alone_does_not_nest() {
    let text =
        "  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"\n";
    assert_eq!(nested_flags(text), [false]);
}

#[test]
fn a_function_and_its_body_are_nested_and_the_line_after_it_is_not() {
    let text = "nvm_load() {\n  . \"$NVM_DIR/nvm.sh\"\n}\n. \"$NVM_DIR/nvm.sh\"\n";
    assert_eq!(nested_flags(text), [true, true, true, false]);
}

#[test]
fn an_if_with_a_body_is_nested_through_else_and_elif() {
    let text = "if [ -s \"$NVM_DIR/nvm.sh\" ]; then\n  . a\nelif true; then\n  . b\nelse\n  . c\nfi\n. d\n";
    assert_eq!(
        nested_flags(text),
        [true, true, true, true, true, true, true, false]
    );
}

#[test]
fn a_one_line_if_or_function_is_top_level() {
    let text = "if [ -s x ]; then . \"$NVM_DIR/nvm.sh\"; fi\nnpm() { nvm; command npm \"$@\" }\n";
    assert_eq!(nested_flags(text), [false, false]);
}

#[test]
fn a_line_that_opens_or_closes_a_block_is_nested() {
    let text = "if true; then . \"$NVM_DIR/nvm.sh\"\nfi\n";
    assert_eq!(nested_flags(text), [true, true]);
}

#[test]
fn loops_and_brace_groups_nest() {
    let text =
        "for cmd in a b; do\n  eval \"x\"\ndone\n{\n  . y\n}\nwhile false\ndo\n  . z\ndone\n";
    assert_eq!(
        nested_flags(text),
        [true, true, true, true, true, true, true, true, true, true]
    );
}

#[test]
fn a_body_at_the_openers_indentation_is_nested() {
    let text = "if true; then\n. \"$NVM_DIR/nvm.sh\"\nfi\n. x\n";
    assert_eq!(nested_flags(text), [true, true, true, false]);
}

#[test]
fn an_if_split_before_then_and_a_case_nest() {
    let text = "if [ -s x ]\nthen\n. x\nfi\ncase $- in\n*i*) . y ;;\nesac\n. z\n";
    assert_eq!(
        nested_flags(text),
        [true, true, true, true, true, true, true, false]
    );
}

#[test]
fn reserved_words_count_only_where_a_command_starts() {
    let text = "echo for if while case\n. x\nfor name in if fi done; do :; done\n. y\n";
    assert_eq!(nested_flags(text), [false, false, false, false]);
}

#[test]
fn nested_blocks_close_one_at_a_time() {
    let text = "f() {\n  if x; then\n    . a\n  fi\n  . b\n}\n. c\n";
    assert_eq!(
        nested_flags(text),
        [true, true, true, true, true, true, false]
    );
}

#[test]
fn braces_in_quotes_comments_and_expansions_are_not_blocks() {
    let text = "echo \"{\" '{' ${HOME} # {\n  . a\necho \\{\n  . b\n";
    assert_eq!(nested_flags(text), [false, false, false, false]);
}

#[test]
fn a_stray_closer_is_ignored() {
    let text = "}\n  . a\n";
    assert_eq!(nested_flags(text), [true, false]);
}

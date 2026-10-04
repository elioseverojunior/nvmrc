use super::standalone::is_standalone;

#[test]
fn the_loader_alone_or_behind_a_test_guard_is_standalone() {
    for line in [
        ". ~/.nvm/nvm.sh",
        "source \"$NVM_DIR/nvm.sh\" --no-use",
        "\\. \"$NVM_DIR/nvm.sh\"  ",
        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
        "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
        "test -f ~/.nvm/nvm.sh && . ~/.nvm/nvm.sh >/dev/null 2>&1",
        "if [ -s x/nvm.sh ]; then . x/nvm.sh; fi",
        ". $(brew --prefix nvm)/nvm.sh",
        "[ -s \"$(brew --prefix)/opt/nvm/nvm.sh\" ] && . \"$(brew --prefix)/opt/nvm/nvm.sh\"",
        "[ -s nvm.sh ] && . ~/.nvm/nvm.sh && . ~/.nvm/bash_completion",
    ] {
        assert!(is_standalone(line), "{line}");
    }
}

#[test]
fn a_loader_sharing_its_line_with_other_commands_is_not_standalone() {
    for line in [
        "export NVM_DIR=\"$HOME/.config/nvm\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"",
        "export PATH=\"$HOME/bin:$PATH\"; source ~/.nvm/nvm.sh",
        "source ~/.nvm/nvm.sh; nvm use 18",
        "source ~/.nvm/nvm.sh && nvm use default",
        "NVM_DIR=/opt/nvm . /opt/nvm/nvm.sh",
        "alias nvm='unalias nvm && . \"$NVM_DIR\"/nvm.sh && nvm'",
        ". ~/.nvm/nvm.sh || echo 'no nvm'",
        ". ~/.bash_aliases; . ~/.nvm/nvm.sh",
        "eval \"source $NVM_DIR/nvm.sh\"",
    ] {
        assert!(!is_standalone(line), "{line}");
    }
}

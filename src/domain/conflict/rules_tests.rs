//! The per-line rules of digest section 7, line by line, without the scanner's
//! state (plugins blocks, nesting, the file-level lazy rule).

use super::kind::Kind::{
    self, Bass, Completion, HelperCall, LazyStub, Loader, NvmDirExport, OmzPlugin, Unset, ZshNvm,
};
use super::lexer::strip_comment;
use super::rules::line_kinds;

fn assert_rows(rows: &[(&str, &[Kind])]) {
    for (line, expected) in rows {
        assert_eq!(line_kinds(strip_comment(line)), *expected, "{line:?}");
    }
}

/// `exp/corpus.sh`, line by line (digest section 7: 13 true positives, the
/// bass line now its own kind, and the rejects of lines 19 to 22).
const CORPUS_ROWS: &[(&str, &[Kind])] = &[
    ("export NVM_DIR=\"$HOME/.nvm\"", &[NvmDirExport]),
    (
        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm",
        &[Loader],
    ),
    (
        "[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion",
        &[Completion],
    ),
    (
        "[ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"  # This loads nvm",
        &[Loader],
    ),
    (
        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" --no-use # This loads nvm, without auto-using the default version",
        &[Loader],
    ),
    (
        "export NVM_DIR=\"$([ -z \"${XDG_CONFIG_HOME-}\" ] && printf %s \"${HOME}/.nvm\" || printf %s \"${XDG_CONFIG_HOME}/nvm\")\"",
        &[NvmDirExport],
    ),
    (
        "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
        &[Completion],
    ),
    (
        "  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm",
        &[Loader],
    ),
    (
        "  [ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion",
        &[Completion],
    ),
    ("source $(brew --prefix nvm)/nvm.sh", &[Loader]),
    (". \"$(brew --prefix nvm)/nvm.sh\"", &[Loader]),
    (
        "[ -s \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\" ] && \\. \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\"",
        &[Loader],
    ),
    (
        "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
        &[Loader],
    ),
    (
        "  [ -s \"${nvm_prefix}/etc/bash_completion.d/nvm\" ] && \\. \"${nvm_prefix}/etc/bash_completion.d/nvm\"",
        &[Completion],
    ),
    ("source ~/.nvm/nvm.sh", &[Loader]),
    (
        "nvm() { unset -f nvm; . \"$NVM_DIR/nvm.sh\"; nvm \"$@\"; }",
        &[LazyStub, Unset, Loader],
    ),
    (
        "      [[ -f \\\"\\$NVM_DIR/nvm.sh\\\" ]] && source \\\"\\$NVM_DIR/nvm.sh\\\"",
        &[Loader],
    ),
    (
        "  bass source ~/.nvm/nvm.sh --no-use ';' nvm $argv",
        &[Bass],
    ),
    (
        "# [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
        &[],
    ),
    (
        "# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
        &[],
    ),
    ("echo dnvm.sh", &[]),
    (". ~/.dnx/dnvm/dnvm.sh", &[]),
    ("source \"$HOME/.nvm/nvm.sh\"", &[Loader]),
];

#[test]
fn the_corpus_of_the_digest() {
    assert_rows(CORPUS_ROWS);
}

/// install.sh's older forms (1.3), the README (1.4) and Homebrew (2).
const INSTALLER_ROWS: &[(&str, &[Kind])] = &[
    ("export NVM_DIR=\"/home/u/.nvm\"", &[NvmDirExport]),
    (
        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" # This loads nvm",
        &[Loader],
    ),
    (
        "export NVM_DIR=\"$HOME/.nvm\" && ( git clone https://github.com/nvm-sh/nvm.git \"$NVM_DIR\" ) && \\. \"$NVM_DIR/nvm.sh\"",
        &[NvmDirExport, Loader],
    ),
    ("\\. \"/usr/local/opt/nvm/nvm.sh\"", &[Loader]),
    (
        "\\. \"/home/linuxbrew/.linuxbrew/opt/nvm/nvm.sh\"",
        &[Loader],
    ),
    ("source ${HOMEBREW_PREFIX}/opt/nvm/nvm.sh", &[Loader]),
    (
        "[ -s \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\"",
        &[Completion],
    ),
    ("source ~/.nvm/bash_completion", &[Completion]),
    ("source ${NVM_DIR}/bash_completion", &[Completion]),
    (
        "    nvm_path=\"$(nvm_find_up .nvmrc | command tr -d '\\n')\"",
        &[HelperCall],
    ),
    ("  local nvmrc_path=\"$(nvm_find_nvmrc)\"", &[HelperCall]),
    ("alias cd='cdnvm'", &[]),
    ("add-zsh-hook chpwd load-nvmrc", &[]),
    ("ZSH_VERSION= source \"$_nvm_completion\"", &[]),
];

#[test]
fn installer_readme_and_homebrew_forms() {
    assert_rows(INSTALLER_ROWS);
}

/// oh-my-zsh (3.1), zsh-nvm (3.2) and the blog patterns A to E (3.3).
const LAZY_LOADER_ROWS: &[(&str, &[Kind])] = &[
    ("which nvm &>/dev/null && return", &[]),
    ("zstyle ':omz:plugins:nvm' lazy yes", &[OmzPlugin]),
    ("zstyle \":omz:plugins:nvm\" autoload yes", &[OmzPlugin]),
    ("    function $nvm_lazy_cmd {", &[]),
    ("          unfunction \\$func", &[]),
    ("antigen bundle lukechilds/zsh-nvm", &[ZshNvm]),
    ("zplug \"lukechilds/zsh-nvm\"", &[ZshNvm]),
    ("zinit light lukechilds/zsh-nvm", &[ZshNvm]),
    ("export NVM_LAZY_LOAD=true", &[ZshNvm]),
    ("    eval \"$cmd(){", &[]),
    ("      unset -f $cmds > /dev/null 2>&1", &[]),
    ("lazynvm() {", &[]),
    ("  unset -f nvm node npm npx", &[Unset]),
    ("  export NVM_DIR=~/.nvm", &[NvmDirExport]),
    (
        "  [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"",
        &[Loader],
    ),
    ("nvm() { lazynvm; nvm $@; }", &[LazyStub]),
    ("node() { lazynvm; node $@; }", &[LazyStub]),
    ("lazy_load_nvm_cmds=(nvm node npm npx yarn)", &[]),
    ("for cmd in \"${lazy_load_nvm_cmds[@]}\"; do", &[]),
    (
        "  eval \"${cmd}() { unset -f ${lazy_load_nvm_cmds[*]}; source \\$NVM_DIR/nvm.sh; ${cmd} \\\"\\$@\\\"; }\"",
        &[Unset, Loader],
    ),
    (
        "alias nvm='unalias nvm node npm && . \"$NVM_DIR\"/nvm.sh && nvm'",
        &[Loader],
    ),
    (
        "export PATH=\"$NVM_DIR/versions/node/$(cat $NVM_DIR/alias/default)/bin:$PATH\"",
        &[],
    ),
];

#[test]
fn lazy_loaders_and_other_managers() {
    assert_rows(LAZY_LOADER_ROWS);
}

/// The user's own dotfiles (3.4).
const USERS_DOTFILES_ROWS: &[(&str, &[Kind])] = &[
    ("export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
    (
        "# Node (nvm itself is lazy-loaded via scripts/lazy-functions.zsh)",
        &[],
    ),
    (
        "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
        &[],
    ),
    ("  . \"$HOME/.swiftly/env.sh\"", &[]),
    ("plugins=(", &[]),
    ("    nvm", &[]),
    ("source $ZSH/oh-my-zsh.sh", &[]),
    ("source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh", &[]),
    ("[[ ! -d \"${NVM_DIR}\" ]] && mkdir \"${NVM_DIR}\"", &[]),
    ("nvm() {", &[LazyStub]),
    (
        "  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
        &[Unset],
    ),
    ("  export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
    (
        "  local nvm_prefix=\"${HOMEBREW_PREFIX:-/opt/homebrew}/opt/nvm\"",
        &[],
    ),
    ("  nvm \"$@\"", &[]),
    (
        "# node() { nvm >/dev/null 2>&1; command node \"$@\" } # TODO: Disabled to install a real node",
        &[],
    ),
    (
        "npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
        &[LazyStub],
    ),
    (
        "npx() { nvm >/dev/null 2>&1; command npx \"$@\" }",
        &[LazyStub],
    ),
    (
        "yarn() { nvm >/dev/null 2>&1; command yarn \"$@\" }",
        &[LazyStub],
    ),
    (
        "pnpm() { nvm >/dev/null 2>&1; command pnpm \"$@\" }",
        &[LazyStub],
    ),
];

#[test]
fn the_users_dotfiles() {
    assert_rows(USERS_DOTFILES_ROWS);
}

const STUB_ROWS: &[(&str, &[Kind])] = &[
    ("function nvm {", &[LazyStub]),
    ("function nvm() {", &[LazyStub]),
    ("function yarn{", &[LazyStub]),
    ("node () {", &[LazyStub]),
    ("corepack(){", &[LazyStub]),
    ("  pnpx ( ) {", &[LazyStub]),
    ("pnvm() {", &[]),
    ("nvmx() {", &[]),
    ("nvm()", &[]),
    ("unfunction nvm", &[Unset]),
    ("unset -f node nvm", &[Unset]),
    ("unset nvm", &[]),
    ("unset -f pnvm", &[]),
    ("myunset -f nvm", &[]),
    ("if nvm_has node; then", &[HelperCall]),
    ("nvm_ls_remote", &[]),
    (
        "echo $(nvm_version current) $(nvm_rc_version) $(nvm_echo x) $(nvm_ls)",
        &[HelperCall],
    ),
    ("source ~/.nvm/nvm.sh.bak", &[]),
    ("./nvm.sh", &[]),
    ("defer_source ~/.nvm/nvm.sh", &[]),
    ("cd ~/.nvm && . nvm.sh", &[Loader]),
    ("(source ~/.nvm/nvm.sh)", &[Loader]),
    ("true;source ~/.nvm/nvm.sh", &[Loader]),
    ("NVM_DIR=/opt/nvm", &[NvmDirExport]),
    ("MY_NVM_DIR=/opt/nvm", &[]),
];

#[test]
fn stub_unset_and_helper_forms_and_their_rejects() {
    assert_rows(STUB_ROWS);
}

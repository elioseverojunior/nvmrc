use super::Kind::{
    self, Completion, CompoundLoader, LazyLoader, LazyStub, Loader, NvmDirExport, OmzPlugin, Unset,
    ZshNvm,
};
use super::{BEGIN_MARKER, END_MARKER, Hit, scan_text};

/// `(line, kind)` of every hit.
fn found(text: &str) -> Vec<(usize, Kind)> {
    scan_text(text)
        .into_iter()
        .map(|hit| (hit.line, hit.kind))
        .collect()
}

/// `scripts/lazy-functions.zsh` of the user (digest 3.4), at its real line
/// numbers.
fn lazy_functions_zsh() -> String {
    let header = "# header\n".repeat(12);
    format!(
        "{header}nvm() {{\n\
  unfunction nvm node npm npx yarn pnpm 2>/dev/null\n\
  export NVM_DIR=\"${{HOME}}/.nvm\"\n\
  local nvm_prefix=\"${{HOMEBREW_PREFIX:-/opt/homebrew}}/opt/nvm\"\n\
  [ -s \"${{nvm_prefix}}/nvm.sh\" ] && \\. \"${{nvm_prefix}}/nvm.sh\"\n\
  [ -s \"${{nvm_prefix}}/etc/bash_completion.d/nvm\" ] && \\. \"${{nvm_prefix}}/etc/bash_completion.d/nvm\"\n\
  nvm \"$@\"\n\
}}\n\
\n\
\n\
# node() {{ nvm >/dev/null 2>&1; command node \"$@\" }} # TODO: Disabled to install a real node\n\
npm() {{ nvm >/dev/null 2>&1; command npm \"$@\" }}\n\
npx() {{ nvm >/dev/null 2>&1; command npx \"$@\" }}\n\
yarn() {{ nvm >/dev/null 2>&1; command yarn \"$@\" }}\n\
pnpm() {{ nvm >/dev/null 2>&1; command pnpm \"$@\" }}\n"
    )
}

#[test]
fn the_users_lazy_functions_file_is_all_manual() {
    assert_eq!(
        found(&lazy_functions_zsh()),
        [
            (13, LazyStub),
            (14, Unset),
            (15, NvmDirExport),
            (17, LazyLoader),
            (18, LazyLoader),
            (24, LazyStub),
            (25, LazyStub),
            (26, LazyStub),
            (27, LazyStub),
        ]
    );
}

#[test]
fn hits_keep_the_line_number_and_the_original_text() {
    let line = "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm";
    let text = format!(
        "\nexport NVM_DIR=\"$HOME/.nvm\"\n{line}\n[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n"
    );
    let hits = scan_text(&text);
    assert_eq!(
        hits.iter()
            .map(|hit| (hit.line, hit.kind))
            .collect::<Vec<_>>(),
        [(2, NvmDirExport), (3, Loader), (4, Completion)]
    );
    assert_eq!(
        hits[1],
        Hit {
            line: 3,
            kind: Loader,
            text: line.to_string()
        }
    );
}

#[test]
fn crlf_line_endings_are_not_part_of_the_text() {
    let hits = scan_text("source ~/.nvm/nvm.sh\r\n");
    assert_eq!(hits[0].text, "source ~/.nvm/nvm.sh");
}

#[test]
fn homebrew_caveats_pasted_indented_stay_auto_migratable() {
    let text = "  export NVM_DIR=\"$HOME/.nvm\"\n\
  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm\n\
  [ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion\n";
    assert_eq!(
        found(text),
        [(1, NvmDirExport), (2, Loader), (3, Completion)]
    );
}

#[test]
fn a_loader_inside_a_function_or_an_if_body_is_a_lazy_loader() {
    let text = "nvm_load() {\n  [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"\n}\n\
if [ -s \"$NVM_DIR/nvm.sh\" ]; then\n  . \"$NVM_DIR/nvm.sh\"\nfi\n\
if true; then . \"$NVM_DIR/nvm.sh\"\nfi\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"\n";
    assert_eq!(
        found(text),
        [
            (2, LazyLoader),
            (5, LazyLoader),
            (7, LazyLoader),
            (9, Completion)
        ]
    );
}

#[test]
fn a_one_line_if_is_top_level() {
    let text = "if [ -s \"$NVM_DIR/nvm.sh\" ]; then . \"$NVM_DIR/nvm.sh\"; fi\n";
    assert_eq!(found(text), [(1, Loader)]);
}

#[test]
fn a_stub_or_an_unset_anywhere_in_the_file_makes_every_loader_lazy() {
    let stub_after = "source ~/.nvm/nvm.sh\nnpm() { nvm; command npm \"$@\" }\n";
    assert_eq!(found(stub_after), [(1, LazyLoader), (2, LazyStub)]);
    let unset_before = "unset -f nvm\n. ~/.nvm/nvm.sh\n. ~/.nvm/bash_completion\n";
    assert_eq!(
        found(unset_before),
        [(1, Unset), (2, LazyLoader), (3, LazyLoader)]
    );
}

#[test]
fn blog_patterns_a_b_and_c_are_lazy_loaders() {
    let pattern_a = "nvm() { unset -f nvm; . \"$NVM_DIR/nvm.sh\"; nvm \"$@\"; }\n";
    assert_eq!(
        found(pattern_a),
        [(1, LazyStub), (1, Unset), (1, LazyLoader)]
    );
    let pattern_b = "lazynvm() {\n  unset -f nvm node npm npx\n  export NVM_DIR=~/.nvm\n\
  [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"\n}\nnvm() { lazynvm; nvm $@; }\n\
node() { lazynvm; node $@; }\nnpm() { lazynvm; npm $@; }\nnpx() { lazynvm; npx $@; }\n";
    assert_eq!(
        found(pattern_b),
        [
            (2, Unset),
            (3, NvmDirExport),
            (4, LazyLoader),
            (6, LazyStub),
            (7, LazyStub),
            (8, LazyStub),
            (9, LazyStub),
        ]
    );
    let pattern_c = "lazy_load_nvm_cmds=(nvm node npm npx yarn)\n\
for cmd in \"${lazy_load_nvm_cmds[@]}\"; do\n\
  eval \"${cmd}() { unset -f ${lazy_load_nvm_cmds[*]}; source \\$NVM_DIR/nvm.sh; ${cmd} \\\"\\$@\\\"; }\"\n\
done\n";
    assert_eq!(found(pattern_c), [(3, Unset), (3, LazyLoader)]);
}

#[test]
fn blog_patterns_d_and_e() {
    let pattern_d = "alias nvm='unalias nvm node npm && . \"$NVM_DIR\"/nvm.sh && nvm'\n";
    assert_eq!(found(pattern_d), [(1, CompoundLoader)]);
    let pattern_e =
        "export PATH=\"$NVM_DIR/versions/node/$(cat $NVM_DIR/alias/default)/bin:$PATH\"\n";
    assert_eq!(found(pattern_e), []);
}

#[test]
fn the_word_nvm_in_a_multi_line_plugins_block() {
    let text =
        "plugins=(\n    git\n    node\n    nvm\n#    nvm\n    'nvm'\n    pnvm\n)\necho nvm\nnvm\n";
    assert_eq!(found(text), [(4, OmzPlugin), (6, OmzPlugin)]);
}

#[test]
fn one_line_plugins_blocks() {
    assert_eq!(found("plugins=(git nvm)\n"), [(1, OmzPlugin)]);
    assert_eq!(found("plugins+=(nvm)\n"), [(1, OmzPlugin)]);
    assert_eq!(found("plugins=(git zsh-nvm)\n"), [(1, ZshNvm)]);
    assert_eq!(found("plugins=(git) ; echo nvm\n"), []);
    assert_eq!(found("my_plugins=(nvm)\n"), []);
    assert_eq!(
        found("zstyle ':omz:plugins:nvm' lazy yes\n"),
        [(1, OmzPlugin)]
    );
}

#[test]
fn comments_migrated_lines_and_the_init_block_are_skipped() {
    let text = format!(
        "# [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"\n\
# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"\n\
foo # . ~/.nvm/nvm.sh\n\
echo dnvm.sh\n\
. ~/.dnx/dnvm/dnvm.sh\n\
{BEGIN_MARKER}\n\
source ~/.nvm/nvm.sh\n\
nvm() {{ command nvm \"$@\"; }}\n\
{END_MARKER}\n\
source ~/.nvm/nvm.sh\n"
    );
    assert_eq!(found(&text), [(10, Loader)]);
}

#[test]
fn an_empty_text_has_no_hits() {
    assert_eq!(found(""), []);
}

#[test]
fn a_loader_sharing_its_line_with_other_commands_is_a_compound_loader() {
    let text = "export NVM_DIR=\"$HOME/.config/nvm\"; [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"\n\
export PATH=\"$HOME/bin:$PATH\"; source ~/.nvm/nvm.sh\n\
source ~/.nvm/nvm.sh; source ~/.nvm/bash_completion; nvm use 18\n";
    assert_eq!(
        found(text),
        [
            (1, NvmDirExport),
            (1, CompoundLoader),
            (2, CompoundLoader),
            (3, CompoundLoader)
        ]
    );
}

#[test]
fn a_continued_loader_and_the_line_it_continues_are_lazy_loaders() {
    let text = "[ -s \"$NVM_DIR/nvm.sh\" ] && \\\n  . \"$NVM_DIR/nvm.sh\"\n\
. ~/.nvm/nvm.sh \\\n  --no-use\n\
echo a \\\\\n. ~/.nvm/nvm.sh\n";
    assert_eq!(found(text), [(2, LazyLoader), (3, LazyLoader), (6, Loader)]);
}

#[test]
fn a_body_at_the_openers_indentation_is_a_lazy_loader() {
    let text = "if [ -s \"$NVM_DIR/nvm.sh\" ]; then\n. \"$NVM_DIR/nvm.sh\"\nfi\n\
case $- in\n*i*) . ~/.nvm/nvm.sh ;;\nesac\n. ~/.nvm/nvm.sh\n";
    assert_eq!(found(text), [(2, LazyLoader), (5, LazyLoader), (7, Loader)]);
}

#[test]
fn a_begin_marker_without_an_end_marker_hides_nothing() {
    let text = format!("{BEGIN_MARKER}\nsource ~/.nvm/nvm.sh\n");
    assert_eq!(found(&text), [(2, Loader)]);
    let stray = format!(
        "{BEGIN_MARKER}\nsource ~/.nvm/nvm.sh\n{BEGIN_MARKER}\neval \"$(nvmrc init zsh)\"\n{END_MARKER}\n"
    );
    assert_eq!(found(&stray), [(2, Loader)]);
}

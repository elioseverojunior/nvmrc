use super::{BlockChange, Migration, init_block, load_line, migrate_text, syntax_check};
use crate::domain::conflict::scan_text;
use crate::shell::init::{SHELLS, Shell};

const LOADER: &str = "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm";
const COMPLETION: &str = "[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion";
const BREW_LOADER: &str = "[ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm";
const BREW_COMPLETION: &str = "[ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion";
const ZSH_BLOCK: &str = "# >>> nvmrc init >>>\neval \"$(nvmrc init zsh)\"\n# <<< nvmrc init <<<\n";
const BASH_BLOCK: &str =
    "# >>> nvmrc init >>>\neval \"$(nvmrc init bash)\"\n# <<< nvmrc init <<<\n";

/// install.sh's appended bytes (digest 1.2) after a PATH line.
fn install_sh_bashrc() -> String {
    format!(
        "export PATH=\"$HOME/bin:$PATH\"\n\nexport NVM_DIR=\"$HOME/.nvm\"\n{LOADER}\n{COMPLETION}\n"
    )
}

/// Every fixture the properties run over, with its shell.
fn fixtures() -> Vec<(String, Shell)> {
    vec![
        (install_sh_bashrc(), Shell::Bash),
        (
            format!("export NVM_DIR=\"$HOME/.nvm\"\n{BREW_LOADER}\n{BREW_COMPLETION}\n"),
            Shell::Zsh,
        ),
        (format!("{LOADER}\nalias ll='ls -l'\n"), Shell::Bash),
        (format!("alias ll='ls -l'\n{LOADER}"), Shell::Bash),
        (users_zshrc(), Shell::Zsh),
        (users_style_loader(), Shell::Zsh),
        (install_sh_bashrc().replace('\n', "\r\n"), Shell::Bash),
        (format!("a\n{ZSH_BLOCK}b\n{LOADER}\n"), Shell::Zsh),
        (
            format!("if true; then\n  {LOADER}\nfi\nnvm() {{ :; }}\n"),
            Shell::Bash,
        ),
        (format!("  {LOADER}\n"), Shell::Bash),
        (
            format!("# [nvmrc-migrated] {LOADER}\n{ZSH_BLOCK}"),
            Shell::Zsh,
        ),
        ("source ~/.nvm/nvm.sh\n".to_owned(), Shell::Fish),
        (String::new(), Shell::Ksh),
    ]
}

/// The nvm-relevant lines of the user's `.zshrc` (digest 3.4): nothing to
/// migrate automatically.
fn users_zshrc() -> String {
    "plugins=(\n    node\n    nvm\n)\nsource $ZSH/oh-my-zsh.sh\nsource ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh\n\
[[ ! -d \"${NVM_DIR}\" ]] && mkdir \"${NVM_DIR}\"\n"
        .to_owned()
}

/// The user's loader lines (digest 3.4, `lazy-functions.zsh:17-18`) written
/// at the top level.
fn users_style_loader() -> String {
    "export NVM_DIR=\"${HOME}/.nvm\"\n\
[ -s \"${HOMEBREW_PREFIX}/opt/nvm/nvm.sh\" ] && \\. \"${HOMEBREW_PREFIX}/opt/nvm/nvm.sh\"\n\
[ -s \"${HOMEBREW_PREFIX}/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"${HOMEBREW_PREFIX}/opt/nvm/etc/bash_completion.d/nvm\"\n"
        .to_owned()
}

fn migrated(line: &str) -> String {
    format!("# [nvmrc-migrated] {line}")
}

#[test]
fn the_load_line_runs_the_nvmrc_binary_never_the_nvm_function() {
    let expected = [
        (Shell::Bash, "eval \"$(nvmrc init bash)\""),
        (Shell::Zsh, "eval \"$(nvmrc init zsh)\""),
        (Shell::Sh, "eval \"$(nvmrc init sh)\""),
        (Shell::Dash, "eval \"$(nvmrc init dash)\""),
        (Shell::Ksh, "eval \"$(nvmrc init ksh)\""),
        (Shell::Fish, "nvmrc init fish | source"),
    ];
    for (shell, line) in expected {
        assert_eq!(load_line(shell), line);
        assert_eq!(
            init_block(shell),
            format!("# >>> nvmrc init >>>\n{line}\n# <<< nvmrc init <<<\n")
        );
    }
}

#[test]
fn the_install_sh_pair_becomes_comments_under_one_block() {
    let result = migrate_text(&install_sh_bashrc(), Shell::Bash);
    let expected = format!(
        "export PATH=\"$HOME/bin:$PATH\"\n\nexport NVM_DIR=\"$HOME/.nvm\"\n{BASH_BLOCK}{}\n{}\n",
        migrated(LOADER),
        migrated(COMPLETION)
    );
    assert_eq!(
        result,
        Migration {
            text: expected,
            migrated_lines: vec![4, 5],
            block: BlockChange::Inserted { before_line: 4 },
        }
    );
}

#[test]
fn homebrew_lines_are_migrated_and_the_nvm_dir_export_stays() {
    let text = format!("export NVM_DIR=\"$HOME/.nvm\"\n{BREW_LOADER}\n{BREW_COMPLETION}\n");
    let result = migrate_text(&text, Shell::Zsh);
    let expected = format!(
        "export NVM_DIR=\"$HOME/.nvm\"\n{ZSH_BLOCK}{}\n{}\n",
        migrated(BREW_LOADER),
        migrated(BREW_COMPLETION)
    );
    assert_eq!(result.text, expected);
    assert_eq!(result.migrated_lines, [2, 3]);
}

#[test]
fn the_block_goes_before_a_loader_on_the_first_line() {
    let result = migrate_text(&format!("{LOADER}\nalias ll='ls -l'\n"), Shell::Bash);
    assert_eq!(
        result.text,
        format!("{BASH_BLOCK}{}\nalias ll='ls -l'\n", migrated(LOADER))
    );
    assert_eq!(result.block, BlockChange::Inserted { before_line: 1 });
}

#[test]
fn a_last_line_without_newline_stays_without() {
    let result = migrate_text(&format!("alias ll='ls -l'\n{LOADER}"), Shell::Bash);
    assert_eq!(
        result.text,
        format!("alias ll='ls -l'\n{BASH_BLOCK}{}", migrated(LOADER))
    );
    assert_eq!(result.migrated_lines, [2]);
}

#[test]
fn the_users_top_level_style_is_migrated() {
    let result = migrate_text(&users_style_loader(), Shell::Zsh);
    assert_eq!(result.migrated_lines, [2, 3]);
    assert!(
        result
            .text
            .starts_with(&format!("export NVM_DIR=\"${{HOME}}/.nvm\"\n{ZSH_BLOCK}"))
    );
}

#[test]
fn a_file_without_loaders_is_unchanged() {
    for text in [
        users_zshrc(),
        String::new(),
        "export NVM_DIR=\"$HOME/.nvm\"\n".to_owned(),
    ] {
        let result = migrate_text(&text, Shell::Zsh);
        assert_eq!(
            result,
            Migration {
                text: text.clone(),
                migrated_lines: Vec::new(),
                block: BlockChange::Unchanged,
            }
        );
    }
}

#[test]
fn manual_kinds_are_never_touched() {
    let lazy = format!("if true; then\n  {LOADER}\nfi\nnvm() {{ :; }}\nsource ~/.nvm/nvm.sh\n");
    let result = migrate_text(&lazy, Shell::Bash);
    assert_eq!(result.text, lazy);
    assert_eq!(result.block, BlockChange::Unchanged);
}

#[test]
fn an_existing_block_is_replaced_and_no_second_block_is_added() {
    let pasted = "# >>> nvmrc init >>>\nnvm() { old; }\n# <<< nvmrc init <<<\n";
    let text = format!("a\n{pasted}b\n{LOADER}\n");
    let result = migrate_text(&text, Shell::Zsh);
    assert_eq!(
        result.text,
        format!("a\n{ZSH_BLOCK}b\n{}\n", migrated(LOADER))
    );
    assert_eq!(result.migrated_lines, [6]);
    assert_eq!(result.block, BlockChange::Replaced);
}

#[test]
fn a_current_block_with_nothing_to_migrate_is_unchanged() {
    let text = format!("a\n{ZSH_BLOCK}b");
    let result = migrate_text(&text, Shell::Zsh);
    assert_eq!(result.text, text);
    assert_eq!(result.block, BlockChange::Unchanged);
}

#[test]
fn a_block_at_the_end_without_newline_keeps_that_state() {
    let text = "a\n# >>> nvmrc init >>>\nold\n# <<< nvmrc init <<<";
    let result = migrate_text(text, Shell::Fish);
    assert_eq!(
        result.text,
        "a\n# >>> nvmrc init >>>\nnvmrc init fish | source\n# <<< nvmrc init <<<"
    );
}

#[test]
fn indentation_is_kept_after_the_prefix() {
    let result = migrate_text(&format!("  {LOADER}\n"), Shell::Bash);
    assert_eq!(
        result.text,
        format!("{BASH_BLOCK}# [nvmrc-migrated]   {LOADER}\n")
    );
}

#[test]
fn crlf_files_stay_crlf() {
    let text = install_sh_bashrc().replace('\n', "\r\n");
    let result = migrate_text(&text, Shell::Bash);
    let expected = format!(
        "export PATH=\"$HOME/bin:$PATH\"\r\n\r\nexport NVM_DIR=\"$HOME/.nvm\"\r\n{}{}\r\n{}\r\n",
        BASH_BLOCK.replace('\n', "\r\n"),
        migrated(LOADER),
        migrated(COMPLETION)
    );
    assert_eq!(result.text, expected);
}

#[test]
fn migrated_lines_and_lines_inside_a_block_are_left_alone() {
    let block_with_loader = format!("# >>> nvmrc init >>>\n{LOADER}\n# <<< nvmrc init <<<\n");
    let already = format!("{}\n", migrated(LOADER));
    for text in [already.clone(), format!("{already}{ZSH_BLOCK}")] {
        assert_eq!(migrate_text(&text, Shell::Zsh).text, text);
    }
    let result = migrate_text(&block_with_loader, Shell::Zsh);
    assert_eq!(result.text, ZSH_BLOCK);
    assert!(result.migrated_lines.is_empty());
}

#[test]
fn migrating_twice_changes_nothing_the_second_time() {
    for shell in SHELLS {
        for (text, _) in fixtures() {
            let first = migrate_text(&text, shell);
            let second = migrate_text(&first.text, shell);
            assert_eq!(second.text, first.text, "{text:?}");
            assert_eq!(second.block, BlockChange::Unchanged, "{text:?}");
            assert!(second.migrated_lines.is_empty(), "{text:?}");
        }
    }
}

#[test]
fn the_migrated_text_has_no_auto_migratable_hit_left() {
    for (text, shell) in fixtures() {
        let result = migrate_text(&text, shell);
        let left: Vec<_> = scan_text(&result.text)
            .into_iter()
            .filter(|hit| hit.kind.is_auto_migratable())
            .collect();
        assert!(left.is_empty(), "{text:?} -> {left:?}");
    }
}

#[test]
fn the_syntax_checker_follows_the_file_name() {
    let cases = [
        ("/home/u/.zshrc", Some(("zsh", vec!["-n"]))),
        ("/home/u/dotfiles/zsh/.zprofile", Some(("zsh", vec!["-n"]))),
        ("/home/u/.bashrc", Some(("bash", vec!["-n"]))),
        ("/home/u/.bash_profile", Some(("bash", vec!["-n"]))),
        (
            "/home/u/.config/fish/config.fish",
            Some(("fish", vec!["--no-execute"])),
        ),
        (
            "/home/u/.config/fish/conf.d/nvm.fish",
            Some(("fish", vec!["--no-execute"])),
        ),
        ("/home/u/.kshrc", Some(("ksh", vec!["-n"]))),
        ("/home/u/.mkshrc", Some(("ksh", vec!["-n"]))),
        ("/home/u/.profile", Some(("sh", vec!["-n"]))),
        ("/home/u/zsh/.profile", Some(("sh", vec!["-n"]))),
        ("/home/u/.shinit", Some(("sh", vec!["-n"]))),
        ("", None),
        ("/home/u/", None),
    ];
    for (path, expected) in cases {
        assert_eq!(syntax_check(path), expected, "{path}");
    }
}

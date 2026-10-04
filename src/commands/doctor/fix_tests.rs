use std::path::Path;

use super::fix::suggested_patch;
use super::fix_text;
use crate::domain::conflict::Kind;

#[test]
fn a_top_level_loader_and_completion_are_fixed_by_migrate() {
    let advice = "run `nvm migrate` (comments the line out and adds the nvmrc init line)";
    assert_eq!(fix_text(Kind::Loader, 0), advice);
    assert_eq!(fix_text(Kind::Completion, 0), advice);
}

#[test]
fn a_loader_in_a_sourced_file_or_a_lazy_loader_is_fixed_by_hand() {
    let advice = "remove or replace this loader by hand: nvmrc has its own function \
(see `eval \"$(nvmrc init <shell>)\"`)";
    for (kind, depth) in [
        (Kind::Loader, 1),
        (Kind::Completion, 2),
        (Kind::LazyLoader, 0),
        (Kind::LazyLoader, 1),
    ] {
        assert_eq!(fix_text(kind, depth), advice, "{kind:?} at {depth}");
    }
}

#[test]
fn the_other_kinds_have_their_own_advice() {
    let stub = "the stub redefines `nvm` after the init line: delete it, or move the init \
line after it";
    assert_eq!(fix_text(Kind::LazyStub, 1), stub);
    assert_eq!(fix_text(Kind::Unset, 1), stub);
    assert!(fix_text(Kind::OmzPlugin, 0).starts_with("remove `nvm` from `plugins=(...)`; it does"));
    assert_eq!(fix_text(Kind::ZshNvm, 0), "remove zsh-nvm");
    assert_eq!(fix_text(Kind::Bass, 0), "remove the bass line");
    assert!(fix_text(Kind::HelperCall, 0).contains("use `nvm` subcommands"));
    assert_eq!(fix_text(Kind::NvmDirExport, 0), "kept by `nvm migrate`");
}

#[test]
fn only_loaders_fixed_by_hand_get_a_patch_in_the_shell_of_the_file() {
    let patch = |kind, depth, path| suggested_patch(kind, depth, Path::new(path));
    assert_eq!(patch(Kind::Loader, 0, "/h/.bashrc"), None);
    assert_eq!(patch(Kind::LazyStub, 1, "/h/lazy.zsh"), None);
    let by_hand = |shell: &str| {
        Some(format!(
            "replace the line with: eval \"$(nvmrc init {shell})\""
        ))
    };
    assert_eq!(patch(Kind::LazyLoader, 1, "/h/lazy.zsh"), by_hand("zsh"));
    assert_eq!(patch(Kind::Loader, 1, "/h/.kshrc"), by_hand("ksh"));
    assert_eq!(patch(Kind::Loader, 1, "/h/nvm.sh.d/x.sh"), by_hand("bash"));
    assert_eq!(
        patch(Kind::LazyLoader, 1, "/h/nvm.fish"),
        Some("replace the line with: nvmrc init fish | source".to_owned())
    );
}

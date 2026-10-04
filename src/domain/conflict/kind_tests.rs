use super::kind::{Kind, Severity};

const ALL: [Kind; 12] = [
    Kind::Loader,
    Kind::Completion,
    Kind::LazyLoader,
    Kind::CompoundLoader,
    Kind::LazyStub,
    Kind::Unset,
    Kind::OmzPlugin,
    Kind::ZshNvm,
    Kind::Bass,
    Kind::HelperCall,
    Kind::NvmDirExport,
    Kind::NotFollowed,
];

#[test]
fn severity_is_info_for_exports_and_unfollowed_sources_hint_for_helpers_and_omz() {
    for kind in ALL {
        let expected = match kind {
            Kind::NvmDirExport | Kind::NotFollowed => Severity::Info,
            Kind::HelperCall | Kind::OmzPlugin => Severity::Hint,
            _ => Severity::Conflict,
        };
        assert_eq!(kind.severity(), expected, "{kind:?}");
        assert_eq!(
            kind.is_conflict(),
            expected == Severity::Conflict,
            "{kind:?}"
        );
    }
}

#[test]
fn only_loaders_and_completions_are_auto_migratable() {
    let migratable: Vec<Kind> = ALL
        .into_iter()
        .filter(|kind| kind.is_auto_migratable())
        .collect();
    assert_eq!(migratable, [Kind::Loader, Kind::Completion]);
}

#[test]
fn every_kind_has_its_own_label_and_displays_it() {
    let labels: Vec<&str> = ALL.into_iter().map(Kind::label).collect();
    for (index, label) in labels.iter().enumerate() {
        assert!(!label.is_empty());
        assert!(!labels[..index].contains(label), "duplicate label {label}");
        assert_eq!(ALL[index].to_string(), *label);
    }
}

#[test]
fn labels_name_what_was_found() {
    assert_eq!(Kind::Loader.label(), "nvm.sh loader");
    assert_eq!(Kind::Completion.label(), "nvm bash_completion");
    assert_eq!(Kind::LazyLoader.label(), "lazy loader");
    assert_eq!(Kind::CompoundLoader.label(), "loader sharing its line");
    assert_eq!(Kind::OmzPlugin.label(), "oh-my-zsh nvm plugin");
    assert_eq!(Kind::NvmDirExport.label(), "NVM_DIR export");
}

#[test]
fn the_omz_plugin_is_a_hint_because_the_binary_on_path_neutralises_it() {
    assert_eq!(Kind::OmzPlugin.severity(), Severity::Hint);
    assert!(!Kind::OmzPlugin.is_conflict());
    assert!(!Kind::OmzPlugin.is_auto_migratable());
}

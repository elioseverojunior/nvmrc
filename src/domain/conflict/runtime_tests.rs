use super::runtime::{RUNTIME_WARNING_ADVICE, RuntimeConflict, runtime_warning};

#[test]
fn each_reason_has_the_name_the_snippet_passes() {
    assert_eq!(RuntimeConflict::Helpers.name(), "helpers");
    assert_eq!(RuntimeConflict::Function.name(), "function");
    for reason in RuntimeConflict::ALL {
        assert_eq!(RuntimeConflict::from_name(reason.name()), Some(reason));
    }
}

#[test]
fn other_names_are_no_reason() {
    for name in ["", "Helpers", "nvm.sh", "function ", "helper"] {
        assert_eq!(RuntimeConflict::from_name(name), None, "{name}");
    }
}

#[test]
fn the_advice_is_the_same_for_every_reason() {
    assert_eq!(
        RUNTIME_WARNING_ADVICE,
        "  nvmrc now answers `nvm`, but the old loader keeps running on every shell start;\n  \
         run `nvm doctor` to see where, and `nvm migrate` to move it"
    );
    for reason in RuntimeConflict::ALL {
        assert!(runtime_warning(reason).ends_with(RUNTIME_WARNING_ADVICE));
    }
}

#[test]
fn the_warning_names_why_on_its_first_line() {
    assert_eq!(
        runtime_warning(RuntimeConflict::Helpers),
        format!(
            "nvm: nvm.sh is still loaded in this shell (its helper functions are defined)\n\
             {RUNTIME_WARNING_ADVICE}"
        )
    );
    assert_eq!(
        runtime_warning(RuntimeConflict::Function),
        format!(
            "nvm: nvm.sh is still loaded in this shell (nvm was already a function)\n\
             {RUNTIME_WARNING_ADVICE}"
        )
    );
}

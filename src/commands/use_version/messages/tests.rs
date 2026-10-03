use super::*;

#[test]
fn a_version_that_is_not_an_alias_gets_its_v_prefix_and_the_install_hint() {
    assert_eq!(
        not_yet_installed("99", None, false),
        [
            "N/A: version \"v99\" is not yet installed.",
            "",
            "You need to run `nvm install 99` to install and use it.",
        ]
    );
}

#[test]
fn an_alias_names_where_it_leads() {
    let lines = not_yet_installed("missing", Some("v99"), false);
    assert_eq!(
        lines[0],
        "N/A: version \"missing -> v99\" is not yet installed."
    );
    let looped = not_yet_installed("loopa", Some("∞"), false);
    assert_eq!(
        looped[0],
        "N/A: version \"loopa -> ∞\" is not yet installed."
    );
}

#[test]
fn a_version_from_nvmrc_points_at_plain_nvm_install() {
    let lines = not_yet_installed("99", None, true);
    assert_eq!(
        lines[2],
        "You need to run `nvm install` to install and use the node version specified in `.nvmrc`."
    );
}

#[test]
fn lts_alone_explains_that_it_is_not_an_alias_even_from_nvmrc() {
    let expected = "`lts` is not an alias - you may need to run `nvm install --lts` to install and `nvm use --lts` to use it.";
    assert_eq!(not_yet_installed("lts", None, false)[2], expected);
    assert_eq!(not_yet_installed("lts", None, true)[2], expected);
}

#[test]
fn an_empty_version_keeps_the_double_space_of_nvm_sh() {
    assert_eq!(
        not_yet_installed("", None, false),
        [
            "N/A: version \"\" is not yet installed.",
            "",
            "You need to run `nvm install ` to install and use it.",
        ]
    );
}

#[test]
fn names_that_are_not_versions_stay_as_they_are() {
    for name in ["foo", "-", "lts/argon", "=x", "none"] {
        let expected = format!("N/A: version \"{name}\" is not yet installed.");
        assert_eq!(not_yet_installed(name, None, false)[0], expected);
    }
}

#[test]
fn the_loop_message_names_what_was_provided() {
    assert_eq!(
        infinite_loop("loopa"),
        "The alias \"loopa\" leads to an infinite loop. Aborting."
    );
    assert_eq!(
        infinite_loop(""),
        "The alias \"\" leads to an infinite loop. Aborting."
    );
}

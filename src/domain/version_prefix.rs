//! `nvm_ensure_version_prefix`: a leading digit gets the `v`, after any
//! `iojs-` prefix.

/// `20` becomes `v20` and `iojs-3` becomes `iojs-v3`; anything else (`node`,
/// `iojs-`, `lts/iron`, `∞`) is left as it is.
#[must_use]
pub fn with_v_prefix(name: &str) -> String {
    let (prefix, rest) = match name.strip_prefix("iojs-") {
        Some(rest) => ("iojs-", rest),
        None => ("", name),
    };
    if rest.starts_with(|first: char| first.is_ascii_digit()) {
        format!("{prefix}v{rest}")
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_v_prefix_is_added_after_the_iojs_prefix() {
        assert_eq!(with_v_prefix("20"), "v20");
        assert_eq!(with_v_prefix("iojs-3"), "iojs-v3");
        assert_eq!(with_v_prefix("iojs-v3"), "iojs-v3");
    }

    #[test]
    fn names_that_are_not_versions_are_left_alone() {
        for name in ["lts/iron", "node", "iojs", "iojs-", "\u{221e}", ""] {
            assert_eq!(with_v_prefix(name), name);
        }
    }
}

//! The line `nvm alias` prints for one alias, as `nvm.sh` prints it when stdout
//! is not a terminal (no colors).

/// `name -> target *` when the target already is the version, else
/// `name -> target (-> version *)`. The `*` marks a version that resolved; the
/// `(default)` suffix marks the implicit aliases.
#[must_use]
pub fn format_line(
    alias: &str,
    target: &str,
    version: &str,
    available: bool,
    default: bool,
) -> String {
    let marker = if available { " *" } else { "" };
    let line = if target == version {
        format!("{alias} -> {version}{marker}")
    } else {
        format!("{alias} -> {target} (-> {version}{marker})")
    };
    if default {
        format!("{line} (default)")
    } else {
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_that_is_the_version_prints_one_arrow() {
        let line = format_line("lts/iron", "v20.10.0", "v20.10.0", true, false);
        assert_eq!(line, "lts/iron -> v20.10.0 *");
    }

    #[test]
    fn a_target_that_resolves_shows_the_version_after_a_second_arrow() {
        let line = format_line("work", "v18", "v18.9.0", true, false);
        assert_eq!(line, "work -> v18 (-> v18.9.0 *)");
    }

    #[test]
    fn unresolved_targets_carry_no_star() {
        assert_eq!(
            format_line("old", "v16", "N/A", false, false),
            "old -> v16 (-> N/A)"
        );
        assert_eq!(format_line("a", "b", "∞", false, false), "a -> b (-> ∞)");
    }

    #[test]
    fn implicit_aliases_end_with_default() {
        let line = format_line("node", "stable", "v20.10.0", true, true);
        assert_eq!(line, "node -> stable (-> v20.10.0 *) (default)");
        let missing = format_line("unstable", "N/A", "N/A", false, true);
        assert_eq!(missing, "unstable -> N/A (default)");
    }
}

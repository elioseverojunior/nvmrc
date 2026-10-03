//! The two pure tests of `nvm_die_on_prefix`: whether an npmrc file sets a
//! `prefix` (`nvm_npmrc_bad_news_bears`) and whether a prefix lies inside
//! `$NVM_DIR` (`nvm_tree_contains_path`).

/// The keys an npmrc file must not set.
const KEYS: [&str; 2] = ["prefix", "globalconfig"];

/// True when a line of `contents` matches the ERE `^(prefix|globalconfig) *=`:
/// the key at the very start, then spaces (no tabs), then `=`.
#[must_use]
pub fn sets_prefix(contents: &str) -> bool {
    contents.split('\n').any(|line| {
        KEYS.iter().any(|key| {
            line.strip_prefix(key)
                .is_some_and(|rest| rest.trim_start_matches(' ').starts_with('='))
        })
    })
}

/// `nvm_tree_contains_path TREE PATH` as a pure string walk: `path` is `tree`
/// or below it once the trailing slashes of `tree` are dropped. Nothing is
/// resolved, so `..` and symbolic links count as written. An empty argument
/// is never contained (nvm.sh fails on it).
#[must_use]
pub fn tree_contains_path(tree: &str, path: &str) -> bool {
    if tree.is_empty() || path.is_empty() {
        return false;
    }
    let tree = match tree.trim_end_matches('/') {
        "" => "/",
        trimmed => trimmed,
    };
    let mut directory = path;
    loop {
        if directory == tree {
            return true;
        }
        if directory == "." || directory == "/" {
            return false;
        }
        directory = match directory.rsplit_once('/') {
            Some(("", _)) => "/",
            Some((parent, _)) => parent,
            None => return false,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prefix_or_globalconfig_line_is_a_setting() {
        assert!(sets_prefix("prefix=/x"));
        assert!(sets_prefix("color=true\nglobalconfig = /y\n"));
        assert!(sets_prefix("prefix   =/x\r\n"));
        assert!(sets_prefix("prefix="));
    }

    #[test]
    fn anything_else_is_not() {
        assert!(!sets_prefix(""));
        assert!(!sets_prefix(" prefix=/x"));
        assert!(!sets_prefix("prefix\t=/x"));
        assert!(!sets_prefix("prefixes=/x"));
        assert!(!sets_prefix("; prefix=/x\n#globalconfig=/y"));
        assert!(!sets_prefix("PREFIX=/x"));
        assert!(!sets_prefix("prefix /x"));
    }

    #[test]
    fn a_path_is_inside_its_own_tree_and_below_it() {
        assert!(tree_contains_path("/n", "/n"));
        assert!(tree_contains_path("/n", "/n/versions/node/v18.20.4"));
        assert!(tree_contains_path("/n//", "/n/x"));
        assert!(tree_contains_path("/", "/foo"));
        assert!(tree_contains_path("/", "/"));
    }

    #[test]
    fn a_path_elsewhere_or_a_sibling_with_the_same_start_is_not() {
        assert!(!tree_contains_path("/n", "/foo"));
        assert!(!tree_contains_path("/n", "/nvm/x"));
        assert!(!tree_contains_path("/n", "relative/n"));
        assert!(!tree_contains_path("/n", "n"));
        assert!(!tree_contains_path("", "/n"));
        assert!(!tree_contains_path("/n", ""));
    }
}

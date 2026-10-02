//! Alias resolution: follow `a -> b -> v20.0.0`, detecting cycles.

use std::collections::HashSet;

use crate::error::AliasError;

/// Names that always exist and have no alias file.
pub const BUILTIN_ALIASES: [&str; 5] = ["stable", "unstable", "iojs", "node", "system"];

pub trait AliasStore {
    /// The target an alias points to, or `None` when `name` is not an alias.
    fn target(&self, name: &str) -> Option<String>;
}

/// Follows the alias chain from `name` to its final target. A name that is not
/// an alias resolves to itself.
///
/// # Errors
/// Returns [`AliasError::Loop`] when the chain revisits a name.
pub fn resolve(store: &dyn AliasStore, name: &str) -> Result<String, AliasError> {
    let mut seen = HashSet::from([name.to_owned()]);
    let mut current = name.to_owned();
    while let Some(next) = store.target(&current) {
        if !seen.insert(next.clone()) {
            return Err(AliasError::Loop(name.to_owned()));
        }
        current = next;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::error::AliasError;

    use super::*;

    struct MapStore(HashMap<&'static str, &'static str>);

    impl AliasStore for MapStore {
        fn target(&self, name: &str) -> Option<String> {
            self.0.get(name).map(|target| (*target).to_owned())
        }
    }

    fn store(pairs: &[(&'static str, &'static str)]) -> MapStore {
        MapStore(pairs.iter().copied().collect())
    }

    #[test]
    fn a_name_that_is_not_an_alias_resolves_to_itself() {
        assert_eq!(resolve(&store(&[]), "v20.0.0").unwrap(), "v20.0.0");
    }

    #[test]
    fn follows_a_chain_to_its_end() {
        let aliases = store(&[("default", "lts"), ("lts", "v20.0.0")]);
        assert_eq!(resolve(&aliases, "default").unwrap(), "v20.0.0");
    }

    #[test]
    fn a_cycle_is_an_alias_loop_error() {
        let aliases = store(&[("a", "b"), ("b", "a")]);
        assert_eq!(resolve(&aliases, "a"), Err(AliasError::Loop("a".into())));
    }

    #[test]
    fn an_alias_pointing_at_itself_is_a_loop() {
        let aliases = store(&[("a", "a")]);
        assert_eq!(resolve(&aliases, "a"), Err(AliasError::Loop("a".into())));
    }
}

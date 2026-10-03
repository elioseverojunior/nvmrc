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

/// The name `nvm.sh`'s `nvm_version` matches against the installed versions.
/// Unlike [`resolve`], the bare names never read their own alias file: `node`
/// means `stable`, and `iojs` stays `iojs` (the newest io.js). A chain that
/// ends at `node` is resolved again as `stable`, through the alias files.
///
/// # Errors
/// Returns [`AliasError::Loop`] when a chain revisits a name, and when
/// `stable` leads back to `node` (where `nvm.sh` recurses without end).
pub fn lookup_target(store: &dyn AliasStore, name: &str) -> Result<String, AliasError> {
    let end = match name {
        "iojs" | "node" => name.to_owned(),
        _ => resolve(store, name)?,
    };
    if end != "node" {
        return Ok(end);
    }
    match resolve(store, "stable")? {
        stable if stable == "node" => Err(AliasError::Loop(name.to_owned())),
        stable => Ok(stable),
    }
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

    #[test]
    fn lookup_reads_bare_node_as_stable_and_never_follows_iojs() {
        let aliases = store(&[("node", "18"), ("iojs", "iojs"), ("stable", "16")]);
        assert_eq!(lookup_target(&aliases, "node").unwrap(), "16");
        assert_eq!(lookup_target(&aliases, "iojs").unwrap(), "iojs");
        assert_eq!(lookup_target(&store(&[]), "node").unwrap(), "stable");
    }

    #[test]
    fn lookup_resolves_a_chain_ending_at_node_again_as_stable() {
        let aliases = store(&[("default", "node"), ("stable", "lts/*"), ("lts/*", "20")]);
        assert_eq!(lookup_target(&aliases, "default").unwrap(), "20");
        let through_node = store(&[("default", "node"), ("node", "18")]);
        assert_eq!(lookup_target(&through_node, "default").unwrap(), "18");
        let to_iojs = store(&[("y", "iojs")]);
        assert_eq!(lookup_target(&to_iojs, "y").unwrap(), "iojs");
    }

    #[test]
    fn lookup_treats_stable_leading_back_to_node_as_a_loop() {
        let aliases = store(&[("stable", "node"), ("default", "node")]);
        let looped = |name: &str| Err(AliasError::Loop(name.into()));
        assert_eq!(lookup_target(&aliases, "node"), looped("node"));
        assert_eq!(lookup_target(&aliases, "default"), looped("default"));
    }
}

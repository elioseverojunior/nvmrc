//! The `PATH` entries of the pinned package managers, which live apart from
//! node's: `$NVM_DIR/package-managers/<tool>/<version>/bin`.

/// `path` without the entries under `<nvm_dir>/package-managers/`; every other
/// entry, an empty one too, stays where it is.
#[must_use]
pub fn strip_tool_bins(path: &str, nvm_dir: &str) -> String {
    let root = format!("{nvm_dir}/package-managers/");
    path.split(':')
        .filter(|entry| !entry.starts_with(&root))
        .collect::<Vec<_>>()
        .join(":")
}

/// `path` with `directories` ahead of it, in order. An empty `path` adds no
/// empty entry (an empty entry means the working directory).
#[must_use]
pub fn prepend(path: &str, directories: &[String]) -> String {
    directories
        .iter()
        .map(String::as_str)
        .chain((!path.is_empty()).then_some(path))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_package_manager_entries() {
        let path = "/n/package-managers/pnpm/9.12.0/bin:/n/versions/node/v20.11.1/bin::/usr/bin:\
/n/package-managers/npm/10.9.0/bin:/other/package-managers/x/1.0.0/bin";
        let expected =
            "/n/versions/node/v20.11.1/bin::/usr/bin:/other/package-managers/x/1.0.0/bin";
        assert_eq!(strip_tool_bins(path, "/n"), expected);
        assert_eq!(strip_tool_bins("", "/n"), "");
        assert_eq!(strip_tool_bins("/usr/bin:", "/n"), "/usr/bin:");
    }

    #[test]
    fn prepends_in_order() {
        let directories = ["/a/bin".to_owned(), "/b/bin".to_owned()];
        assert_eq!(prepend("/usr/bin", &directories), "/a/bin:/b/bin:/usr/bin");
        assert_eq!(prepend("/usr/bin", &[]), "/usr/bin");
        assert_eq!(prepend("", &directories), "/a/bin:/b/bin");
    }
}

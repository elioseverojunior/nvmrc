//! The global packages of a node, read from `npm list -g --depth=0`, as
//! `nvm_npm_global_modules` does with `sed`.

/// What `reinstall-packages` installs and links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalPackages {
    /// `name@version`, without `npm` and `corepack`, which come with node.
    pub installs: Vec<String>,
    /// Where the linked packages point (`pkg -> /path`).
    pub links: Vec<String>,
}

/// `npm list -g --depth=0` without its first line (the prefix) and without the
/// lines about unmet peer dependencies.
fn listed(output: &str) -> impl Iterator<Item = &str> {
    output
        .lines()
        .skip(1)
        .filter(|line| !line.contains("UNMET PEER DEPENDENCY"))
}

/// `s/^.* \(.*@[^ ]*\).*/\1/`: from the last space that still leaves an `@`
/// after it, up to the end of the word that holds the last `@`. A line that
/// does not fit stays as it is.
fn package_of(line: &str) -> &str {
    let Some(space) = line
        .char_indices()
        .rev()
        .find(|&(index, character)| character == ' ' && line[index + 1..].contains('@'))
        .map(|(index, _)| index)
    else {
        return line;
    };
    let rest = &line[space + 1..];
    let at = rest.rfind('@').unwrap_or(0);
    let word_end = rest[at..]
        .find(' ')
        .map_or(rest.len(), |offset| at + offset);
    &rest[..word_end]
}

/// # Panics
/// Never.
#[must_use]
pub fn parse(output: &str) -> GlobalPackages {
    let installs = listed(output)
        .filter(|line| !line.contains(" -> ") && !line.contains("(empty)"))
        .map(package_of)
        .filter(|package| !is_bundled(package))
        .flat_map(str::split_whitespace)
        .map(str::to_owned)
        .collect();
    let links = listed(output)
        .filter_map(|line| line.rfind(" -> ").map(|at| line[at + 4..].to_owned()))
        .collect();
    GlobalPackages { installs, links }
}

/// `^npm@` and `^corepack@`.
fn is_bundled(package: &str) -> bool {
    package.starts_with("npm@") || package.starts_with("corepack@")
}

#[cfg(test)]
mod tests;

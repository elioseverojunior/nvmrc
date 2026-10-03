//! `$NVM_DIR/default-packages`: one package per line, `#` comments, installed
//! with `npm install -g` after every install (`nvm_get_default_packages`).

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "Only one package per line is allowed in `{file}`. Please remove any lines with multiple space-separated values."
)]
pub struct DefaultPackagesError {
    pub file: String,
}

/// What `nvm.sh`'s `awk` prints for the file: the packages joined by spaces,
/// with a space in front of every one that is not on the first line (so a
/// file that starts with a comment gives a string that starts with a space,
/// as in `nvm.sh`). Empty when there are no packages.
///
/// # Errors
/// [`DefaultPackagesError`] for a line with a space or a tab that is not a
/// comment: more than one value on it.
pub fn parse(contents: &str, file: &str) -> Result<String, DefaultPackagesError> {
    let mut joined = String::new();
    for (index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.contains([' ', '\t']) {
            return Err(DefaultPackagesError {
                file: file.to_owned(),
            });
        }
        if index > 0 {
            joined.push(' ');
        }
        joined.push_str(line);
    }
    Ok(joined)
}

#[cfg(test)]
mod tests;

//! The `packageManager` field of `package.json`, checked against a pin.

use thiserror::Error;

use crate::domain::tool::Tool;
use crate::domain::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PackageManagerError {
    #[error("{0}")]
    Malformed(String),
    #[error("no packageManager")]
    Missing,
    #[error("packageManager {found:?} differs")]
    Different { found: String },
}

/// The `packageManager` value that matches a pin: `<name>@<version>`.
#[must_use]
pub fn package_manager_field(tool: Tool, version: &Version) -> String {
    format!("{}@{}", tool.name(), tool.show(version))
}

/// Checks the `packageManager` field of the `package.json` in `text` against
/// the pinned `tool` and `version`: it must read `<name>@<version>`, with a
/// corepack `+<hash>` suffix ignored.
pub fn check_package_manager(
    text: &str,
    tool: Tool,
    version: &Version,
) -> Result<(), PackageManagerError> {
    let document: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| PackageManagerError::Malformed(error.to_string()))?;
    let found = match document.get("packageManager") {
        None | Some(serde_json::Value::Null) => return Err(PackageManagerError::Missing),
        Some(serde_json::Value::String(field)) => field.clone(),
        Some(other) => other.to_string(),
    };
    let expected = package_manager_field(tool, version);
    let without_hash = found.split('+').next().unwrap_or("");
    if without_hash == expected {
        Ok(())
    } else {
        Err(PackageManagerError::Different { found })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pnpm() -> Version {
        "9.12.0".parse().unwrap()
    }

    fn check(text: &str) -> Result<(), PackageManagerError> {
        check_package_manager(text, Tool::Pnpm, &pnpm())
    }

    #[test]
    fn a_matching_package_manager_passes() {
        assert_eq!(check(r#"{"packageManager": "pnpm@9.12.0"}"#), Ok(()));
    }

    #[test]
    fn a_corepack_hash_suffix_is_ignored() {
        let text = r#"{"packageManager": "pnpm@9.12.0+sha512.abc"}"#;
        assert_eq!(check(text), Ok(()));
    }

    #[test]
    fn a_missing_field_is_reported() {
        assert_eq!(check(r#"{"name": "x"}"#), Err(PackageManagerError::Missing));
    }

    #[test]
    fn another_name_or_version_is_different() {
        for found in ["yarn@9.12.0", "pnpm@9.12.1", "pnpm", "pnpm@9.12"] {
            let text = format!(r#"{{"packageManager": "{found}"}}"#);
            assert_eq!(
                check(&text),
                Err(PackageManagerError::Different {
                    found: found.into()
                })
            );
        }
    }

    #[test]
    fn malformed_json_is_reported() {
        assert!(matches!(
            check("{not json"),
            Err(PackageManagerError::Malformed(_))
        ));
        assert!(matches!(check("[]"), Err(PackageManagerError::Missing)));
    }
}

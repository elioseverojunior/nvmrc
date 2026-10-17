//! The tools a policy covers: node and the package managers.

use crate::domain::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    Node,
    Npm,
    Yarn,
    Pnpm,
}

impl Tool {
    pub const PACKAGE_MANAGERS: [Self; 3] = [Self::Npm, Self::Yarn, Self::Pnpm];

    /// The name used in policy fields, flags and messages.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Npm => "npm",
            Self::Yarn => "yarn",
            Self::Pnpm => "pnpm",
        }
    }

    /// A version as users know it: node with a `v`, package managers without.
    #[must_use]
    pub fn show(self, version: &Version) -> String {
        match self {
            Self::Node => version.to_string(),
            _ => format!("{}.{}.{}", version.major, version.minor, version.patch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_node_with_a_v_and_package_managers_without() {
        let version: Version = "v20.11.1".parse().unwrap();
        assert_eq!(Tool::Node.show(&version), "v20.11.1");
        assert_eq!(Tool::Npm.show(&version), "20.11.1");
        assert_eq!(Tool::Pnpm.name(), "pnpm");
        assert_eq!(Tool::PACKAGE_MANAGERS.len(), 3);
    }
}

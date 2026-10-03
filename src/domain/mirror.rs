//! Where releases are listed and downloaded from: `NVM_NODEJS_ORG_MIRROR` and
//! `NVM_IOJS_ORG_MIRROR`, validated the way `nvm_get_mirror` does.

use thiserror::Error;

use crate::domain::version::Flavor;
use crate::ports::Env;

const NODE_DEFAULT: &str = "https://nodejs.org/dist";
const IOJS_DEFAULT: &str = "https://iojs.org/dist";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL")]
pub struct MirrorError;

/// An `http://` or `https://` URL made only of the characters a mirror needs:
/// letters, digits and `. _ ~ : / @ % [ ] -`. Anything else (spaces, quotes,
/// backticks, `?`, `#`, `;`, `$`...) is rejected, because `nvm.sh` appends
/// paths to it and passes it to shell commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorUrl(String);

fn is_allowed(character: char) -> bool {
    character.is_ascii_alphanumeric() || "._~:/@%[]-".contains(character)
}

impl MirrorUrl {
    /// # Errors
    /// Returns [`MirrorError`] for anything that is not a plain mirror URL.
    pub fn parse(raw: &str) -> Result<Self, MirrorError> {
        let rest = raw
            .strip_prefix("https://")
            .or_else(|| raw.strip_prefix("http://"))
            .ok_or(MirrorError)?;
        if rest.is_empty() || !rest.chars().all(is_allowed) {
            return Err(MirrorError);
        }
        Ok(Self(raw.to_owned()))
    }

    /// `<mirror>/<path>`, appended as is (a trailing slash in the mirror
    /// stays, as in `nvm.sh`).
    #[must_use]
    pub fn join(&self, path: &str) -> String {
        format!("{}/{path}", self.0)
    }

    /// The release index: `<mirror>/index.tab`.
    #[must_use]
    pub fn index_url(&self) -> String {
        self.join("index.tab")
    }
}

/// The mirror for `flavor`: its variable when set and not empty, else the
/// default.
///
/// # Errors
/// Returns [`MirrorError`] when the configured value is not a plain URL.
pub fn from_env(env: &dyn Env, flavor: Flavor) -> Result<MirrorUrl, MirrorError> {
    let (variable, default) = match flavor {
        Flavor::Node => ("NVM_NODEJS_ORG_MIRROR", NODE_DEFAULT),
        Flavor::IoJs => ("NVM_IOJS_ORG_MIRROR", IOJS_DEFAULT),
    };
    let configured = env.var(variable).filter(|value| !value.is_empty());
    MirrorUrl::parse(configured.as_deref().unwrap_or(default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeEnv;

    #[test]
    fn plain_http_and_https_urls_are_accepted() {
        for raw in [
            "https://nodejs.org/dist",
            "http://127.0.0.1:18081",
            "https://user@mirror.example.com:8443/node/dist/",
            "http://[::1]:8080/dist",
            "https://mirror.example.com/a_b~c%20d-e",
        ] {
            assert!(MirrorUrl::parse(raw).is_ok(), "{raw}");
        }
    }

    #[test]
    fn anything_else_is_rejected() {
        for raw in [
            "",
            "http://",
            "ftp://x/y",
            "nodejs.org/dist",
            "HTTP://x/y",
            "http://a b",
            "http://x/y?z=1",
            "http://x/y#frag",
            "http://x/`id`",
            "http://x/'q'",
            "http://x/(a)",
            "http://x/a\\b",
            "http://x/a;b",
            "http://x/$HOME",
            "http://x/ä",
        ] {
            assert_eq!(MirrorUrl::parse(raw), Err(MirrorError), "{raw:?}");
        }
    }

    #[test]
    fn the_error_message_is_the_one_of_nvm_sh() {
        assert_eq!(
            MirrorError.to_string(),
            "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL"
        );
    }

    #[test]
    fn the_index_is_appended_as_is() {
        let mirror = MirrorUrl::parse("http://x/dist/").unwrap();
        assert_eq!(mirror.index_url(), "http://x/dist//index.tab");
        let plain = MirrorUrl::parse("http://x/dist").unwrap();
        assert_eq!(plain.index_url(), "http://x/dist/index.tab");
    }

    #[test]
    fn paths_are_joined_with_one_slash_after_the_mirror() {
        let mirror = MirrorUrl::parse("http://x/dist").unwrap();
        assert_eq!(
            mirror.join("v1.0.0/f.tar.gz"),
            "http://x/dist/v1.0.0/f.tar.gz"
        );
    }

    #[test]
    fn the_defaults_are_nodejs_org_and_iojs_org() {
        let env = FakeEnv::default();
        let node = from_env(&env, Flavor::Node).unwrap();
        assert_eq!(node.index_url(), "https://nodejs.org/dist/index.tab");
        let iojs = from_env(&env, Flavor::IoJs).unwrap();
        assert_eq!(iojs.index_url(), "https://iojs.org/dist/index.tab");
    }

    #[test]
    fn the_variables_override_the_defaults_unless_empty() {
        let env = FakeEnv::default()
            .with_var("NVM_NODEJS_ORG_MIRROR", "http://127.0.0.1:1")
            .with_var("NVM_IOJS_ORG_MIRROR", "");
        let node = from_env(&env, Flavor::Node).unwrap();
        assert_eq!(node.index_url(), "http://127.0.0.1:1/index.tab");
        let iojs = from_env(&env, Flavor::IoJs).unwrap();
        assert_eq!(iojs.index_url(), "https://iojs.org/dist/index.tab");
    }

    #[test]
    fn an_invalid_variable_is_an_error_for_that_flavor_only() {
        let env = FakeEnv::default().with_var("NVM_NODEJS_ORG_MIRROR", "ftp://x/y");
        assert_eq!(from_env(&env, Flavor::Node), Err(MirrorError));
        assert!(from_env(&env, Flavor::IoJs).is_ok());
    }
}

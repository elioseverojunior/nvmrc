//! Talking to the npm registry: the manifest of a version and the date it was
//! published. Every request goes over https, redirects included.

use std::fmt;

use crate::context::Context;
use crate::domain::registry::{
    Manifest, RegistryError, manifest_url, packument_url, parse_manifest, release_day,
};
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::ports::{Env, Http, HttpError};

const REGISTRY_VARIABLE: &str = "NVMRC_NPM_REGISTRY";
const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org";

/// What went wrong and where: a URL or a variable name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchError {
    pub location: String,
    pub reason: String,
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.location, self.reason)
    }
}

impl std::error::Error for FetchError {}

fn is_https(url: &str) -> bool {
    url.get(.."https://".len())
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// The registry to ask: `NVMRC_NPM_REGISTRY` (default npmjs), without a
/// trailing `/`.
///
/// # Errors
/// When the registry is not an https URL.
pub fn registry_base(context: &Context<'_>) -> Result<String, FetchError> {
    registry_base_from(context.env)
}

/// [`registry_base`] read from `env` alone, for wiring the clients before
/// there is a context.
///
/// # Errors
/// When the registry is not an https URL.
pub fn registry_base_from(env: &dyn Env) -> Result<String, FetchError> {
    let value = env
        .var(REGISTRY_VARIABLE)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REGISTRY.to_owned());
    if !is_https(&value) {
        return Err(FetchError {
            location: REGISTRY_VARIABLE.to_owned(),
            reason: format!("`{value}` is not an https URL"),
        });
    }
    Ok(value.trim_end_matches('/').to_owned())
}

fn http_reason(error: HttpError) -> String {
    match error {
        HttpError::Status { code, .. } => format!("HTTP {code}"),
        HttpError::Transport { message, .. } | HttpError::Body { message, .. } => message,
    }
}

fn fetch_text(http: &dyn Http, url: &str) -> Result<String, FetchError> {
    http.get_secure_text(url).map_err(|error| FetchError {
        location: url.to_owned(),
        reason: http_reason(error),
    })
}

/// Like [`fetch_text`], but read as bytes: a packument (26 MB for npm) is past
/// the text limit. A body that is not UTF-8 is unusable.
fn fetch_large_text(http: &dyn Http, url: &str) -> Result<String, FetchError> {
    let failure = |reason: String| FetchError {
        location: url.to_owned(),
        reason,
    };
    let bytes = http
        .get_secure_bytes(url)
        .map_err(|error| failure(http_reason(error)))?;
    String::from_utf8(bytes).map_err(|error| failure(error.to_string()))
}

fn invalid(url: &str, error: &RegistryError) -> FetchError {
    FetchError {
        location: url.to_owned(),
        reason: error.to_string(),
    }
}

/// The manifest of `tool` at `version`.
///
/// # Errors
/// When the registry is refused, the request fails or the manifest is unusable.
pub fn fetch_manifest(
    context: &Context<'_>,
    tool: Tool,
    version: &Version,
) -> Result<Manifest, FetchError> {
    let url = manifest_url(&registry_base(context)?, tool, version);
    let text = fetch_text(context.third_party_http(), &url)?;
    parse_manifest(&text).map_err(|error| invalid(&url, &error))
}

/// The packument of the package that ships `tool` at `version`, read with the
/// third-party client.
///
/// # Errors
/// When the registry is refused or the request fails.
pub fn fetch_packument(
    context: &Context<'_>,
    tool: Tool,
    version: &Version,
) -> Result<String, FetchError> {
    packument_with(context, context.third_party_http(), tool, version).map(|(_, text)| text)
}

/// The packument and the URL it came from, asked of `http`.
fn packument_with(
    context: &Context<'_>,
    http: &dyn Http,
    tool: Tool,
    version: &Version,
) -> Result<(String, String), FetchError> {
    let url = packument_url(&registry_base(context)?, tool, version);
    let text = fetch_large_text(http, &url)?;
    Ok((url, text))
}

/// The day `version` was published (days since 1970-01-01), asked of `http`
/// (the caller may pass a client that retries); `None` when the registry does
/// not list it.
///
/// # Errors
/// When the registry is refused, the request fails or the packument is unusable.
pub fn fetch_release_day(
    context: &Context<'_>,
    http: &dyn Http,
    tool: Tool,
    version: &Version,
) -> Result<Option<i64>, FetchError> {
    let (url, text) = packument_with(context, http, tool, version)?;
    release_day(&text, version).map_err(|error| invalid(&url, &error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

    const NPM_URL: &str = "https://registry.npmjs.org/npm/10.9.0";
    const NPM_MANIFEST: &str = r#"{"bin": {"npm": "bin/npm-cli.js"},
        "dist": {"tarball": "https://t/npm.tgz", "integrity": "sha512-x"}}"#;

    fn base_with(registry: Option<&str>) -> Result<String, FetchError> {
        let fs = FakeFileSystem::default();
        let env = registry.map_or_else(FakeEnv::default, |value| {
            FakeEnv::default().with_var(REGISTRY_VARIABLE, value)
        });
        registry_base(&Context::new(&fs, &env))
    }

    #[test]
    fn the_default_registry_is_npmjs() {
        assert_eq!(base_with(None).unwrap(), "https://registry.npmjs.org");
    }

    #[test]
    fn a_trailing_slash_is_trimmed() {
        assert_eq!(
            base_with(Some("https://mirror.test/npm/")).unwrap(),
            "https://mirror.test/npm"
        );
    }

    #[test]
    fn an_http_registry_is_refused() {
        let error = base_with(Some("http://mirror.test")).unwrap_err();
        assert_eq!(error.location, REGISTRY_VARIABLE);
        assert!(error.to_string().contains("not an https URL"), "{error}");
    }

    #[test]
    fn a_manifest_is_fetched_from_the_registry_and_parsed() {
        let http = FakeHttp::default().with_body(NPM_URL, NPM_MANIFEST);
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let context = Context::new(&fs, &env).with_third_party_http(&http, &http);
        let version: Version = "v10.9.0".parse().unwrap();
        let manifest = fetch_manifest(&context, Tool::Npm, &version).unwrap();
        assert_eq!(manifest.tarball, "https://t/npm.tgz");
    }

    #[test]
    fn registry_requests_use_the_third_party_client() {
        let regular = FakeHttp::default();
        let third_party = FakeHttp::default().with_body(NPM_URL, NPM_MANIFEST);
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let context = Context::new(&fs, &env)
            .with_http(&regular)
            .with_third_party_http(&third_party, &third_party);
        let version: Version = "v10.9.0".parse().unwrap();
        assert!(fetch_manifest(&context, Tool::Npm, &version).is_ok());
        assert_eq!(third_party.requests(), [NPM_URL]);
        assert!(regular.requests().is_empty());
    }

    #[test]
    fn a_failed_request_names_the_url() {
        let http = FakeHttp::default().with_status("https://registry.npmjs.org/npm/1.0.0", 404);
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let context = Context::new(&fs, &env).with_third_party_http(&http, &http);
        let version: Version = "v1.0.0".parse().unwrap();
        let error = fetch_manifest(&context, Tool::Npm, &version).unwrap_err();
        assert_eq!(
            error.to_string(),
            "https://registry.npmjs.org/npm/1.0.0: HTTP 404"
        );
    }

    #[test]
    fn the_release_day_comes_from_the_packument() {
        let body = r#"{"time": {"1.22.22": "2024-03-09T21:35:40.868Z"}}"#;
        let http = FakeHttp::default().with_body("https://registry.npmjs.org/yarn", body);
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let context = Context::new(&fs, &env);
        let version: Version = "v1.22.22".parse().unwrap();
        let day = fetch_release_day(&context, &http, Tool::Yarn, &version).unwrap();
        assert_eq!(
            day,
            crate::domain::timestamp::days_from_iso_date("2024-03-09")
        );
    }

    #[test]
    fn a_packument_over_the_text_limit_still_yields_the_release_day() {
        // The real npm packument is over 26 MB, past the text limit.
        let entry = r#"{"time": {"10.9.0": "2024-10-03T16:09:31.000Z"}, "pad": ""#;
        let size = usize::try_from(crate::ports::MAX_TEXT_BYTES).unwrap();
        let body = format!("{entry}{}\"}}", "a".repeat(size));
        let http = FakeHttp::default().with_body("https://registry.npmjs.org/npm", &body);
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let context = Context::new(&fs, &env);
        let version: Version = "v10.9.0".parse().unwrap();
        let day = fetch_release_day(&context, &http, Tool::Npm, &version).unwrap();
        assert_eq!(
            day,
            crate::domain::timestamp::days_from_iso_date("2024-10-03")
        );
    }

    #[test]
    fn a_packument_that_is_not_utf8_is_a_fetch_error() {
        let url = "https://registry.npmjs.org/npm";
        let http = FakeHttp::default().with_bytes(url, &[0xff, 0xfe]);
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let context = Context::new(&fs, &env);
        let version: Version = "v10.9.0".parse().unwrap();
        let error = fetch_release_day(&context, &http, Tool::Npm, &version).unwrap_err();
        assert_eq!(error.location, url);
    }
}

//! The releases `nvm init` can pick from, and which of them the policy allows.

use crate::commands::package_managers::registry::fetch_packument;
use crate::commands::policy::enforce::index_location;
use crate::commands::policy::judge::Judge;
use crate::commands::remote_index;
use crate::context::Context;
use crate::domain::policy::{ReleaseInfo, Violation};
use crate::domain::registry::published_versions;
use crate::domain::tool::Tool;
use crate::domain::version::{Flavor, Version};
use crate::error::PolicyError;

/// A release that exists, with what the policy needs to judge it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub version: Version,
    pub release: ReleaseInfo,
}

/// What the policy says of one requested version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exact {
    Allowed,
    NotPublished,
    NotAllowed(Vec<Violation>),
}

/// The releases of the node index, newest first.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] when the index cannot be had.
pub fn node_candidates(context: &Context<'_>) -> Result<Vec<Candidate>, PolicyError> {
    let unavailable = || PolicyError::ReleaseInfoUnavailable(index_location(context));
    let fetched = remote_index::fetch(context, Flavor::Node).map_err(|_| unavailable())?;
    let releases = fetched.releases.filter(|found| !found.is_empty());
    Ok(releases
        .ok_or_else(unavailable)?
        .into_iter()
        .map(|release| Candidate {
            version: release.version,
            release: ReleaseInfo {
                lts: release.lts,
                published_day: release.published_day,
            },
        })
        .collect())
}

/// The stable releases of the registry package that ships `tool` at
/// `package_of` (yarn `2.0.0` for the newest search, the exact version
/// otherwise), newest first.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] when the registry cannot be read or
/// its packument is unusable.
pub fn tool_candidates(
    context: &Context<'_>,
    tool: Tool,
    package_of: &Version,
) -> Result<Vec<Candidate>, PolicyError> {
    let text = fetch_packument(context, tool, package_of)
        .map_err(|error| PolicyError::ReleaseInfoUnavailable(error.location))?;
    let published = published_versions(&text)
        .map_err(|error| PolicyError::ReleaseInfoUnavailable(error.to_string()))?;
    Ok(published
        .into_iter()
        .rev()
        .map(|(version, published_day)| Candidate {
            version,
            release: ReleaseInfo {
                lts: None,
                published_day,
            },
        })
        .collect())
}

/// The first candidate with no violation; for node, among those with an LTS
/// name only.
///
/// # Errors
/// [`PolicyError`] when the policy cannot judge a candidate.
pub fn newest_allowed(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    candidates: &[Candidate],
) -> Result<Option<Version>, PolicyError> {
    for candidate in candidates {
        if tool == Tool::Node && candidate.release.lts.is_none() {
            continue;
        }
        let found = judge.violations(context, tool, &candidate.version, &candidate.release)?;
        if found.is_empty() {
            return Ok(Some(candidate.version));
        }
    }
    Ok(None)
}

/// Whether `version` is published and allowed.
///
/// # Errors
/// [`PolicyError`] when the policy cannot judge the version.
pub fn exact(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    candidates: &[Candidate],
    version: &Version,
) -> Result<Exact, PolicyError> {
    let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.version == *version)
    else {
        return Ok(Exact::NotPublished);
    };
    let found = judge.violations(context, tool, version, &candidate.release)?;
    Ok(if found.is_empty() {
        Exact::Allowed
    } else {
        Exact::NotAllowed(found)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::transcript::Transcript;
    use crate::domain::fixtures::index_text_dated;
    use crate::fakes::{FakeClock, FakeEnv, FakeFileSystem, FakeHttp};

    const INDEX_URL: &str = "https://nodejs.org/dist/index.tab";
    /// 2024-06-01.
    const NOW: i64 = 19_875 * 86_400;

    fn version(text: &str) -> Version {
        format!("v{text}").parse().unwrap()
    }

    fn node_index() -> String {
        index_text_dated(&[
            ("v23.0.0", "2024-05-01", "-"),
            ("v22.3.0", "2024-05-30", "Jod"),
            ("v22.2.0", "2024-05-01", "Jod"),
            ("v22.1.0", "2024-04-01", "Jod"),
            ("v20.0.0", "2023-04-01", "Iron"),
        ])
    }

    fn policy_files(policy: &str) -> (FakeFileSystem, FakeEnv) {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        if policy.is_empty() {
            return (FakeFileSystem::default(), env);
        }
        let fs = FakeFileSystem::default().with_file("/p.json", policy);
        (fs, env.with_var("NVMRC_POLICY", "/p.json"))
    }

    fn judged(context: &Context<'_>) -> Judge {
        Judge::load(context, &mut Transcript::default()).unwrap()
    }

    fn registry_context<'a>(
        fs: &'a FakeFileSystem,
        env: &'a FakeEnv,
        http: &'a FakeHttp,
    ) -> Context<'a> {
        Context::new(fs, env)
            .with_http(http)
            .with_third_party_http(http, http)
    }

    const NPM_PACKUMENT: &str = r#"{
        "dist-tags": {"latest": "10.9.0"},
        "versions": {"10.0.0-rc.1": {}, "10.8.0": {}, "10.9.0": {}},
        "time": {"10.8.0": "2024-05-01T00:00:00.000Z", "10.9.0": "2024-10-03T00:00:00.000Z"}
    }"#;
    const NPM_URL: &str = "https://registry.npmjs.org/npm";
    const DENY_NPM_10_9: &str =
        r#"{ "schemaVersion": 1, "tools": { "npm": { "deny": ["10.9.0"] } } }"#;

    #[test]
    fn the_newest_allowed_lts_skips_denied_and_too_recent_releases() {
        let policy = r#"{ "schemaVersion": 1, "tools": { "node":
            { "deny": ["22.2.0"], "minimumReleaseAgeDays": 7 } } }"#;
        let (fs, env) = policy_files(policy);
        let http = FakeHttp::default()
            .with_body(INDEX_URL, &node_index())
            .with_empty_advisories();
        let clock = FakeClock::at(NOW);
        let context = registry_context(&fs, &env, &http).with_clock(&clock);
        let candidates = node_candidates(&context).unwrap();
        let mut judge = judged(&context);
        let newest = newest_allowed(&context, &mut judge, Tool::Node, &candidates).unwrap();
        assert_eq!(newest, Some(version("22.1.0")));
    }

    #[test]
    fn newest_node_without_a_policy_is_the_newest_lts() {
        let (fs, env) = policy_files("");
        let http = FakeHttp::default().with_body(INDEX_URL, &node_index());
        let context = Context::new(&fs, &env).with_http(&http);
        let candidates = node_candidates(&context).unwrap();
        let mut judge = judged(&context);
        let newest = newest_allowed(&context, &mut judge, Tool::Node, &candidates).unwrap();
        assert_eq!(newest, Some(version("22.3.0")));
    }

    #[test]
    fn a_denied_newest_package_manager_falls_back_to_the_previous_one() {
        let (fs, env) = policy_files(DENY_NPM_10_9);
        let http = FakeHttp::default().with_body(NPM_URL, NPM_PACKUMENT);
        let context = registry_context(&fs, &env, &http);
        let candidates = tool_candidates(&context, Tool::Npm, &version("10.9.0")).unwrap();
        let mut judge = judged(&context);
        let newest = newest_allowed(&context, &mut judge, Tool::Npm, &candidates).unwrap();
        assert_eq!(newest, Some(version("10.8.0")));
    }

    #[test]
    fn yarn_candidates_come_from_cli_dist() {
        let (fs, env) = policy_files("");
        let url = "https://registry.npmjs.org/@yarnpkg%2fcli-dist";
        let http = FakeHttp::default().with_body(url, r#"{"versions": {"4.5.0": {}}}"#);
        let context = registry_context(&fs, &env, &http);
        let candidates = tool_candidates(&context, Tool::Yarn, &version("2.0.0")).unwrap();
        assert_eq!(http.requests(), [url]);
        assert_eq!(candidates[0].version, version("4.5.0"));
    }

    #[test]
    fn an_exact_yarn_one_reads_the_yarn_package() {
        let (fs, env) = policy_files("");
        let url = "https://registry.npmjs.org/yarn";
        let http = FakeHttp::default().with_body(url, r#"{"versions": {"1.22.22": {}}}"#);
        let context = registry_context(&fs, &env, &http);
        let wanted = version("1.22.22");
        let candidates = tool_candidates(&context, Tool::Yarn, &wanted).unwrap();
        let mut judge = judged(&context);
        let verdict = exact(&context, &mut judge, Tool::Yarn, &candidates, &wanted).unwrap();
        assert_eq!(http.requests(), [url]);
        assert_eq!(verdict, Exact::Allowed);
    }

    #[test]
    fn an_unpublished_exact_version_is_not_published() {
        let (fs, env) = policy_files("");
        let http = FakeHttp::default().with_body(NPM_URL, NPM_PACKUMENT);
        let context = registry_context(&fs, &env, &http);
        let wanted = version("9.9.9");
        let candidates = tool_candidates(&context, Tool::Npm, &wanted).unwrap();
        let mut judge = judged(&context);
        let verdict = exact(&context, &mut judge, Tool::Npm, &candidates, &wanted).unwrap();
        assert_eq!(verdict, Exact::NotPublished);
    }

    #[test]
    fn a_denied_exact_version_is_not_allowed() {
        let (fs, env) = policy_files(DENY_NPM_10_9);
        let http = FakeHttp::default().with_body(NPM_URL, NPM_PACKUMENT);
        let context = registry_context(&fs, &env, &http);
        let wanted = version("10.9.0");
        let candidates = tool_candidates(&context, Tool::Npm, &wanted).unwrap();
        let mut judge = judged(&context);
        let verdict = exact(&context, &mut judge, Tool::Npm, &candidates, &wanted).unwrap();
        assert!(matches!(verdict, Exact::NotAllowed(found) if !found.is_empty()));
    }

    #[test]
    fn an_unreachable_registry_is_a_failure_to_know() {
        let (fs, env) = policy_files("");
        let http = FakeHttp::default().with_status(NPM_URL, 503);
        let context = registry_context(&fs, &env, &http);
        let error = tool_candidates(&context, Tool::Npm, &version("10.9.0")).unwrap_err();
        assert_eq!(
            error,
            PolicyError::ReleaseInfoUnavailable(NPM_URL.to_owned())
        );
    }

    #[test]
    fn an_unreachable_index_is_a_failure_to_know() {
        let (fs, env) = policy_files("");
        let http = FakeHttp::default().with_status(INDEX_URL, 503);
        let context = Context::new(&fs, &env).with_http(&http);
        let error = node_candidates(&context).unwrap_err();
        assert_eq!(
            error,
            PolicyError::ReleaseInfoUnavailable(INDEX_URL.to_owned())
        );
    }
}

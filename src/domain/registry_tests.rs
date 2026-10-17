use super::*;

fn version(text: &str) -> Version {
    format!("v{text}").parse().unwrap()
}

const NPM_MANIFEST: &str = r#"{
  "name": "npm", "version": "10.9.0", "description": "ignored",
  "bin": {"npm": "bin/npm-cli.js", "npx": "bin/npx-cli.js"},
  "dist": {
    "tarball": "https://registry.npmjs.org/npm/-/npm-10.9.0.tgz",
    "integrity": "sha512-abc==", "shasum": "0123"
  }
}"#;

fn manifest_with_bin(bin: &str) -> String {
    format!(r#"{{"bin": {bin}, "dist": {{"tarball": "t", "integrity": "i"}}}}"#)
}

#[test]
fn yarn_one_and_berry_come_from_different_packages() {
    assert_eq!(package_name(Tool::Yarn, &version("1.22.22")), "yarn");
    let berry = "@yarnpkg/cli-dist";
    assert_eq!(package_name(Tool::Yarn, &version("2.0.0")), berry);
    assert_eq!(package_name(Tool::Yarn, &version("4.5.0")), berry);
    assert_eq!(package_name(Tool::Npm, &version("10.9.0")), "npm");
    assert_eq!(package_name(Tool::Pnpm, &version("9.0.0")), "pnpm");
}

#[test]
fn scoped_names_are_encoded_in_urls() {
    let yarn = version("4.5.0");
    assert_eq!(
        manifest_url("https://r", Tool::Yarn, &yarn),
        "https://r/@yarnpkg%2fcli-dist/4.5.0"
    );
    assert_eq!(
        packument_url("https://r", Tool::Yarn, &yarn),
        "https://r/@yarnpkg%2fcli-dist"
    );
    assert_eq!(
        manifest_url("https://r", Tool::Npm, &version("10.9.0")),
        "https://r/npm/10.9.0"
    );
}

#[test]
fn parses_the_npm_manifest() {
    let manifest = parse_manifest(NPM_MANIFEST).unwrap();
    assert_eq!(
        manifest.tarball,
        "https://registry.npmjs.org/npm/-/npm-10.9.0.tgz"
    );
    assert_eq!(manifest.integrity, "sha512-abc==");
    let commands: Vec<_> = manifest
        .commands
        .iter()
        .map(|(name, target)| (name.as_str(), target.as_str()))
        .collect();
    assert_eq!(
        commands,
        [("npm", "bin/npm-cli.js"), ("npx", "bin/npx-cli.js")]
    );
}

#[test]
fn a_bin_path_outside_the_package_is_refused() {
    for bin in [
        r#"{"a": "../../evil"}"#,
        r#"{"a": "/etc/x"}"#,
        r#"{"a": ""}"#,
        r#"{"a": 1}"#,
    ] {
        let error = parse_manifest(&manifest_with_bin(bin)).unwrap_err();
        assert!(matches!(error, RegistryError::BadTarget { .. }), "{bin}");
    }
    let error = parse_manifest(&manifest_with_bin(r#"{"a/b": "bin/x"}"#)).unwrap_err();
    assert_eq!(error, RegistryError::BadCommand("a/b".to_owned()));
    let error = parse_manifest(&manifest_with_bin(r#"{"": "bin/x"}"#)).unwrap_err();
    assert_eq!(error, RegistryError::BadCommand(String::new()));
}

#[test]
fn a_missing_bin_is_refused() {
    for bin in ["{}", r#""bin/x""#, "null"] {
        let error = parse_manifest(&manifest_with_bin(bin)).unwrap_err();
        assert_eq!(error, RegistryError::NoCommands, "{bin}");
    }
    let no_bin = r#"{"dist": {"tarball": "t", "integrity": "i"}}"#;
    assert_eq!(parse_manifest(no_bin), Err(RegistryError::NoCommands));
}

#[test]
fn a_manifest_without_dist_or_json_is_refused() {
    assert!(matches!(
        parse_manifest("nope"),
        Err(RegistryError::Json(_))
    ));
    let no_dist = r#"{"bin": {"a": "b"}}"#;
    assert_eq!(
        parse_manifest(no_dist),
        Err(RegistryError::Missing("tarball"))
    );
}

#[test]
fn release_day_reads_the_time_entry() {
    let packument = r#"{"time": {"created": "2010-01-01T00:00:00.000Z",
        "1.22.22": "2024-03-09T21:35:40.868Z"}}"#;
    let expected = days_from_iso_date("2024-03-09");
    assert!(expected.is_some());
    assert_eq!(release_day(packument, &version("1.22.22")), Ok(expected));
    assert_eq!(release_day(packument, &version("1.22.23")), Ok(None));
    let bad = r#"{"time": {"1.0.0": "soon"}}"#;
    assert!(matches!(
        release_day(bad, &version("1.0.0")),
        Err(RegistryError::BadDate(_))
    ));
}

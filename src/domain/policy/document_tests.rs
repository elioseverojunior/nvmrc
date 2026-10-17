use super::*;

const SPEC_EXAMPLE: &str = r#"{
  "$schema": "https://raw.githubusercontent.com/elioseverojunior/nvmrc/main/schemas/nvmrc-policy.v1.schema.json",
  "schemaVersion": 1,
  "tools": {
    "node": {
      "minimum": "20.11.1",
      "allow": ">=20 <25",
      "deny": ["20.12.0", "22.3.0"],
      "ltsOnly": true,
      "minimumReleaseAgeDays": 14
    },
    "npm": { "minimum": "10.2.4" },
    "pnpm": { "minimum": "9.1.0", "minimumReleaseAgeDays": 7 },
    "yarn": { "allow": ">=4.1.0 <5" }
  },
  "cve": {
    "failOnSeverity": "high",
    "source": "https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json"
  }
}"#;

#[test]
fn the_spec_example_parses() {
    let document = parse_document(SPEC_EXAMPLE).unwrap();
    let node = document.tools.node.unwrap();
    assert_eq!(node.minimum.as_deref(), Some("20.11.1"));
    assert_eq!(node.allow.as_deref(), Some(">=20 <25"));
    assert_eq!(node.deny, Some(vec!["20.12.0".into(), "22.3.0".into()]));
    assert_eq!(node.lts_only, Some(true));
    assert_eq!(node.minimum_release_age_days, Some(14));
    assert_eq!(document.cve.unwrap().fail_on_severity, Some(Severity::High));
}

#[test]
fn an_unknown_field_is_rejected_at_any_level() {
    for text in [
        r#"{"schemaVersion":1,"extra":true}"#,
        r#"{"schemaVersion":1,"tools":{"deno":{}}}"#,
        r#"{"schemaVersion":1,"tools":{"node":{"minimun":"20"}}}"#,
        r#"{"schemaVersion":1,"cve":{"failOn":"high"}}"#,
    ] {
        assert!(
            matches!(parse_document(text), Err(DocumentError::Malformed(_))),
            "{text}"
        );
    }
}

#[test]
fn lts_only_is_rejected_outside_node() {
    let text = r#"{"schemaVersion":1,"tools":{"pnpm":{"ltsOnly":true}}}"#;
    assert!(matches!(
        parse_document(text),
        Err(DocumentError::Malformed(_))
    ));
}

#[test]
fn only_schema_version_1_is_understood() {
    assert_eq!(
        parse_document(r#"{"schemaVersion":2}"#),
        Err(DocumentError::UnsupportedSchemaVersion(2))
    );
    assert!(matches!(
        parse_document("{}"),
        Err(DocumentError::Malformed(_))
    ));
}

#[test]
fn versions_and_ranges_are_validated() {
    let bad_minimum = r#"{"schemaVersion":1,"tools":{"node":{"minimum":"twenty"}}}"#;
    let bad_deny = r#"{"schemaVersion":1,"tools":{"node":{"deny":["20"]}}}"#; // deny is exact only
    let bad_range = r#"{"schemaVersion":1,"tools":{"node":{"allow":">>20"}}}"#;
    assert!(matches!(
        parse_document(bad_minimum),
        Err(DocumentError::InvalidVersion { .. })
    ));
    assert!(matches!(
        parse_document(bad_deny),
        Err(DocumentError::InvalidVersion { .. })
    ));
    assert!(matches!(
        parse_document(bad_range),
        Err(DocumentError::InvalidRange { .. })
    ));
}

#[test]
fn a_blank_allow_is_rejected_rather_than_allowing_everything() {
    for allow in ["", "  "] {
        let text = format!(r#"{{"schemaVersion":1,"tools":{{"node":{{"allow":"{allow}"}}}}}}"#);
        assert_eq!(
            parse_document(&text),
            Err(DocumentError::InvalidRange {
                field: "tools.node.allow".into(),
                value: allow.into(),
            })
        );
    }
}

#[test]
fn an_iojs_minimum_or_deny_entry_is_rejected() {
    let minimum = r#"{"schemaVersion":1,"tools":{"node":{"minimum":"iojs-v3"}}}"#;
    let deny = r#"{"schemaVersion":1,"tools":{"node":{"deny":["iojs-v3.0.0"]}}}"#;
    assert_eq!(
        parse_document(minimum),
        Err(DocumentError::InvalidVersion {
            field: "tools.node.minimum".into(),
            value: "iojs-v3".into(),
        })
    );
    assert_eq!(
        parse_document(deny),
        Err(DocumentError::InvalidVersion {
            field: "tools.node.deny[0]".into(),
            value: "iojs-v3.0.0".into(),
        })
    );
}

#[test]
fn invalid_fields_are_named_by_their_json_path() {
    let text = r#"{"schemaVersion":1,"tools":{"yarn":{"deny":["4.0.0","4"]}}}"#;
    assert_eq!(
        parse_document(text),
        Err(DocumentError::InvalidVersion {
            field: "tools.yarn.deny[1]".into(),
            value: "4".into(),
        })
    );
}

#[test]
fn malformed_json_reports_line_and_column() {
    let error = parse_document("{\n  \"schemaVersion\": 1,\n}").unwrap_err();
    assert!(error.to_string().contains("line 3"), "{error}");
}

#[test]
fn committed_schema_matches_the_types() {
    let generated = serde_json::to_string_pretty(&schemars::schema_for!(PolicyDocument)).unwrap();
    let committed = include_str!("../../../schemas/nvmrc-policy.v1.schema.json");
    assert_eq!(
        committed.trim_end(),
        generated,
        "regenerate schemas/nvmrc-policy.v1.schema.json"
    );
}

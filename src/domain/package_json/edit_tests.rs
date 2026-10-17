use crate::domain::package_json::{PackageManagerError, set_package_manager};

fn set(text: &str) -> Result<String, PackageManagerError> {
    set_package_manager(text, "pnpm@9.12.0")
}

#[test]
fn replaces_the_package_manager_value() {
    let text = "{\n  \"name\": \"x\",\n  \"packageManager\" :  \"yarn@1.22.22\" ,\n  \"v\": 1\n}\n";
    assert_eq!(
        set(text).unwrap(),
        "{\n  \"name\": \"x\",\n  \"packageManager\" :  \"pnpm@9.12.0\" ,\n  \"v\": 1\n}\n"
    );
}

#[test]
fn appends_the_field_with_the_file_indentation() {
    assert_eq!(
        set("{\n  \"name\": \"x\"\n}\n").unwrap(),
        "{\n  \"name\": \"x\",\n  \"packageManager\": \"pnpm@9.12.0\"\n}\n"
    );
    assert_eq!(
        set("{\n\t\"name\": \"x\",\n\t\"a\": {\"b\": [1, 2]}\n}").unwrap(),
        "{\n\t\"name\": \"x\",\n\t\"a\": {\"b\": [1, 2]},\n\t\"packageManager\": \"pnpm@9.12.0\"\n}"
    );
}

#[test]
fn an_empty_object_gets_the_field() {
    for text in ["{}", "{ }", "{\n}"] {
        assert_eq!(
            set(text).unwrap(),
            "{\n  \"packageManager\": \"pnpm@9.12.0\"\n}",
            "{text:?}"
        );
    }
}

#[test]
fn only_the_top_level_package_manager_changes() {
    let text = "{\n  \"devEngines\": {\"packageManager\": \"x\"},\n  \"a\": 1\n}";
    assert_eq!(
        set(text).unwrap(),
        "{\n  \"devEngines\": {\"packageManager\": \"x\"},\n  \"a\": 1,\n  \"packageManager\": \"pnpm@9.12.0\"\n}"
    );
}

#[test]
fn escaped_quotes_in_strings_do_not_confuse_the_scan() {
    let text =
        "{\n  \"a\": \"q\\\" , \\\"packageManager\\\": 1 }\",\n  \"packageManager\": \"x\"\n}";
    assert_eq!(
        set(text).unwrap(),
        "{\n  \"a\": \"q\\\" , \\\"packageManager\\\": 1 }\",\n  \"packageManager\": \"pnpm@9.12.0\"\n}"
    );
}

#[test]
fn a_top_level_array_is_malformed() {
    for text in ["[]", "\"x\"", "{not json", ""] {
        assert!(
            matches!(set(text), Err(PackageManagerError::Malformed(_))),
            "{text:?}"
        );
    }
}

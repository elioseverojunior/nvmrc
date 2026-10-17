use super::super::test_support::{file, init, project, served};

#[test]
fn a_matching_package_manager_keeps_its_corepack_hash() {
    let package_json = "{ \"packageManager\": \"pnpm@9.12.0+sha512.abc\" }";
    let (fs, env) = project(&[("/p/package.json", package_json)], "");
    let output = init(&fs, &env, &served(), &["--pnpm=9.12.0"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "Wrote /p/.nvmrc: pnpm 9.12.0");
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some(package_json));
}

#[test]
fn a_replaced_package_manager_names_the_old_value() {
    let (fs, env) = project(
        &[("/p/package.json", "{ \"packageManager\": \"pnpm@9.1.0\" }")],
        "",
    );
    let output = init(&fs, &env, &served(), &["--pnpm"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(
        output.stdout,
        "Wrote /p/.nvmrc: pnpm 9.12.0\n\
         Set packageManager to \"pnpm@9.12.0\" in /p/package.json (was \"pnpm@9.1.0\")"
    );
    assert_eq!(
        file(&fs, "/p/package.json").as_deref(),
        Some("{ \"packageManager\": \"pnpm@9.12.0\" }")
    );
}

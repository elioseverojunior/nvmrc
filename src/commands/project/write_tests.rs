use super::*;
use crate::commands::init::test_support::{file, init, project, served};
use crate::commands::transcript::Transcript;
use crate::error::NvmExitCode;

const YARN_URL: &str = "https://registry.npmjs.org/yarn";

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

#[test]
fn an_exact_yarn_one_is_pinned_from_the_yarn_package() {
    let (fs, env) = project(&[("/p/package.json", "{}")], "");
    let http = served().with_body(YARN_URL, r#"{"versions": {"1.22.22": {}}}"#);
    let output = init(&fs, &env, &http, &["--yarn=1.22.22"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert!(http.requests().iter().any(|url| url == YARN_URL));
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some("yarn=1.22.22\n"));
    assert_eq!(
        file(&fs, "/p/package.json").as_deref(),
        Some("{\n  \"packageManager\": \"yarn@1.22.22\"\n}")
    );
    assert_eq!(
        output.stdout,
        "Wrote /p/.nvmrc: yarn 1.22.22\n\
         Set packageManager to \"yarn@1.22.22\" in /p/package.json"
    );
}

#[test]
fn node_alone_leaves_package_json_untouched() {
    let package_json = "{ \"packageManager\": \"pnpm@9.1.0\" }\n";
    let (fs, env) = project(&[("/p/package.json", package_json)], "");
    let output = init(&fs, &env, &served(), &["--node"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "Wrote /p/.nvmrc: node v22.11.0");
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some(package_json));
}

fn denied(path: &str) -> String {
    let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    format!("{path}: {error}")
}

#[test]
fn a_failed_nvmrc_write_restores_package_json() {
    let package_json = "{\n  \"name\": \"app\"\n}\n";
    let (fs, env) = project(&[("/p/package.json", package_json)], "");
    let fs = fs.with_write_failure("/p/.nvmrc", 0);
    let output = init(&fs, &env, &served(), &["--pnpm"]);
    assert_eq!(output.status.code(), 1);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, denied("/p/.nvmrc"));
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some(package_json));
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
}

#[test]
fn a_failed_restore_says_package_json_was_updated() {
    let (fs, env) = project(&[("/p/package.json", "{}")], "");
    let fs = fs
        .with_write_failure("/p/.nvmrc", 0)
        .with_write_failure("/p/package.json", 1);
    let output = init(&fs, &env, &served(), &["--pnpm"]);
    assert_eq!(output.status.code(), 1);
    assert_eq!(
        output.stderr,
        format!(
            "{}\nnvm init: /p/package.json was already updated to \"pnpm@9.12.0\".",
            denied("/p/.nvmrc")
        )
    );
}

#[test]
fn beside_puts_package_json_next_to_the_nvmrc() {
    let files = ProjectFiles::beside(Path::new("/a/b/.nvmrc"));
    assert_eq!(files.nvmrc, Path::new("/a/b/.nvmrc"));
    assert_eq!(files.package_json, Path::new("/a/b/package.json"));
}

#[test]
fn the_restore_failure_line_names_the_command() {
    let (fs, env) = project(&[("/p/package.json", "{}")], "");
    let fs = fs
        .with_write_failure("/p/.nvmrc", 0)
        .with_write_failure("/p/package.json", 1);
    let context = Context::new(&fs, &env);
    let files = ProjectFiles::in_directory(Path::new("/p"));
    let chosen = [(Tool::Pnpm, "v9.12.0".parse().unwrap())];
    let edit = new_package_json(&context, &files.package_json, &chosen).unwrap();
    let mut transcript = Transcript::default();
    let saved = save(
        &context,
        "nvm upgrade",
        &files,
        "",
        edit.as_ref(),
        &mut transcript,
    );
    assert!(saved.is_err());
    let stderr = transcript.finish(NvmExitCode::Success).stderr;
    let line = "nvm upgrade: /p/package.json was already updated to \"pnpm@9.12.0\".";
    assert!(stderr.ends_with(line), "{stderr}");
}

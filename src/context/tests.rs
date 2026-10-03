use super::*;
use crate::domain::platform::Platform;
use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper};

fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
    let fs = FakeFileSystem::default();
    Context::new(&fs, env).nvm_dir()
}

fn installed(fs: &FakeFileSystem) -> Vec<String> {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(fs, &env);
    let mut found: Vec<String> = context
        .installed_versions()
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    found.sort();
    found
}

#[test]
fn nvm_dir_removes_the_trailing_slash() {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n/");
    assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/n"));
}

#[test]
fn nvm_dir_keeps_the_root_directory() {
    let env = FakeEnv::default().with_var("NVM_DIR", "/");
    assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/"));
}

#[test]
fn nvm_dir_defaults_to_home_dot_nvm() {
    let env = FakeEnv::default().with_var("HOME", "/home/me");
    assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/home/me/.nvm"));
}

#[test]
fn an_empty_nvm_dir_falls_back_to_home() {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "")
        .with_var("HOME", "/home/me");
    assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/home/me/.nvm"));
}

#[test]
fn nvm_dir_is_an_error_without_nvm_dir_and_home() {
    let result = nvm_dir_for(&FakeEnv::default());
    assert!(matches!(result, Err(CliError::NvmDirUnresolved)));
}

#[cfg(unix)]
#[test]
fn a_non_utf8_nvm_dir_is_honoured_not_ignored() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let raw = OsString::from_vec(b"/n\xff".to_vec());
    let env = FakeEnv::default()
        .with_var_os("NVM_DIR", raw.clone())
        .with_var("HOME", "/home/me");
    assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from(raw));
}

#[test]
fn a_context_cannot_reach_the_network_until_given_an_http() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default();
    assert!(
        Context::new(&fs, &env)
            .http()
            .get_text("http://x/")
            .is_err()
    );
    let http = FakeHttp::default().with_body("http://x/", "ok");
    let context = Context::new(&fs, &env).with_http(&http);
    assert_eq!(context.http().get_text("http://x/").unwrap(), "ok");
}

#[test]
fn a_context_hashes_nothing_until_it_is_given_a_digest() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default();
    assert!(
        Context::new(&fs, &env)
            .digest()
            .sha256_file(Path::new("/f"))
            .is_err()
    );
    let digest = FakeDigest::default().with_digest("/f", "abc");
    let context = Context::new(&fs, &env).with_digest(&digest);
    assert_eq!(
        context.digest().sha256_file(Path::new("/f")).unwrap(),
        "abc"
    );
}

#[test]
fn a_context_unpacks_nothing_until_it_is_given_an_archive() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default();
    let (a, b) = (Path::new("/a.tgz"), Path::new("/out"));
    assert!(Context::new(&fs, &env).archive().extract(a, b).is_err());
    let archive = FakeArchive::new(&fs).with_archive("/a.tgz", &[("t/f", "x", false)]);
    let context = Context::new(&fs, &env).with_archive(&archive);
    context.archive().extract(a, b).unwrap();
    assert!(fs.is_file(Path::new("/out/t/f")));
}

#[test]
fn a_context_uses_the_sleeper_it_is_given() {
    let (fs, env, sleeper) = (
        FakeFileSystem::default(),
        FakeEnv::default(),
        FakeSleeper::default(),
    );
    let context = Context::new(&fs, &env).with_sleeper(&sleeper);
    context.sleeper().sleep(std::time::Duration::from_secs(1));
    assert_eq!(sleeper.slept().len(), 1);
}

#[test]
fn a_context_is_linux_x64_until_told_otherwise() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let context = Context::new(&fs, &env);
    assert_eq!(context.platform().map(|p| p.arch.as_str()), Some("x64"));
    let mac = Platform::from_host("macos", "aarch64", false);
    let context = context.with_platform(mac.clone());
    assert_eq!(context.platform(), mac.as_ref());
    assert_eq!(context.with_platform(None).platform(), None);
}

#[test]
fn alias_dir_is_under_nvm_dir() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(&fs, &env);
    assert_eq!(context.alias_dir().unwrap(), PathBuf::from("/n/alias"));
}

#[test]
fn alias_store_reads_the_alias_directory() {
    let fs = FakeFileSystem::default().with_file("/n/alias/default", "v20");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(&fs, &env);
    let store = context.alias_store().unwrap();
    assert_eq!(store.target("default"), Some("v20".to_owned()));
}

#[test]
fn installed_versions_reads_node_and_iojs_directories() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.0.0/bin/iojs", "");
    assert_eq!(installed(&fs), ["iojs-v3.0.0", "v20.1.0"]);
}

#[test]
fn installed_versions_ignores_non_canonical_names_and_files() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/node/20.2.0/bin/node", "")
        .with_file("/n/versions/node/v020.3.0/bin/node", "")
        .with_file("/n/versions/node/not-a-version/x", "")
        .with_file("/n/versions/node/v22.0.0", "a plain file");
    assert_eq!(installed(&fs), ["v20.1.0"]);
}

#[test]
fn an_empty_version_directory_still_counts_as_installed() {
    let fs = FakeFileSystem::default().with_dir("/n/versions/node/v18.0.0");
    assert_eq!(installed(&fs), ["v18.0.0"]);
}

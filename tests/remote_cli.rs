//! End-to-end: the real binary against a real temporary `$NVM_DIR` and a mirror
//! served on a local port.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;

const NODE_INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
v20.10.0\t2023-11-22\tlinux-x64\t10.2.3\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v20.9.0\t2023-10-24\tlinux-x64\t10.1.0\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v18.19.0\t2023-11-29\tlinux-x64\t10.2.3\t10.2\t1.44\t1.3\t3.0\t108\tHydrogen\t-\n\
v4.0.0\t2015-09-08\tlinux-x64\t2.14.2\t4.5\t1.7\t1.2\t1.0\t46\t-\t-\n";

/// Serves `index.tab` to every request, and 404 to the rest, until the test
/// process ends. Returns the mirror URL.
fn serve_mirror(index: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 1024];
            let read = stream.read(&mut request).unwrap_or(0);
            let found = String::from_utf8_lossy(&request[..read]).contains("GET /index.tab");
            let (status, body) = if found {
                ("200 OK", index)
            } else {
                ("404 Not Found", "")
            };
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// An io.js mirror that offers nothing, so only Node releases are listed.
const IOJS_INDEX: &str =
    "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n";

fn nvm(nvm_dir: &Path, mirror: &str, args: &[&str]) -> Output {
    let iojs_mirror = serve_mirror(IOJS_INDEX);
    Command::new(env!("CARGO_BIN_EXE_nvm"))
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PATH", "/nonexistent")
        .env("NVM_NODEJS_ORG_MIRROR", mirror)
        .env("NVM_IOJS_ORG_MIRROR", iojs_mirror)
        .output()
        .expect("run the binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn ls_remote_lts_lists_the_lts_releases_of_the_mirror() {
    let (home, mirror) = (tempfile::tempdir().unwrap(), serve_mirror(NODE_INDEX));
    let output = nvm(home.path(), &mirror, &["ls-remote", "--lts"]);
    let expected = "       v18.19.0   (Latest LTS: Hydrogen)\n        v20.9.0   (LTS: Iron)\n       v20.10.0   (Latest LTS: Iron)\n";
    assert_eq!(stdout(&output), expected);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn ls_remote_refreshes_the_lts_aliases_on_disk() {
    let (home, mirror) = (tempfile::tempdir().unwrap(), serve_mirror(NODE_INDEX));
    nvm(home.path(), &mirror, &["ls-remote"]);
    let alias = |name: &str| fs::read_to_string(home.path().join("alias/lts").join(name));
    assert_eq!(alias("*").unwrap(), "lts/iron\n");
    assert_eq!(alias("hydrogen").unwrap(), "v18.19.0\n");
    let listed = nvm(home.path(), &mirror, &["alias", "lts/iron"]);
    assert_eq!(stdout(&listed), "v20.10.0\n");
}

#[test]
fn version_remote_prints_one_release_or_n_a() {
    let (home, mirror) = (tempfile::tempdir().unwrap(), serve_mirror(NODE_INDEX));
    let found = nvm(home.path(), &mirror, &["version-remote", "18"]);
    assert_eq!(
        (stdout(&found).as_str(), found.status.code()),
        ("v18.19.0\n", Some(0))
    );
    let missing = nvm(home.path(), &mirror, &["version-remote", "99"]);
    assert_eq!(
        (stdout(&missing).as_str(), missing.status.code()),
        ("N/A\n", Some(3))
    );
}

#[test]
fn an_unreachable_mirror_is_n_a_with_status_3() {
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), "http://127.0.0.1:1", &["ls-remote"]);
    assert_eq!(stdout(&output), "            N/A\n");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn cache_clear_empties_the_cache_and_keeps_the_rest() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".cache/bin")).unwrap();
    fs::write(home.path().join(".cache/bin/file.tar.gz"), "x").unwrap();
    fs::create_dir_all(home.path().join("alias")).unwrap();
    fs::write(home.path().join("alias/default"), "node").unwrap();
    let output = nvm(home.path(), "http://127.0.0.1:1", &["cache", "clear"]);
    assert_eq!(stdout(&output), "nvm cache cleared.\n");
    assert_eq!(fs::read_dir(home.path().join(".cache")).unwrap().count(), 0);
    assert!(home.path().join("alias/default").is_file());
    let dir = nvm(home.path(), "http://127.0.0.1:1", &["cache", "dir"]);
    assert_eq!(stdout(&dir), format!("{}/.cache\n", home.path().display()));
}

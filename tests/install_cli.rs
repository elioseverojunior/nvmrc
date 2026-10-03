//! End-to-end: `install` and `uninstall` with the real binary, a real
//! temporary `$NVM_DIR` and a mirror served on a local port that holds a real
//! `.tar.gz`.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;

use flate2::Compression;
use flate2::write::GzEncoder;
use nvmrc::domain::platform::Platform;
use sha2::{Digest, Sha256};

const NODE_INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
v20.10.0\t2023-11-22\tx\t10.2.3\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v18.19.0\t2023-11-29\tx\t10.2.3\t10.2\t1.44\t1.3\t3.0\t108\tHydrogen\t-\n";
const IOJS_INDEX: &str =
    "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n";
const INHERITED_NETWORK_SETTINGS: [&str; 7] = [
    "ALL_PROXY",
    "all_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "NVM_AUTH_HEADER",
];

/// The archive name nvmrc will ask for on this machine, or `None` where it has
/// no binaries (the tests then have nothing to check).
fn slug(version: &str) -> Option<String> {
    let platform = Platform::from_host(std::env::consts::OS, std::env::consts::ARCH, false)?;
    Some(platform.download_slug(&version.parse().unwrap()))
}

/// `<slug>/bin/node` (executable) and `<slug>/lib/README`, gzip-compressed.
fn tarball(slug: &str, version: &str) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let script = format!("#!/bin/sh\necho {version}\n");
    for (path, contents, mode) in [
        (format!("{slug}/bin/node"), script.as_str(), 0o755),
        (format!("{slug}/lib/README"), "readme", 0o644),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        builder
            .append_data(&mut header, path, contents.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Serves each path with its body and 404 for the rest; returns the URL.
fn serve(files: BTreeMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap_or(0);
            let text = String::from_utf8_lossy(&request[..read]).into_owned();
            let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let (status, body) = match files.get(&path) {
                Some(body) => ("200 OK", body.clone()),
                None => ("404 Not Found", Vec::new()),
            };
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// A mirror with Node 20.10.0 whose checksum is `listed` (or the right one).
fn mirror(listed: Option<&str>) -> Option<String> {
    let slug = slug("v20.10.0")?;
    let archive = tarball(&slug, "v20.10.0");
    let name = format!("{slug}.tar.gz");
    let checksum = listed.map_or_else(|| digest(&archive), str::to_owned);
    let mut files = BTreeMap::new();
    files.insert("/index.tab".to_owned(), NODE_INDEX.as_bytes().to_vec());
    files.insert(
        "/v20.10.0/SHASUMS256.txt".to_owned(),
        format!("{checksum}  {name}\n").into_bytes(),
    );
    files.insert(format!("/v20.10.0/{name}"), archive);
    Some(serve(files))
}

fn nvm(nvm_dir: &Path, mirror: &str, args: &[&str]) -> Output {
    let iojs = serve(BTreeMap::from([(
        "/index.tab".to_owned(),
        IOJS_INDEX.as_bytes().to_vec(),
    )]));
    let mut command = Command::new(env!("CARGO_BIN_EXE_nvm"));
    command
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PATH", "/nonexistent")
        .env("NVM_NODEJS_ORG_MIRROR", mirror)
        .env("NVM_IOJS_ORG_MIRROR", iojs)
        .env("NO_PROXY", "127.0.0.1");
    for name in INHERITED_NETWORK_SETTINGS {
        command.env_remove(name);
    }
    command.output().expect("run the binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn install_puts_a_working_version_in_place_and_ls_sees_it() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "20"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Downloading and installing node v20.10.0...\n\
         Creating default alias: default -> 20 (-> v20.10.0 *)\n"
    );
    assert!(stderr(&output).contains("Checksums matched!"));
    let node = home.path().join("versions/node/v20.10.0/bin/node");
    assert_eq!(
        fs::read_to_string(&node).unwrap(),
        "#!/bin/sh\necho v20.10.0\n"
    );
    let listed = nvm(home.path(), &mirror, &["ls", "--no-alias"]);
    assert_eq!(stdout(&listed), "       v20.10.0 *\n");
    assert!(
        !home
            .path()
            .join(".cache/bin")
            .join(slug("v20.10.0").unwrap())
            .join("files")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn the_installed_node_can_run() {
    use std::os::unix::fs::PermissionsExt;
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    nvm(home.path(), &mirror, &["install", "20"]);
    let node = home.path().join("versions/node/v20.10.0/bin/node");
    assert_ne!(fs::metadata(&node).unwrap().permissions().mode() & 0o111, 0);
    let ran = Command::new(&node).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "v20.10.0\n");
}

#[test]
fn a_second_install_says_so_and_a_cached_archive_is_reused_after_uninstall() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    nvm(home.path(), &mirror, &["install", "20"]);
    let again = nvm(home.path(), &mirror, &["install", "v20.10.0"]);
    assert_eq!(stderr(&again), "v20.10.0 is already installed.\n");
    assert_eq!(stdout(&again), "");
    let removed = nvm(home.path(), &mirror, &["uninstall", "20"]);
    assert_eq!(stdout(&removed), "Uninstalled node v20.10.0\n");
    assert!(!home.path().join("versions/node/v20.10.0").exists());
    let reinstalled = nvm(home.path(), &mirror, &["install", "20"]);
    assert!(stderr(&reinstalled).contains("Checksums match! Using existing downloaded archive"));
    assert!(
        home.path()
            .join("versions/node/v20.10.0/bin/node")
            .is_file()
    );
}

#[test]
fn a_wrong_checksum_installs_nothing_and_exits_2() {
    let Some(mirror) = mirror(Some("0000")) else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "20"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Checksums do not match:"));
    assert!(stderr(&output).ends_with("Binary download failed. Download from source aborted.\n"));
    assert!(!home.path().join("versions").exists());
}

#[test]
fn a_version_that_is_not_on_the_mirror_exits_3() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "99"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        stderr(&output),
        "Version '99' not found - try `nvm ls-remote` to browse available versions.\n"
    );
}

#[test]
fn install_lts_makes_the_default_point_at_lts_star() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "--lts"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("Installing latest LTS version.\n"));
    assert_eq!(
        fs::read_to_string(home.path().join("alias/default")).unwrap(),
        "lts/*\n"
    );
}

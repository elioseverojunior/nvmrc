//! What the end-to-end tests of `install` share: a mirror served from a local
//! socket that holds real archives, and the binary run against it.
//!
//! Every test crate uses some of this, so what one of them leaves out is not
//! dead code.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;

use lzma_rust2::{XzOptions, XzWriter};
use nvmrc::domain::platform::Platform;
use sha2::{Digest, Sha256};

pub const NODE_INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
v20.10.0\t2023-11-22\tx\t10.2.3\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v18.19.0\t2023-11-29\tx\t10.2.3\t10.2\t1.44\t1.3\t3.0\t108\tHydrogen\t-\n";
pub const IOJS_INDEX: &str =
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

/// A file of an archive: its path, contents and permissions.
pub type Entry<'a> = (String, &'a str, u32);

/// The archive name nvmrc will ask for on this machine, or `None` where it has
/// no binaries (the tests then have nothing to check).
pub fn slug(version: &str) -> Option<String> {
    let platform = Platform::from_host(std::env::consts::OS, std::env::consts::ARCH, false)?;
    Some(platform.download_slug(&version.parse().unwrap()))
}

/// An xz-compressed tar of `entries`: what nvmrc asks a mirror for, as
/// `nvm.sh` does, for every version of Node 4 and later.
pub fn tar_xz(entries: &[Entry<'_>]) -> Vec<u8> {
    let writer = XzWriter::new(Vec::new(), XzOptions::with_preset(1)).unwrap();
    let mut builder = tar::Builder::new(writer);
    for (path, contents, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder
            .append_data(&mut header, path, contents.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// `<slug>/bin/node` (executable, prints its version) and `<slug>/lib/README`,
/// plus `extra`, which is under `<slug>/` too.
pub fn tarball_with(slug: &str, version: &str, extra: &[(&str, &str, u32)]) -> Vec<u8> {
    let script = format!("#!/bin/sh\necho {version}\n");
    let mut entries: Vec<Entry<'_>> = vec![
        (format!("{slug}/bin/node"), &script, 0o755),
        (format!("{slug}/lib/README"), "readme", 0o644),
    ];
    entries.extend(
        extra
            .iter()
            .map(|(path, contents, mode)| (format!("{slug}/{path}"), *contents, *mode)),
    );
    tar_xz(&entries)
}

pub fn tarball(slug: &str, version: &str) -> Vec<u8> {
    tarball_with(slug, version, &[])
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Serves each path with its body and 404 for the rest; returns the URL.
pub fn serve(files: BTreeMap<String, Vec<u8>>) -> String {
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

/// A mirror under construction: the index, and per version the archives and
/// their checksums.
pub struct Mirror {
    files: BTreeMap<String, Vec<u8>>,
    sums: BTreeMap<String, String>,
}

impl Mirror {
    pub fn new() -> Self {
        let files = BTreeMap::from([("/index.tab".to_owned(), NODE_INDEX.as_bytes().to_vec())]);
        Self {
            files,
            sums: BTreeMap::new(),
        }
    }

    fn add(&mut self, version: &str, name: &str, bytes: Vec<u8>, listed: Option<&str>) {
        let checksum = listed.map_or_else(|| digest(&bytes), str::to_owned);
        let sums = self.sums.entry(version.to_owned()).or_default();
        sums.push_str(&format!("{checksum}  {name}\n"));
        self.files.insert(format!("/{version}/{name}"), bytes);
    }

    /// The prebuilt binary of `version`, with `extra` files in it; `listed`
    /// replaces the checksum in `SHASUMS256.txt`. `None` on a machine with no
    /// binaries.
    pub fn binary(
        mut self,
        version: &str,
        extra: &[(&str, &str, u32)],
        listed: Option<&str>,
    ) -> Option<Self> {
        let slug = slug(version)?;
        let archive = tarball_with(&slug, version, extra);
        self.add(version, &format!("{slug}.tar.xz"), archive, listed);
        Some(self)
    }

    /// The source archive of `version`, holding `files` under `node-<version>/`.
    pub fn source(mut self, version: &str, files: &[(&str, &str, u32)]) -> Self {
        let top = format!("node-{version}");
        let entries: Vec<Entry<'_>> = files
            .iter()
            .map(|(path, contents, mode)| (format!("{top}/{path}"), *contents, *mode))
            .collect();
        self.add(version, &format!("{top}.tar.xz"), tar_xz(&entries), None);
        self
    }

    pub fn serve(mut self) -> String {
        for (version, text) in std::mem::take(&mut self.sums) {
            self.files
                .insert(format!("/{version}/SHASUMS256.txt"), text.into_bytes());
        }
        serve(self.files)
    }
}

/// A mirror with Node 20.10.0 whose checksum is `listed` (or the right one).
pub fn mirror(listed: Option<&str>) -> Option<String> {
    Some(Mirror::new().binary("v20.10.0", &[], listed)?.serve())
}

/// Runs the binary with the environment `extra` added, and `path` as `PATH`.
pub fn nvm_with(
    nvm_dir: &Path,
    mirror: &str,
    args: &[&str],
    path: &str,
    extra: &[(&str, &str)],
) -> Output {
    let iojs = serve(BTreeMap::from([(
        "/index.tab".to_owned(),
        IOJS_INDEX.as_bytes().to_vec(),
    )]));
    let mut command = Command::new(env!("CARGO_BIN_EXE_nvm"));
    command
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PWD", nvm_dir)
        .env("PATH", path)
        .env("NVM_NODEJS_ORG_MIRROR", mirror)
        .env("NVM_IOJS_ORG_MIRROR", iojs)
        .env("NO_PROXY", "127.0.0.1")
        .current_dir(nvm_dir);
    for name in INHERITED_NETWORK_SETTINGS {
        command.env_remove(name);
    }
    for (name, value) in extra {
        command.env(name, value);
    }
    command.output().expect("run the binary")
}

pub fn nvm(nvm_dir: &Path, mirror: &str, args: &[&str]) -> Output {
    nvm_with(nvm_dir, mirror, args, "/nonexistent", &[])
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

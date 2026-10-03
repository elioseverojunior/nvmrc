//! What the end-to-end tests of the `nvm` function share: a temporary
//! `$NVM_DIR` with fake `node` and `npm` scripts, the built binary on `PATH`
//! as `nvm`, and the real shells that evaluate `nvm init`.
//!
//! Every test crate uses some of this, so what one of them leaves out is not
//! dead code.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const BINARY: &str = env!("CARGO_BIN_EXE_nvm");
pub const SHELLS: [&str; 5] = ["bash", "zsh", "sh", "dash", "ksh"];
pub const USING_18: &str = "Now using node v18.20.4 (npm v10.7.0)\n";

/// The absolute path of `name` on the `PATH` of the tests, if installed.
pub fn find_shell(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain([PathBuf::from("/bin")])
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

pub fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

pub struct Lab {
    root: tempfile::TempDir,
}

/// What a shell printed, and its status.
pub struct Run {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Lab {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for (version, npm) in [("v18.20.4", "10.7.0"), ("v20.11.1", "10.2.4")] {
            let bin = root.path().join(format!("nvm/versions/node/{version}/bin"));
            fs::create_dir_all(&bin).unwrap();
            script(&bin.join("node"), &format!("echo {version}"));
            script(&bin.join("npm"), &format!("echo {npm}"));
        }
        for directory in ["bin", "path", "home", "proj", "nvm/alias"] {
            fs::create_dir_all(root.path().join(directory)).unwrap();
        }
        symlink(BINARY, root.path().join("bin/nvm")).unwrap();
        Self { root }
    }

    pub fn file(self, relative: &str, text: &str) -> Self {
        fs::write(self.root.path().join(relative), text).unwrap();
        self
    }

    /// Replaces the `npm` of `version` with a script running `body`.
    pub fn npm(self, version: &str, body: &str) -> Self {
        script(
            Path::new(&self.version_bin(version)).join("npm").as_path(),
            body,
        );
        self
    }

    pub fn at(&self, relative: &str) -> String {
        self.root.path().join(relative).display().to_string()
    }

    pub fn base_path(&self) -> String {
        format!("{}:{}", self.at("bin"), self.at("path"))
    }

    pub fn version_bin(&self, version: &str) -> String {
        self.at(&format!("nvm/versions/node/{version}/bin"))
    }

    /// Runs `commands` in `shell`, from the project directory.
    pub fn run(&self, shell: &Path, commands: &str) -> Run {
        let mut command = Command::new(shell);
        if shell.ends_with("zsh") {
            command.arg("-f");
        }
        let output = command
            .args(["-c", commands])
            .env_clear()
            .env("PATH", self.base_path())
            .env("NVM_DIR", self.at("nvm"))
            .env("HOME", self.at("home"))
            .env("PWD", self.at("proj"))
            .current_dir(self.at("proj"))
            .output()
            .unwrap();
        Run {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// Runs `body` once per installed shell, with the shell's name for messages.
pub fn each_shell(body: impl Fn(&str, &Path)) {
    for name in SHELLS {
        match find_shell(name) {
            Some(shell) => body(name, &shell),
            None => eprintln!("skipped: {name} is not installed"),
        }
    }
}

pub fn with_function(name: &str, commands: &str) -> String {
    format!("eval \"$(nvm init {name} --no-use)\"\n{commands}")
}

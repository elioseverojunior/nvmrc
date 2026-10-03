//! How a version is built from source: the number of `make` jobs
//! (`nvm_get_make_jobs`), the compiler words and the `make` program of
//! `nvm_install_source`, as pure rules.

use crate::domain::platform::Os;
use crate::domain::version::Version;

/// How many jobs `make` gets, and what is said about it on the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jobs {
    pub jobs: usize,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

/// `nvm_is_natural_num`: digits, and not zero.
#[must_use]
pub fn natural_jobs(text: &str) -> Option<usize> {
    let number: usize = text.parse().ok()?;
    (number > 0 && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(number)
}

/// The jobs for a build: `requested` (from `-j`) when it is a natural number,
/// else one fewer than the cores when there are more than two.
#[must_use]
pub fn jobs(requested: Option<&str>, cores: Option<usize>) -> Jobs {
    let mut said = Jobs {
        jobs: 1,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    if let Some(number) = requested.and_then(natural_jobs) {
        said.jobs = number;
        said.stdout.push(format!("number of `make` jobs: {number}"));
        return said;
    }
    if let Some(bad) = requested.filter(|text| !text.is_empty()) {
        said.stderr.push(format!(
            "{bad} is invalid for number of `make` jobs, must be a natural number"
        ));
    }
    let Some(cores) = cores.filter(|cores| *cores > 0) else {
        said.stderr.push(
            "Can not determine how many core(s) are available, running in single-threaded mode."
                .to_owned(),
        );
        said.stderr.push(
            "Please report an issue on GitHub to help us make nvm run faster on your computer!"
                .to_owned(),
        );
        return said;
    };
    said.stdout
        .push(format!("Detected that you have {cores} CPU core(s)"));
    if cores > 2 {
        said.jobs = cores - 1;
        said.stdout.push(format!(
            "Running with {} threads to speed up the build",
            said.jobs
        ));
    } else {
        said.stdout.push(
            "Number of CPU core(s) less than or equal to 2, running in single-threaded mode"
                .to_owned(),
        );
    }
    said
}

/// The version `clang --version` reports as `(major, minor)`: the word after
/// `version`, as `nvm_clang_version` reads it, without a `-suffix`.
#[must_use]
pub fn clang_version(output: &str) -> Option<(u64, u64)> {
    let line = output.lines().next()?;
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = words.iter().position(|word| *word == "version")?;
    if at != 1 && at != 2 {
        return None;
    }
    let number = words.get(at + 1)?.split('-').next()?;
    let mut parts = number.split('.').map(|part| part.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0)))
}

/// What `nvm.sh` does about the compiler: the `CC=` and `CXX=` words that
/// `make` gets, and whether to say that it picked Clang.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Compiler {
    pub words: Vec<String>,
    pub clang_note: bool,
}

/// `cc` and `c++` are named on macOS and the BSDs, and when Clang 3.5 or later
/// is there and `CC` or `CXX` is not set.
#[must_use]
pub fn compiler(
    os: Os,
    clang: Option<(u64, u64)>,
    cc: Option<&str>,
    cxx: Option<&str>,
) -> Compiler {
    let cc = cc.filter(|text| !text.is_empty());
    let cxx = cxx.filter(|text| !text.is_empty());
    let words = |_: ()| {
        vec![
            format!("CC={}", cc.unwrap_or("cc")),
            format!("CXX={}", cxx.unwrap_or("c++")),
        ]
    };
    let clang_note =
        clang.is_some_and(|version| version >= (3, 5)) && (cc.is_none() || cxx.is_none());
    let named = matches!(os, Os::Darwin | Os::FreeBsd | Os::OpenBsd) || clang_note;
    Compiler {
        words: if named { words(()) } else { Vec::new() },
        clang_note,
    }
}

/// `make`, or `gmake` where `make` is not GNU, and for `node` before 0.12 the
/// word that stops a shell from expanding globs in its Makefiles.
#[must_use]
pub fn make(os: Os, version: &Version) -> (&'static str, Vec<String>) {
    let program = if matches!(os, Os::FreeBsd | Os::OpenBsd | Os::Aix) {
        "gmake"
    } else {
        "make"
    };
    let shell = (version.triple() < (0, 12, 0)).then(|| "SHELL=/bin/sh".to_owned());
    (program, shell.into_iter().collect())
}

#[cfg(test)]
mod tests;

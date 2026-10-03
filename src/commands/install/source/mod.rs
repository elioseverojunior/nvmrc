//! Building a version from source: `nvm_install_source`. Download the source
//! archive, unpack it, then `./configure --prefix=<version path>`, `make` and
//! `make install`.

use std::path::Path;

use crate::commands::install::fetch::{Artifact, fetch};
use crate::commands::install::place::unpack;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::path_search::find_in_path;
use crate::domain::platform::Os;
use crate::domain::source_build::{clang_version, compiler, make};
use crate::domain::version::Version;
use crate::ports::Invocation;

/// What the build needs from the command line and the environment.
pub struct Build<'a> {
    pub version: &'a Version,
    pub version_path: &'a Path,
    pub jobs: usize,
    /// The words after the version, for `./configure`.
    pub extra: &'a [String],
    pub offline: bool,
}

/// A build that failed; what it printed is in the transcript.
#[derive(Debug, PartialEq, Eq)]
pub struct BuildFailed;

/// `--without-snapshot` for 32-bit ARM, then what the user passed, each
/// preceded by a space as `nvm.sh` builds the string (so it shows two spaces
/// after the arm option).
fn parameters(context: &Context<'_>, extra: &[String]) -> String {
    let given: String = extra.iter().map(|word| format!(" {word}")).collect();
    let arm = context
        .platform()
        .is_some_and(|platform| matches!(platform.arch.as_str(), "armv6l" | "armv7l"));
    match (arm, given.is_empty()) {
        (true, true) => "--without-snapshot".to_owned(),
        (true, false) => format!("--without-snapshot {given}"),
        (false, _) => given,
    }
}

/// The version Clang reports, when both `clang` and `clang++` are on `PATH`.
fn clang(context: &Context<'_>) -> Option<(u64, u64)> {
    let path = context.env.var_os("PATH").unwrap_or_default();
    find_in_path(context.fs, &path, "clang++")?;
    let program = find_in_path(context.fs, &path, "clang")?;
    let output = context.process().run(&program, &["--version"]).ok()?;
    clang_version(&output.stdout)
}

/// Runs a step of the build in `directory`; false (after printing why) when it
/// does not succeed.
fn run(context: &Context<'_>, invocation: &Invocation, transcript: &mut Transcript) -> bool {
    match context.process().execute(invocation) {
        Ok(done) => {
            done.stdout.lines().for_each(|line| transcript.out(line));
            done.stderr.lines().for_each(|line| transcript.err(line));
            done.success
        }
        Err(error) => {
            transcript.err(format!("{}: {error}", invocation.program.display()));
            false
        }
    }
}

/// # Errors
/// [`BuildFailed`], after the transcript says why.
pub fn build(
    context: &Context<'_>,
    job: &Build<'_>,
    transcript: &mut Transcript,
) -> Result<(), BuildFailed> {
    let params = parameters(context, job.extra);
    if !params.is_empty() {
        transcript.out(format!("Additional options while compiling: {params}"));
    }
    let os = context.platform().map_or(Os::Linux, |platform| platform.os);
    let toolchain = compiler(
        os,
        clang(context),
        context.env.var("CC").as_deref(),
        context.env.var("CXX").as_deref(),
    );
    if toolchain.clang_note {
        transcript.out(
            "Clang v3.5+ detected! CC or CXX not specified, will use Clang as C/C++ compiler!",
        );
    }
    let artifact = Artifact::source_of(context, job.version).ok_or(BuildFailed)?;
    let tarball =
        fetch(context, &artifact, job.version, job.offline, transcript).map_err(|_| BuildFailed)?;
    let files = artifact.files();
    unpack_source(context, &tarball, &files)
        .map_err(|message| failed(context, job, &files, message, transcript))?;
    compile(
        context,
        job,
        &files,
        &params,
        &toolchain.words,
        os,
        transcript,
    )
    .map_err(|()| failed(context, job, &files, String::new(), transcript))
}

/// The source tree is `files` itself, the archive's top-level directory
/// stripped (`tar --strip-components 1`).
fn unpack_source(context: &Context<'_>, tarball: &Path, files: &Path) -> Result<(), String> {
    let top = unpack(context, tarball, files)?;
    let entries = context
        .fs
        .read_dir(&top)
        .map_err(|error| error.to_string())?;
    for entry in entries {
        context
            .fs
            .rename(&top.join(&entry.name), &files.join(&entry.name))
            .map_err(|error| error.to_string())?;
    }
    context
        .fs
        .remove_dir_all(&top)
        .map_err(|error| error.to_string())
}

/// `nvm: install <v> failed!`, and the unpacked tree is removed.
fn failed(
    context: &Context<'_>,
    job: &Build<'_>,
    files: &Path,
    message: String,
    transcript: &mut Transcript,
) -> BuildFailed {
    if !message.is_empty() {
        transcript.err(message);
    }
    transcript.err(format!(
        "nvm: install {} failed!",
        job.version.directory_name()
    ));
    let _ = context.fs.remove_dir_all(files);
    BuildFailed
}

/// `./configure`, `make`, then `make install`.
fn compile(
    context: &Context<'_>,
    job: &Build<'_>,
    top: &Path,
    params: &str,
    compiler_words: &[String],
    os: Os,
    transcript: &mut Transcript,
) -> Result<(), ()> {
    let prefix = format!("--prefix={}", job.version_path.display());
    let words: Vec<&str> = params.split_whitespace().collect();
    // `nvm.sh` writes `$ADDITIONAL_PARAMETERS'<'`: the `<` sticks to the last
    // option, and stands alone when there are none.
    let mut shown = vec!["$>./configure".to_owned(), prefix.clone()];
    shown.extend(words.iter().map(|word| (*word).to_owned()));
    if words.is_empty() {
        shown.push("<".to_owned());
    } else if let Some(last) = shown.last_mut() {
        last.push('<');
    }
    transcript.out(shown.join(" "));
    let configure = Invocation::new(top.join("configure"))
        .args(&[&prefix])
        .args(&words)
        .dir(top);
    let (program, shell) = make(os, job.version);
    let jobs = job.jobs.to_string();
    let make_args = |goal: Option<&str>| {
        let mut args: Vec<&str> = shell.iter().map(String::as_str).collect();
        args.extend(["-j", &jobs]);
        args.extend(compiler_words.iter().map(String::as_str));
        args.extend(goal);
        Invocation::new(program).args(&args).dir(top)
    };
    if !run(context, &configure, transcript) || !run(context, &make_args(None), transcript) {
        return Err(());
    }
    let _ = context.fs.remove_file(job.version_path);
    run(context, &make_args(Some("install")), transcript)
        .then_some(())
        .ok_or(())
}

#[cfg(test)]
mod tests;

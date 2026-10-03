//! The npmrc half of `nvm_die_on_prefix`: the builtin, global, user and
//! project files, in that order, refused or cleaned with `npm config`.

use std::ffi::OsStr;
use std::path::Path;

use super::{Check, Refusal};
use crate::commands::npm::Npm;
use crate::commands::sanitize::sanitize_path;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::prefix::sets_prefix;

/// Where an npmrc file sits, which names it and decides how npm reaches it.
#[derive(Debug, Clone, Copy)]
enum Scope {
    Builtin,
    Global,
    User,
    Project,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Self::Builtin => "builtin npmrc file",
            Self::Global => "global npmrc file",
            Self::User => "user\u{2019}s .npmrc file",
            Self::Project => "project npmrc file",
        }
    }

    /// The arguments of `npm config ... delete <key>` for the file.
    fn delete_args(self, file: &str, key: &str) -> Vec<String> {
        let mut args = vec!["config".to_owned()];
        if matches!(self, Self::Global) {
            args.push("--global".to_owned());
        }
        args.extend(["--loglevel=warn", "delete", key].map(str::to_owned));
        if matches!(self, Self::Builtin | Self::User) {
            args.push(format!("--userconfig={file}"));
        }
        args
    }
}

/// Checks the four files; with `--delete-prefix` a bad one is cleaned and
/// the checks go on.
pub(super) fn check(
    context: &Context<'_>,
    check: &Check<'_>,
    transcript: &mut Transcript,
) -> Result<(), Refusal> {
    for (scope, file) in files(context, check.directory) {
        if !is_bad(context, &file) {
            continue;
        }
        if !check.delete {
            refuse(context, scope, &file, check.command, transcript);
            return Err(Refusal::KeepSwitch);
        }
        delete(context, scope, &file, check.path, transcript);
    }
    Ok(())
}

fn files(context: &Context<'_>, directory: &str) -> [(Scope, String); 4] {
    let home = context.env.var("HOME").unwrap_or_default();
    [
        (
            Scope::Builtin,
            format!("{directory}/lib/node_modules/npm/npmrc"),
        ),
        (Scope::Global, format!("{directory}/etc/npmrc")),
        (Scope::User, format!("{home}/.npmrc")),
        (Scope::Project, format!("{}/.npmrc", project_dir(context))),
    ]
}

/// `nvm_find_project_dir`: the nearest ancestor of the logical working
/// directory ([`Context::working_directory`]) with a `package.json` file or
/// a `node_modules` directory; empty when none has.
fn project_dir(context: &Context<'_>) -> String {
    let mut directory = context
        .working_directory()
        .map(|directory| directory.to_string_lossy().into_owned())
        .unwrap_or_default();
    while !directory.is_empty() && directory != "." && !holds_project(context, &directory) {
        directory = directory
            .rsplit_once('/')
            .map_or_else(String::new, |(parent, _)| parent.to_owned());
    }
    directory
}

fn holds_project(context: &Context<'_>, directory: &str) -> bool {
    let directory = Path::new(directory);
    context.fs.is_file(&directory.join("package.json"))
        || context
            .fs
            .file_info(&directory.join("node_modules"))
            .is_ok_and(|info| info.is_dir)
}

/// `nvm_npmrc_bad_news_bears`: a regular file with a `prefix` or
/// `globalconfig` line. One that cannot be read as text is taken as clean.
fn is_bad(context: &Context<'_>, file: &str) -> bool {
    let file = Path::new(file);
    context.fs.is_file(file)
        && context
            .fs
            .read_to_string(file)
            .is_ok_and(|contents| sets_prefix(&contents))
}

fn refuse(
    context: &Context<'_>,
    scope: Scope,
    file: &str,
    command: &str,
    transcript: &mut Transcript,
) {
    let shown = sanitize_path(context, file);
    transcript.err(format!("Your {} ({shown})", scope.label()));
    transcript
        .err("has a `globalconfig` and/or a `prefix` setting, which are incompatible with nvm.");
    transcript.err(format!("Run `{command}` to unset it."));
}

/// The two `npm config ... delete` runs, with the `npm` of the new `PATH`;
/// their output goes on to the transcript and their status is ignored.
fn delete(
    context: &Context<'_>,
    scope: Scope,
    file: &str,
    path: &str,
    transcript: &mut Transcript,
) {
    let Some(npm) = Npm::on_path_value(context, OsStr::new(path)) else {
        transcript.err("npm: command not found");
        return;
    };
    for key in ["prefix", "globalconfig"] {
        let args = scope.delete_args(file, key);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        npm.run(context, &args, transcript);
    }
}

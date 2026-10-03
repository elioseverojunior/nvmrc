//! Moving the global packages of one node version to another:
//! `nvm reinstall-packages` and `nvm install --reinstall-packages-from`.

use crate::commands::install::place::version_path;
use crate::commands::npm::Npm;
use crate::commands::resolve::system_node;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::global_packages::parse;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// Where the packages come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Version(Version),
    /// The `node` outside `$NVM_DIR`.
    System,
    /// Nothing is installed under that name (shown as `N/A`).
    Missing,
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Self::Version(version) => version.to_string(),
            Self::System => "system".to_owned(),
            Self::Missing => "N/A".to_owned(),
        }
    }

    fn npm(&self, context: &Context<'_>) -> Option<Npm> {
        match self {
            Self::Version(version) => {
                Npm::in_version(context, &version_path(context, version).ok()?)
            }
            Self::System => {
                let node = system_node(context).ok()??;
                Npm::in_version(context, node.parent()?.parent()?)
            }
            Self::Missing => None,
        }
    }
}

/// What `nvm use <missing>` prints, which `nvm.sh` lets through.
fn not_yet_installed(name: &str) -> String {
    format!(
        "{name}: version \"{name}\" is not yet installed.\n\nYou need to run `nvm install {name}` to install and use it."
    )
}

/// Installs the packages of `source` with `destination`, then links the ones
/// that were linked. The status is that of the last link, as in `nvm.sh`
/// (a package that fails to install does not change it).
pub fn reinstall(
    context: &Context<'_>,
    source: &Source,
    destination: &Npm,
    transcript: &mut Transcript,
) -> NvmExitCode {
    let listing = source
        .npm(context)
        .and_then(|npm| npm.output(context, &["list", "-g", "--depth=0"]))
        .unwrap_or_default();
    if *source == Source::Missing {
        transcript.err(not_yet_installed("N/A"));
    }
    let packages = parse(&listing);
    let label = source.label();
    transcript.out(format!("Reinstalling global packages from {label}..."));
    if packages.installs.is_empty() {
        transcript.out("No installed global packages found...");
    } else {
        let mut args = vec!["install", "-g", "--quiet"];
        args.extend(packages.installs.iter().map(String::as_str));
        destination.run(context, &args, transcript);
    }
    transcript.out(format!("Linking global packages from {label}..."));
    link(context, destination, &packages.links, transcript)
}

/// Where `npm link` runs for a linked package: an absolute target as it is, a
/// relative one from the global `node_modules` (as `nvm.sh` joins them).
fn link_directory(root: &str, target: &str) -> String {
    if target.starts_with('/') {
        target.to_owned()
    } else {
        format!("{root}/../{target}")
    }
}

/// `npm root -g`: where the global packages are.
fn global_root(context: &Context<'_>, npm: &Npm) -> String {
    npm.output(context, &["root", "-g"])
        .map(|text| text.trim().to_owned())
        .unwrap_or_default()
}

fn link(
    context: &Context<'_>,
    destination: &Npm,
    links: &[String],
    transcript: &mut Transcript,
) -> NvmExitCode {
    if links.is_empty() {
        transcript.out("No linked global packages found...");
        return NvmExitCode::Success;
    }
    let root = global_root(context, destination);
    let mut status = NvmExitCode::Success;
    for target in links.iter().filter(|target| !target.is_empty()) {
        let directory = link_directory(&root, target);
        let linked = destination.run_in(
            context,
            std::path::Path::new(&directory),
            &["link"],
            transcript,
        );
        status = if linked {
            NvmExitCode::Success
        } else {
            NvmExitCode::Failure
        };
    }
    status
}

#[cfg(test)]
mod tests;

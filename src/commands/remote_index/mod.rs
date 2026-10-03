//! Downloading a mirror's release index, and refreshing the `lts/*` aliases
//! from it, as `nvm_ls_remote_index_tab` does.

use std::path::Path;

use crate::context::Context;
use crate::domain::index::{Release, lts_aliases, parse_index};
use crate::domain::mirror;
use crate::domain::remote::normalize_lts;
use crate::domain::version::Flavor;
use crate::error::CliError;

/// The releases of one mirror, or `None` when they could not be had (a
/// mirror that is not a plain URL, or a download that failed): `nvm.sh` then
/// lists nothing for that flavor and exits 3.
pub struct Fetched {
    pub releases: Option<Vec<Release>>,
    /// What `nvm.sh` prints on stderr about it.
    pub warning: Option<String>,
}

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn fetch(context: &Context<'_>, flavor: Flavor) -> Result<Fetched, CliError> {
    let alias_lts_dir = context.alias_dir()?.join("lts");
    // nvm.sh creates the directory whatever the download does.
    let _ = context.fs.create_dir_all(&alias_lts_dir);
    let mirror = match mirror::from_env(context.env, flavor) {
        Ok(mirror) => mirror,
        Err(error) => {
            return Ok(Fetched {
                releases: None,
                warning: Some(error.to_string()),
            });
        }
    };
    let Ok(text) = context.http().get_text(&mirror.index_url()) else {
        return Ok(Fetched {
            releases: None,
            warning: None,
        });
    };
    let releases = parse_index(&text, flavor);
    refresh_lts_aliases(context, &alias_lts_dir, &releases);
    Ok(Fetched {
        releases: Some(releases),
        warning: None,
    })
}

/// Writes `alias/lts/*` and one alias per codename. `nvm.sh` hides every
/// failure of this, and so does it.
fn refresh_lts_aliases(context: &Context<'_>, directory: &Path, releases: &[Release]) {
    for (name, target) in lts_aliases(releases) {
        let file = directory.join(name.trim_start_matches("lts/"));
        let _ = context.fs.write_file(&file, &format!("{target}\n"));
    }
}

/// The codename a `--lts` argument stands for, once the aliases are fresh;
/// the message `nvm.sh` prints when it stands for none.
///
/// # Errors
/// The message to print.
pub fn lts_filter(context: &Context<'_>, wanted: &str) -> Result<String, String> {
    let directory = context
        .alias_dir()
        .map_err(|error| error.to_string())?
        .join("lts");
    let mut names: Vec<String> = context
        .fs
        .read_dir(&directory)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.sort();
    normalize_lts(wanted, &names).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;

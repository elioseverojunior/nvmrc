//! Getting the archive of a version into `$NVM_DIR/.cache/bin/<slug>/`, and
//! checking it against the mirror's `SHASUMS256.txt`, as `nvm_download_artifact`
//! does, the `.tar.xz` where it would and the `.tar.gz` elsewhere.

use std::path::{Path, PathBuf};

use crate::commands::sanitize::sanitize_path;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::checksum::{compare, expected_digest};
use crate::domain::compression::Compression;
use crate::domain::mirror::{self, MirrorUrl};
use crate::domain::platform::Os;
use crate::domain::version::{Flavor, Version};

/// A failure whose messages are in the transcript already.
#[derive(Debug, PartialEq, Eq)]
pub struct Failed;

/// Where an archive lives in the cache.
pub struct Artifact {
    /// `node-v20.10.0-linux-x64`.
    pub slug: String,
    /// `node-v20.10.0-linux-x64.tar.xz`.
    pub file_name: String,
    /// `$NVM_DIR/.cache/bin/<slug>`.
    pub directory: PathBuf,
    pub tarball: PathBuf,
}

impl Artifact {
    /// `None` when there are no binaries for this machine, or `$NVM_DIR` is
    /// unknown.
    #[must_use]
    pub fn of(context: &Context<'_>, version: &Version) -> Option<Self> {
        let platform = context.platform()?;
        let slug = platform.download_slug(version);
        let directory = context.cache_dir().ok()?.join("bin").join(&slug);
        let file_name = file_name(&slug, version, platform.os);
        let tarball = directory.join(&file_name);
        Some(Self {
            slug,
            file_name,
            directory,
            tarball,
        })
    }

    /// The source archive of `version`: `node-v20.10.0` in `.cache/src`, the
    /// same for every platform.
    #[must_use]
    pub fn source_of(context: &Context<'_>, version: &Version) -> Option<Self> {
        let flavor = match version.flavor {
            Flavor::Node => "node",
            Flavor::IoJs => "iojs",
        };
        let slug = format!("{flavor}-{}", version.directory_name());
        let directory = context.cache_dir().ok()?.join("src").join(&slug);
        let file_name = file_name(&slug, version, context.platform()?.os);
        let tarball = directory.join(&file_name);
        Some(Self {
            slug,
            file_name,
            directory,
            tarball,
        })
    }

    /// Where the archive is unpacked.
    #[must_use]
    pub fn files(&self) -> PathBuf {
        self.directory.join("files")
    }
}

/// `<slug>.tar.xz` or `<slug>.tar.gz`, as `nvm_get_artifact_compression` says.
fn file_name(slug: &str, version: &Version, os: Os) -> String {
    format!("{slug}.{}", Compression::preferred(version, os).extension())
}

/// The archive of `version`, from the cache when its checksum matches and from
/// the mirror otherwise.
///
/// # Errors
/// [`Failed`], after the transcript says why.
pub fn fetch(
    context: &Context<'_>,
    artifact: &Artifact,
    version: &Version,
    offline: bool,
    transcript: &mut Transcript,
) -> Result<PathBuf, Failed> {
    if offline {
        return cached_only(context, artifact, transcript);
    }
    let mirror = mirror::from_env(context.env, version.flavor).map_err(|error| {
        transcript.err(error.to_string());
        Failed
    })?;
    let expected = expected_checksum(context, &mirror, version, artifact);
    let files = artifact.files();
    context.fs.create_dir_all(&files).map_err(|_| {
        transcript.err(format!("creating directory {} failed", files.display()));
        Failed
    })?;
    if reuse_cache(context, artifact, &expected, transcript) {
        return Ok(artifact.tarball.clone());
    }
    download(context, &mirror, version, artifact, transcript)?;
    verify(context, artifact, &expected, transcript)?;
    Ok(artifact.tarball.clone())
}

/// `--offline`: the cached archive, taken without a checksum, or nothing.
fn cached_only(
    context: &Context<'_>,
    artifact: &Artifact,
    transcript: &mut Transcript,
) -> Result<PathBuf, Failed> {
    let cached = context.fs.file_info(&artifact.tarball);
    if cached.is_ok_and(|info| !info.is_dir) {
        let shown = sanitize_path(context, &artifact.tarball.to_string_lossy());
        transcript.err(format!("Offline: using cached archive {shown}"));
        return Ok(artifact.tarball.clone());
    }
    transcript.err(format!(
        "Offline: no cached archive found for {}",
        artifact.slug
    ));
    Err(Failed)
}

/// What `SHASUMS256.txt` lists for the archive; empty when it cannot be had.
fn expected_checksum(
    context: &Context<'_>,
    mirror: &MirrorUrl,
    version: &Version,
    artifact: &Artifact,
) -> String {
    let url = mirror.join(&format!("{}/SHASUMS256.txt", version.directory_name()));
    context
        .http()
        .get_text(&url)
        .ok()
        .and_then(|shasums| expected_digest(&shasums, &artifact.file_name))
        .unwrap_or_default()
}

fn digest_of(context: &Context<'_>, tarball: &Path) -> String {
    context.digest().sha256_file(tarball).unwrap_or_default()
}

/// True when a cached archive exists and is the right one; a broken one is
/// removed.
fn reuse_cache(
    context: &Context<'_>,
    artifact: &Artifact,
    expected: &str,
    transcript: &mut Transcript,
) -> bool {
    if context.fs.file_info(&artifact.tarball).is_err() {
        return false;
    }
    let shown = sanitize_path(context, &artifact.tarball.to_string_lossy());
    transcript.err(format!("Local cache found: {shown}"));
    match compare(&digest_of(context, &artifact.tarball), expected) {
        Ok(()) => {
            transcript.err(format!(
                "Checksums match! Using existing downloaded archive {shown}"
            ));
            true
        }
        Err(error) => {
            transcript.err(error.to_string());
            transcript.err("Checksum check failed!");
            transcript.err("Removing the broken local cache...");
            let _ = context.fs.remove_dir_all(&artifact.tarball);
            false
        }
    }
}

fn download(
    context: &Context<'_>,
    mirror: &MirrorUrl,
    version: &Version,
    artifact: &Artifact,
    transcript: &mut Transcript,
) -> Result<(), Failed> {
    let url = mirror.join(&format!(
        "{}/{}",
        version.directory_name(),
        artifact.file_name
    ));
    transcript.err(format!("Downloading {url}..."));
    let stored = context
        .http()
        .get_bytes(&url)
        .map_err(|_| ())
        .and_then(|bytes| {
            context
                .fs
                .write_bytes(&artifact.tarball, &bytes)
                .map_err(|_| ())
        });
    if stored.is_err() {
        let _ = context.fs.remove_dir_all(&artifact.directory);
        transcript.err(format!("download from {url} failed"));
        return Err(Failed);
    }
    Ok(())
}

fn verify(
    context: &Context<'_>,
    artifact: &Artifact,
    expected: &str,
    transcript: &mut Transcript,
) -> Result<(), Failed> {
    match compare(&digest_of(context, &artifact.tarball), expected) {
        Ok(()) => {
            transcript.err("Checksums matched!");
            Ok(())
        }
        Err(error) => {
            transcript.err(error.to_string());
            // Never left for a later `--offline` install, which takes the
            // cache without a checksum.
            let _ = context.fs.remove_file(&artifact.tarball);
            let _ = context.fs.remove_dir_all(&artifact.files());
            Err(Failed)
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod name_tests;

#[cfg(test)]
mod offline_tests;

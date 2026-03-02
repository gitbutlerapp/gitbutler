use std::path::{Path, PathBuf};

use anyhow::Context as _;
use but_path::AppChannel;

use crate::ProjectHandle;

/// Return the config key used to customize the GitButler storage path for the current app channel.
pub fn storage_path_config_key() -> &'static str {
    storage_path_config_key_for_channel(&AppChannel::new())
}

/// Return the path where per-project GitButler data should be stored for `repo`.
pub fn gitbutler_storage_path(repo: &gix::Repository) -> anyhow::Result<PathBuf> {
    let git_dir = repo.git_dir();
    let channel = AppChannel::new();
    let storage_key = storage_path_config_key_for_channel(&channel);

    match repo.config_snapshot().trusted_path(storage_key) {
        Some(Ok(path)) => resolve_configured_storage_path(git_dir, path.as_ref()),
        Some(Err(err)) => {
            Err(err).with_context(|| format!("{storage_key} contains an invalid path"))
        }
        None => Ok(git_dir.join(default_gitbutler_storage_dir_name(&channel))),
    }
}

fn resolve_configured_storage_path(
    git_dir: &Path,
    configured_path: &Path,
) -> anyhow::Result<PathBuf> {
    if !configured_path.is_absolute() {
        return Ok(git_dir.join(configured_path));
    }

    let storage_path = configured_path.to_owned();
    if storage_path.starts_with(git_dir) {
        return Ok(storage_path);
    }
    let gitdir_real = gix::path::realpath(git_dir)?;
    if storage_path.starts_with(&gitdir_real) {
        return Ok(storage_path);
    }

    let project_handle = ProjectHandle::from_path(git_dir)?;
    let storage_path = storage_path.join(project_handle.to_string());
    tracing::trace!(
        storage_path = %storage_path.display(),
        config_key = %storage_path_config_key(),
        "Resolved GitButler storage path; set this config key to override"
    );
    Ok(storage_path)
}

fn default_gitbutler_storage_dir_name(channel: &AppChannel) -> &'static str {
    match channel {
        AppChannel::Release => "gitbutler",
        AppChannel::Nightly => "gitbutler.nightly",
        AppChannel::Dev => "gitbutler.dev",
    }
}

fn storage_path_config_key_for_channel(channel: &AppChannel) -> &'static str {
    match channel {
        AppChannel::Release => "gitbutler.storagePath",
        AppChannel::Nightly => "gitbutler.nightly.storagePath",
        AppChannel::Dev => "gitbutler.dev.storagePath",
    }
}

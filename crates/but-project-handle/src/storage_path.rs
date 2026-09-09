use std::{
    borrow::Cow,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context as _, bail};
use but_path::AppChannel;

use crate::ProjectHandle;

/// Return the config key used to customize the GitButler storage path for the current app channel.
pub fn storage_path_config_key() -> &'static str {
    storage_path_config_key_for_channel(&AppChannel::new())
}

/// Return the config key used to customize the GitButler storage path for `channel`.
pub fn storage_path_config_key_for_app_channel(channel: AppChannel) -> &'static str {
    storage_path_config_key_for_channel(&channel)
}

/// Return the path where per-project GitButler data should be stored for `repo`.
pub fn gitbutler_storage_path(repo: &gix::Repository) -> anyhow::Result<PathBuf> {
    gitbutler_storage_path_for_channel(repo, AppChannel::new())
}

/// Return the path where per-project GitButler data should be stored for `repo` and `channel`.
pub fn gitbutler_storage_path_for_channel(
    repo: &gix::Repository,
    channel: AppChannel,
) -> anyhow::Result<PathBuf> {
    let git_dir = repo.git_dir();
    let storage_key = storage_path_config_key_for_channel(&channel);

    match repo.config_snapshot().trusted_path(storage_key) {
        Ok(Some(path)) => resolve_configured_storage_path(git_dir, path.as_ref()),
        Err(err) => Err(err).with_context(|| format!("{storage_key} contains an invalid path")),
        Ok(None) => Ok(git_dir.join(DEFAULT_STORAGE_DIR_NAME)),
    }
}

fn resolve_configured_storage_path(
    git_dir: &Path,
    configured_path: &Path,
) -> anyhow::Result<PathBuf> {
    let mut storage_path = gix::path::normalize(configured_path.into(), git_dir)
        .unwrap_or(Cow::Borrowed(configured_path))
        .into_owned();
    if storage_path.is_relative() {
        storage_path = git_dir.join(storage_path);
    }
    if let Some(relative_to_git_dir) = storage_path_relative_to_git_dir(git_dir, &storage_path)? {
        validate_in_git_storage_path(&relative_to_git_dir, &storage_path)?;
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

/// Return the `storage_path` as relative to `git_dir` if it is contained inside of it.
fn storage_path_relative_to_git_dir(
    git_dir: &Path,
    storage_path: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    if let Ok(relative) = storage_path.strip_prefix(git_dir) {
        return Ok(Some(relative.to_owned()));
    }

    let gitdir_real = gix::path::realpath(git_dir)?;
    Ok(storage_path
        .strip_prefix(&gitdir_real)
        .ok()
        .map(Path::to_owned))
}

/// Only accept `storage_path_relative_to_git_dir` if it's a top-level `gitbutler` directory.
/// Use `storage_path` to format error messages.
fn validate_in_git_storage_path(
    storage_path_relative_to_git_dir: &Path,
    storage_path: &Path,
) -> anyhow::Result<()> {
    let Some(Component::Normal(top_level_dir)) =
        storage_path_relative_to_git_dir.components().next()
    else {
        bail!(
            "configured storage path '{}' resolves to '.git' itself; choose a dedicated GitButler directory instead",
            storage_path.display()
        );
    };

    if !top_level_dir
        .to_string_lossy()
        .get(..DEFAULT_STORAGE_DIR_NAME.len())
        .is_some_and(|name| name.eq_ignore_ascii_case(DEFAULT_STORAGE_DIR_NAME))
    {
        bail!(
            "configured storage path '{}' resolves inside '.git' but not under a top-level 'gitbutler*' directory",
            storage_path.display()
        );
    }

    Ok(())
}

/// Name of the default GitButler storage directory inside the git dir.
pub const DEFAULT_STORAGE_DIR_NAME: &str = "gitbutler";

/// Git-dir-relative path of the refresh sentinel, `gitbutler/REFRESH`.
///
/// Single source of truth shared by the writer (`write_refresh_sentinel`) and
/// the watcher (`gitbutler_filemonitor`).
pub const REFRESH_SENTINEL_PATH: &str = "gitbutler/REFRESH";

/// Identity written into the refresh sentinel so a process can skip the write
/// it caused (its own writes are already handled in-process) while still
/// reacting to others — the `but` CLI, or a second GUI window.
///
/// A pid suffices: a recycled one can't be misread as ours, since we only
/// compare right after the watcher fires on a fresh write.
pub fn process_sentinel_token() -> String {
    std::process::id().to_string()
}

/// Best-effort write of the refresh sentinel in `metadata_file`'s directory —
/// how out-of-process writes (notably the `but` CLI) reach the desktop watcher.
/// Contents are this process's [`process_sentinel_token`] so it can skip its own
/// write. Errors are logged and swallowed, never failing the mutation.
pub fn write_refresh_sentinel(metadata_file: &Path) {
    let Some(dir) = metadata_file.parent() else {
        return;
    };
    let Some(filename) = Path::new(REFRESH_SENTINEL_PATH).file_name() else {
        return;
    };
    let sentinel = dir.join(filename);
    if let Err(err) = std::fs::write(&sentinel, process_sentinel_token()) {
        tracing::warn!(?sentinel, %err, "failed to touch refresh sentinel");
    }
}

/// Git-dir-relative path of the invalidation sentinel, `gitbutler/INVALIDATE`.
///
/// Where a successful mutation records the cache tags it declared stale, for
/// the watchers of other processes. Kept apart from [`REFRESH_SENTINEL_PATH`],
/// which every metadata flush overwrites.
pub const INVALIDATION_SENTINEL_PATH: &str = "gitbutler/INVALIDATE";

/// Best-effort write of `tags` to the invalidation sentinel in
/// `project_data_dir`, signed with this process's [`process_sentinel_token`]
/// and replacing whatever was there. A write the watcher did not get to read
/// before the next one is lost, and the clients' polls cover that; a log to
/// replay is not worth its bookkeeping. Errors are logged and swallowed,
/// never failing the mutation.
pub fn write_invalidation_sentinel(project_data_dir: &Path, tags: &[&str]) {
    if tags.is_empty() {
        return;
    }
    let Some(filename) = Path::new(INVALIDATION_SENTINEL_PATH).file_name() else {
        return;
    };
    let sentinel = project_data_dir.join(filename);
    let content = format!("{} {}", process_sentinel_token(), tags.join(","));
    let written =
        std::fs::create_dir_all(project_data_dir).and_then(|()| std::fs::write(&sentinel, content));
    if let Err(err) = written {
        tracing::warn!(?sentinel, %err, "failed to write the invalidation sentinel");
    }
}

/// The tags the sentinel `content` names, unless `own_token` wrote them.
pub fn invalidation_by_others(content: &str, own_token: &str) -> Vec<String> {
    let mut fields = content.split_whitespace();
    let Some(token) = fields.next() else {
        return Vec::new();
    };
    if token == own_token {
        return Vec::new();
    }
    fields
        .next()
        .map(|tags| {
            tags.split(',')
                .filter(|tag| !tag.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn storage_path_config_key_for_channel(channel: &AppChannel) -> &'static str {
    match channel {
        AppChannel::Release => "gitbutler.storagePath",
        AppChannel::Nightly => "gitbutler.nightly.storagePath",
        AppChannel::Dev => "gitbutler.dev.storagePath",
    }
}

#[cfg(test)]
mod invalidation_tests {
    use super::*;

    fn tempdir() -> but_testsupport::gix_testtools::tempfile::TempDir {
        but_testsupport::gix_testtools::tempfile::TempDir::new().unwrap()
    }

    #[test]
    fn tags_by_others_are_read_and_own_writes_are_not() {
        assert_eq!(
            invalidation_by_others("1 Reviews,Checks", "2"),
            ["Reviews", "Checks"]
        );
        assert!(invalidation_by_others("1 Reviews", "1").is_empty());
        assert!(invalidation_by_others("", "1").is_empty());
        assert!(invalidation_by_others("1", "2").is_empty());
    }

    #[test]
    fn each_write_replaces_the_last() {
        let dir = tempdir();
        write_invalidation_sentinel(dir.path(), &["Reviews"]);
        write_invalidation_sentinel(dir.path(), &["Checks", "MergeStatus"]);
        let content = std::fs::read_to_string(dir.path().join("INVALIDATE")).unwrap();
        assert_eq!(
            content,
            format!("{} Checks,MergeStatus", process_sentinel_token())
        );
    }

    #[test]
    fn nothing_to_declare_writes_nothing() {
        let dir = tempdir();
        write_invalidation_sentinel(dir.path(), &[]);
        assert!(!dir.path().join("INVALIDATE").exists());
    }
}

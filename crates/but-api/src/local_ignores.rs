use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

use anyhow::{Context as _, Result, bail};
use but_core::git_config::{edit_repo_config, ensure_config_value};
use gix::config::AsKey as _;

const LOCAL_IGNORED_PATH_KEY: &str = "gitbutler.localignoredpath";

/// Return the repo-local list of paths hidden by GitButler on this machine.
pub(crate) fn list_local_ignored_paths(repo: &gix::Repository) -> Result<Vec<String>> {
    let config = open_local_config_for_reading(repo)?;
    let key = LOCAL_IGNORED_PATH_KEY
        .try_as_key()
        .with_context(|| format!("invalid git config key: {LOCAL_IGNORED_PATH_KEY}"))?;

    let mut paths = BTreeSet::new();
    if let Ok(values) = config.raw_values_by(key.section_name, key.subsection_name, key.value_name) {
        for value in values {
            if let Some(normalized) =
                normalize_local_ignore_path_string(&String::from_utf8_lossy(value.as_ref()))
            {
                paths.insert(normalized);
            }
        }
    }
    Ok(paths.into_iter().collect())
}

/// Add or remove one repo-relative path from the repo-local GitButler ignore list.
pub(crate) fn set_local_ignored_path(
    repo: &gix::Repository,
    relative_path: &Path,
    ignored: bool,
) -> Result<bool> {
    let normalized_path = normalize_local_ignore_path(relative_path)?;
    let mut paths: BTreeSet<_> = list_local_ignored_paths(repo)?.into_iter().collect();

    let changed = if ignored {
        paths.insert(normalized_path)
    } else {
        paths.remove(&normalized_path)
    };
    if !changed {
        return Ok(false);
    }

    edit_repo_config(repo, gix::config::Source::Local, |config| {
        remove_all_local_ignored_paths(config)?;
        for path in &paths {
            ensure_config_value(config, LOCAL_IGNORED_PATH_KEY, path)?;
        }
        Ok(())
    })?;
    Ok(true)
}

/// Add or remove multiple repo-relative paths from the repo-local GitButler ignore list.
pub(crate) fn set_local_ignored_paths<'a>(
    repo: &gix::Repository,
    relative_paths: impl IntoIterator<Item = &'a Path>,
    ignored: bool,
) -> Result<usize> {
    let normalized_paths = relative_paths
        .into_iter()
        .map(normalize_local_ignore_path)
        .collect::<Result<Vec<_>>>()?;
    let mut paths: BTreeSet<_> = list_local_ignored_paths(repo)?.into_iter().collect();

    let mut changed = 0;
    for normalized_path in normalized_paths {
        let path_changed = if ignored {
            paths.insert(normalized_path)
        } else {
            paths.remove(&normalized_path)
        };
        changed += usize::from(path_changed);
    }
    if changed == 0 {
        return Ok(0);
    }

    edit_repo_config(repo, gix::config::Source::Local, |config| {
        remove_all_local_ignored_paths(config)?;
        for path in &paths {
            ensure_config_value(config, LOCAL_IGNORED_PATH_KEY, path)?;
        }
        Ok(())
    })?;
    Ok(changed)
}

/// Remove locally ignored paths from visible worktree changes.
pub(crate) fn filter_locally_ignored_worktree_changes(
    mut changes: but_core::WorktreeChanges,
    locally_ignored_paths: &[String],
) -> but_core::WorktreeChanges {
    let ignored_paths: Vec<_> = locally_ignored_paths
        .iter()
        .filter_map(|path| normalize_local_ignore_path_string(path))
        .collect();
    if ignored_paths.is_empty() {
        return changes;
    }

    changes
        .changes
        .retain(|change| !path_is_locally_ignored(change.path.as_slice(), &ignored_paths));
    changes
        .ignored_changes
        .retain(|change| !path_is_locally_ignored(change.path.as_slice(), &ignored_paths));
    changes
        .index_conflicts
        .retain(|(path, _)| !path_is_locally_ignored(path.as_slice(), &ignored_paths));
    changes
}

fn open_local_config_for_reading(repo: &gix::Repository) -> Result<gix::config::File<'static>> {
    let path = repo.common_dir().join("config");
    if !path.exists() {
        return Ok(gix::config::File::new(gix::config::file::Metadata::from(
            gix::config::Source::Local,
        )));
    }
    gix::config::File::from_path_no_includes(path.clone(), gix::config::Source::Local)
        .with_context(|| format!("failed to open Local git config at {}", path.display()))
}

fn normalize_local_ignore_path(relative_path: &Path) -> Result<String> {
    if relative_path.as_os_str().is_empty() {
        bail!("locally ignored paths must not be empty");
    }

    let mut parts = Vec::new();
    for component in relative_path.components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .with_context(|| format!("path component is not valid UTF-8: {relative_path:?}"))?;
                if part.contains('/') || part.contains('\\') {
                    for nested in part.split(['/', '\\']).filter(|segment| !segment.is_empty()) {
                        parts.push(nested.to_owned());
                    }
                } else {
                    parts.push(part.to_owned());
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("locally ignored paths must be repository-relative")
            }
        }
    }

    if parts.is_empty() {
        bail!("locally ignored paths must not be empty");
    }
    Ok(parts.join("/"))
}

fn normalize_local_ignore_path_string(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            _ => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn remove_all_local_ignored_paths(config: &mut gix::config::File<'static>) -> Result<()> {
    let key = LOCAL_IGNORED_PATH_KEY
        .try_as_key()
        .with_context(|| format!("invalid git config key: {LOCAL_IGNORED_PATH_KEY}"))?;
    if let Ok(mut section) = config.section_mut(key.section_name, key.subsection_name) {
        while section.remove(key.value_name).is_some() {}
    }
    Ok(())
}

fn path_is_locally_ignored(path: &[u8], ignored_paths: &[String]) -> bool {
    let Some(normalized_path) =
        normalize_local_ignore_path_string(&String::from_utf8_lossy(path))
    else {
        return false;
    };

    ignored_paths.iter().any(|ignored_path| {
        normalized_path == *ignored_path || normalized_path.starts_with(&format!("{ignored_path}/"))
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::Result;
    use but_core::{
        ChangeState, IgnoredWorktreeChange, IgnoredWorktreeTreeChangeStatus, TreeChange,
        TreeStatus, WorktreeChanges,
    };
    use gix::{ObjectId, hash::Kind, object::tree::EntryKind};

    use super::*;

    #[test]
    fn local_ignored_paths_round_trip_in_local_git_config() -> Result<()> {
        let repo_dir = tempfile::tempdir()?;
        let repo = gix::init(repo_dir.path())?;

        assert_eq!(list_local_ignored_paths(&repo)?, Vec::<String>::new());

        assert!(set_local_ignored_path(
            &repo,
            Path::new(r"Assets\Scenes\dealers\LightingData.asset"),
            true,
        )?);
        assert!(set_local_ignored_path(
            &repo,
            Path::new("Assets/Generated"),
            true,
        )?);

        let ignored_paths = list_local_ignored_paths(&repo)?;
        assert_eq!(
            ignored_paths,
            vec![
                "Assets/Generated".to_owned(),
                "Assets/Scenes/dealers/LightingData.asset".to_owned(),
            ],
            "paths should be normalized to repo-relative forward-slash form and stored in sorted order"
        );

        assert!(set_local_ignored_path(
            &repo,
            Path::new("Assets/Generated"),
            false,
        )?);
        assert_eq!(
            list_local_ignored_paths(&repo)?,
            vec!["Assets/Scenes/dealers/LightingData.asset".to_owned()]
        );
        Ok(())
    }

    #[test]
    fn filter_locally_ignored_worktree_changes_hides_matching_paths_and_children() {
        let filtered = filter_locally_ignored_worktree_changes(
            WorktreeChanges {
                changes: vec![
                    tracked_change("Assets/Scenes/dealers.unity"),
                    tracked_change("Assets/Generated/NavMesh.asset"),
                    tracked_change("Assets/Generated/Subdir/Bake.asset"),
                    tracked_change("ProjectSettings/ProjectSettings.asset"),
                ],
                ignored_changes: vec![
                    ignored_change("Assets/Generated/Subdir/Bake.asset"),
                    ignored_change("ProjectSettings/EditorSettings.asset"),
                ],
                index_changes: Vec::new(),
                index_conflicts: Vec::new(),
            },
            &[
                "Assets/Generated".to_owned(),
                "Assets/Scenes/dealers.unity".to_owned(),
            ],
        );

        assert_eq!(
            filtered
                .changes
                .iter()
                .map(|change| change.path.to_string())
                .collect::<Vec<_>>(),
            vec!["ProjectSettings/ProjectSettings.asset".to_owned()],
            "direct file matches and directory children should be hidden from visible worktree changes"
        );
        assert_eq!(
            filtered
                .ignored_changes
                .iter()
                .map(|change| change.path.to_string())
                .collect::<Vec<_>>(),
            vec!["ProjectSettings/EditorSettings.asset".to_owned()],
            "the same path rules should apply to ignored worktree changes too"
        );
    }

    fn tracked_change(path: &str) -> TreeChange {
        TreeChange {
            path: path.into(),
            status: TreeStatus::Modification {
                previous_state: state(),
                state: state(),
                flags: None,
            },
        }
    }

    fn ignored_change(path: &str) -> IgnoredWorktreeChange {
        IgnoredWorktreeChange {
            path: path.into(),
            status: IgnoredWorktreeTreeChangeStatus::TreeIndex,
        }
    }

    fn state() -> ChangeState {
        ChangeState {
            id: ObjectId::null(Kind::Sha1),
            kind: EntryKind::Blob,
        }
    }
}

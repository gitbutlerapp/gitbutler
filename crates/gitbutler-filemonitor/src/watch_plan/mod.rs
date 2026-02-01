use std::{
    borrow::Cow,
    collections::{BTreeSet, HashSet},
    fs::FileType,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use gix::bstr::BStr;
use notify::RecursiveMode;

#[cfg(test)]
mod tests;

/// Traverse all unignored or tracked directories in `worktree_path` and pass them to `visit_dir(path, notify-mode)`,
/// which can decide to stop the iteration by returning `ControlFlow::Break`.
/// `git_dir` is used to select specific directories for recursive watching first,
/// and other contained `.git` directories are watched similarly.
#[tracing::instrument(skip(repo, visit_dir), level = "debug", err)]
pub(crate) fn compute_watch_plan_for_repo(
    repo: &gix::Repository,
    worktree_path: &Path,
    git_dir: &Path,
    mut visit_dir: impl FnMut(&Path, notify::RecursiveMode) -> anyhow::Result<ControlFlow<()>>,
) -> anyhow::Result<()> {
    let index = repo.index_or_empty()?;
    let icase_acc = build_index_icase_accelerator_if_needed(repo, &index);
    let mut excludes = repo.excludes(
        &index,
        None,
        gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
    )?;

    let mut seen = HashSet::new();
    let mut stack = Vec::new();

    // Prioritize watches for git-dir paths so we still get critical git events even when we hit
    // OS watch limits while adding worktree watches.
    if visit_dir(worktree_path, RecursiveMode::NonRecursive)?.is_break() {
        return Ok(());
    }
    if emit_git_dir_watches(git_dir, &mut visit_dir)?.is_break() {
        return Ok(());
    }

    seen.insert(worktree_path.to_owned());
    push_child_dirs_sorted(worktree_path, &mut stack);
    while let Some(dir) = stack.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        if dir.starts_with(git_dir) {
            continue;
        }
        let Ok(relative) = dir.strip_prefix(worktree_path) else {
            continue;
        };

        if !relative.as_os_str().is_empty() {
            if relative
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(gix::discover::DOT_GIT_DIR))
            {
                if emit_git_dir_watches(&dir, &mut visit_dir)?.is_break() {
                    continue;
                }
                if dir
                    .ancestors()
                    .filter_map(|d| d.file_name().and_then(|f| f.to_str()))
                    .any(|name| name.eq_ignore_ascii_case(gix::discover::DOT_GIT_DIR))
                {
                    continue;
                }
            }
            let is_excluded = excludes
                .at_path(relative, Some(gix::index::entry::Mode::DIR))
                .is_ok_and(|platform| platform.is_excluded());
            if is_excluded
                && !is_tracked_in_index(
                    to_repo_relative_path(relative).as_ref(),
                    true,
                    &index,
                    icase_acc.as_ref(),
                )
            {
                continue;
            }
        }
        if visit_dir(&dir, RecursiveMode::NonRecursive)?.is_break() {
            return Ok(());
        }

        push_child_dirs_sorted(&dir, &mut stack);
    }

    Ok(())
}

fn emit_git_dir_watches(
    git_dir: &Path,
    visit_dir: &mut impl FnMut(&Path, RecursiveMode) -> anyhow::Result<ControlFlow<()>>,
) -> anyhow::Result<ControlFlow<()>> {
    // Non-recursive to not pick up objects, while this will pick up changes to `HEAD`, `FETCH_HEAD`,
    // and other root-refs, as well as `.git/config`.
    if visit_dir(git_dir, RecursiveMode::NonRecursive)?.is_break() {
        return Ok(ControlFlow::Break(()));
    }
    let logs_dir = git_dir.join("logs");
    // This is non-recursive because we are only interested in `.git/logs/HEAD` which is changed when
    // `refs/heads/main` (or whatever `.git/HEAD` points to) is changed, affecting `HEAD`.
    if logs_dir.is_dir() && visit_dir(&logs_dir, RecursiveMode::NonRecursive)?.is_break() {
        return Ok(ControlFlow::Break(()));
    }
    let refs_heads_dir = git_dir.join("refs").join("heads");
    // For `.git/refs/heads`, the built-in watch mode is working well enough, and we have no facility
    // to auto-track newly added directories here. But if that would change, we could non-recursively
    // watch everything.
    if refs_heads_dir.is_dir() && visit_dir(&refs_heads_dir, RecursiveMode::Recursive)?.is_break() {
        return Ok(ControlFlow::Break(()));
    }
    Ok(ControlFlow::Continue(()))
}

fn push_child_dirs_sorted(dir: &Path, stack: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut children = BTreeSet::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if is_watchable_directory(file_type) {
            children.insert(entry.file_name());
        }
    }
    stack.extend(children.into_iter().map(|child_dir| dir.join(child_dir)));
}

pub(crate) fn is_watchable_directory(file_type: FileType) -> bool {
    file_type.is_dir() && !file_type.is_symlink()
}

pub(crate) fn build_index_icase_accelerator_if_needed<'index>(
    repo: &gix::Repository,
    index: &'index gix::index::State,
) -> Option<gix::index::AccelerateLookup<'index>> {
    repo.filesystem_options()
        .ok()
        .and_then(|opts| opts.ignore_case.then(|| index.prepare_icase_backing()))
}

/// `relative_path` is a `/` separated repo-relative path, which is considered a directory if
/// `is_dir` is `true`.
/// If `icase_acc` is set, the lookup will be case-insensitive.
pub(crate) fn is_tracked_in_index(
    relative_path: &BStr,
    is_dir: bool,
    index: &gix::index::State,
    icase_acc: Option<&gix::index::AccelerateLookup>,
) -> bool {
    if let Some(icase_acc) = icase_acc {
        if is_dir {
            index.path_is_directory_icase(relative_path, true, icase_acc)
        } else {
            index
                .entry_by_path_icase(relative_path, true, icase_acc)
                .is_some()
        }
    } else if is_dir {
        index.path_is_directory(relative_path)
    } else {
        index.entry_by_path(relative_path).is_some()
    }
}

pub(crate) fn to_repo_relative_path(path: &Path) -> Cow<'_, BStr> {
    gix::path::to_unix_separators_on_windows(gix::path::into_bstr(path))
}

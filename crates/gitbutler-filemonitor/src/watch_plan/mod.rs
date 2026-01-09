use gix::bstr::{BStr, ByteSlice};
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[tracing::instrument(skip(repo, visit), level = "debug", err)]
pub(crate) fn compute_watch_plan_for_repo(
    repo: &gix::Repository,
    worktree_path: &Path,
    git_dir: &Path,
    mut visit: impl FnMut(&Path, notify::RecursiveMode) -> anyhow::Result<ControlFlow<()>>,
) -> anyhow::Result<()> {
    let index = repo.index_or_empty()?;
    let tracked_dirs = tracked_worktree_dir_prefixes(&index);
    let mut excludes = repo.excludes(
        &index,
        None,
        gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
    )?;

    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    // Prioritize watches for git-dir paths so we still get critical git events even when we hit
    // OS watch limits while adding worktree watches.
    if visit(worktree_path, notify::RecursiveMode::NonRecursive)?.is_break() {
        return Ok(());
    }
    if visit(git_dir, notify::RecursiveMode::NonRecursive)?.is_break() {
        return Ok(());
    }
    let logs_dir = git_dir.join("logs");
    if logs_dir.is_dir() && visit(&logs_dir, notify::RecursiveMode::NonRecursive)?.is_break() {
        return Ok(());
    }
    let refs_heads_dir = git_dir.join("refs").join("heads");
    if refs_heads_dir.is_dir()
        && visit(&refs_heads_dir, notify::RecursiveMode::Recursive)?.is_break()
    {
        return Ok(());
    }

    visited.insert(worktree_path.to_owned());
    push_child_dirs(worktree_path, &mut stack);
    while let Some(dir) = stack.pop() {
        if !visited.insert(dir.clone()) {
            continue;
        }
        if dir.starts_with(git_dir) {
            continue;
        }
        let Ok(relative) = dir.strip_prefix(worktree_path) else {
            continue;
        };

        if !relative.as_os_str().is_empty() {
            let is_excluded = excludes
                .at_path(relative, Some(gix::index::entry::Mode::DIR))
                .is_ok_and(|platform| platform.is_excluded());
            if is_excluded && !tracked_dirs.contains(to_repo_relative_key(relative).as_ref()) {
                continue;
            }
        }
        if visit(&dir, notify::RecursiveMode::NonRecursive)?.is_break() {
            return Ok(());
        }

        push_child_dirs(&dir, &mut stack);
    }

    Ok(())
}

fn push_child_dirs(dir: &Path, stack: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() && !file_type.is_symlink() {
            stack.push(entry.path());
        }
    }
}

pub(crate) fn tracked_worktree_dir_prefixes(index: &gix::index::State) -> HashSet<&BStr> {
    let mut out = HashSet::new();
    out.insert(b"".as_bstr());

    for entry in index.entries() {
        let mut path = entry.path(index);
        while let Some(last_slash) = path.rfind_byte(b'/') {
            path = &path[..last_slash];
            if !out.insert(path) {
                break;
            }
        }
    }

    out
}

pub(crate) fn to_repo_relative_key(path: &Path) -> Cow<'_, BStr> {
    gix::path::to_unix_separators_on_windows(gix::path::into_bstr(path))
}

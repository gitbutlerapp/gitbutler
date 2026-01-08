use gix::bstr::{BString, ByteSlice};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[cfg(test)]
mod tests;

#[tracing::instrument(skip(repo), level = "debug", err)]
pub(crate) fn compute_watch_plan_for_repo(
    repo: &gix::Repository,
    worktree_path: &Path,
    git_dir: &Path,
) -> anyhow::Result<Vec<(PathBuf, notify::RecursiveMode)>> {
    let index = repo.index_or_empty()?;
    let tracked_dirs = tracked_worktree_dir_prefixes(&index);
    let mut excludes = repo.excludes(
        &index,
        None,
        gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
    )?;

    let mut watched = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![worktree_path.to_owned()];
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
        if relative
            .components()
            .any(|c| matches!(c, Component::Normal(name) if name == ".git"))
        {
            continue;
        }

        if !relative.as_os_str().is_empty() {
            let is_excluded = excludes
                .at_path(relative, Some(gix::index::entry::Mode::DIR))
                .is_ok_and(|platform| platform.is_excluded());
            if is_excluded && !tracked_dirs.contains(&to_repo_relative_key(relative)) {
                continue;
            }
        }
        watched.push((dir.clone(), notify::RecursiveMode::NonRecursive));

        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
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

    // Watch the git directory explicitly, while avoiding recursing into `.git/objects`.
    watched.push((git_dir.to_owned(), notify::RecursiveMode::NonRecursive));
    let logs_dir = git_dir.join("logs");
    if logs_dir.is_dir() {
        watched.push((logs_dir, notify::RecursiveMode::NonRecursive));
    }
    let refs_heads_dir = git_dir.join("refs").join("heads");
    if refs_heads_dir.is_dir() {
        watched.push((refs_heads_dir, notify::RecursiveMode::Recursive));
    }

    Ok(watched)
}

pub(crate) fn tracked_worktree_dir_prefixes(index: &gix::index::State) -> HashSet<BString> {
    let mut out = HashSet::new();
    out.insert(BString::default());

    for entry in index.entries() {
        let path = entry.path(index);
        let Some(last_slash) = path.rfind_byte(b'/') else {
            continue;
        };
        let mut dir = BString::from(path[..last_slash].to_vec());
        loop {
            if !out.insert(dir.clone()) {
                break;
            }
            let Some(next_slash) = dir.rfind_byte(b'/') else {
                break;
            };
            dir.truncate(next_slash);
        }
    }

    out
}

pub(crate) fn to_repo_relative_key(path: &Path) -> BString {
    gix::path::to_unix_separators_on_windows(gix::path::into_bstr(path)).into_owned()
}

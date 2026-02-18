use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum HooksPathValidationError {
    #[error("Failed to resolve repository worktree root '{path}': {source}")]
    ResolveWorktreeRoot {
        path: PathBuf,
        #[source]
        source: gix::path::realpath::Error,
    },
    #[error("Failed to resolve repository git directory '{path}': {source}")]
    ResolveGitDirRoot {
        path: PathBuf,
        #[source]
        source: gix::path::realpath::Error,
    },
    #[error("Configured core.hooksPath is empty")]
    EmptyConfiguredHooksPath,
    #[error("Failed to resolve hooks path '{path}': {source}")]
    ResolveHooksPath {
        path: PathBuf,
        #[source]
        source: gix::path::realpath::Error,
    },
    #[error("Resolved hooks path '{path}' is outside repository roots {roots:?}")]
    OutsideRepository { path: PathBuf, roots: Vec<PathBuf> },
}

pub(crate) fn path_within_repo_roots(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

pub(crate) fn repo_roots(repo: &git2::Repository) -> Result<Vec<PathBuf>, HooksPathValidationError> {
    let mut roots = Vec::new();

    if let Some(workdir) = repo.workdir() {
        let resolved_workdir =
            gix::path::realpath(workdir).map_err(|source| HooksPathValidationError::ResolveWorktreeRoot {
                path: workdir.to_path_buf(),
                source,
            })?;
        roots.push(resolved_workdir);
    }

    let git_dir = repo.path();
    let resolved_git_dir =
        gix::path::realpath(git_dir).map_err(|source| HooksPathValidationError::ResolveGitDirRoot {
            path: git_dir.to_path_buf(),
            source,
        })?;
    if !roots.iter().any(|root| root == &resolved_git_dir) {
        roots.push(resolved_git_dir);
    }

    Ok(roots)
}

pub(crate) fn resolve_safe_hooks_dir(repo: &git2::Repository) -> Result<PathBuf, HooksPathValidationError> {
    let roots = repo_roots(repo)?;
    let configured_hooks_dir = repo
        .config()
        .and_then(|config| config.get_path("core.hooksPath"))
        .unwrap_or_else(|_| repo.path().join("hooks"));

    if configured_hooks_dir.as_os_str().is_empty() {
        return Err(HooksPathValidationError::EmptyConfiguredHooksPath);
    }

    let hooks_dir = if configured_hooks_dir.is_absolute() {
        configured_hooks_dir
    } else {
        repo.workdir().unwrap_or_else(|| repo.path()).join(configured_hooks_dir)
    };

    let resolved_hooks_dir =
        gix::path::realpath(&hooks_dir).map_err(|source| HooksPathValidationError::ResolveHooksPath {
            path: hooks_dir,
            source,
        })?;

    if !path_within_repo_roots(&resolved_hooks_dir, &roots) {
        return Err(HooksPathValidationError::OutsideRepository {
            path: resolved_hooks_dir,
            roots,
        });
    }

    Ok(resolved_hooks_dir)
}

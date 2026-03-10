//! Repository discovery and repo-relative path normalization helpers.
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use crate::cli::{Cmd, normalize_claim_path};

/// Deduplicate and normalize already-resolved command paths.
pub(crate) fn normalized_unique_paths(paths: &[String]) -> Vec<String> {
    let mut seen = HashSet::with_capacity(paths.len());
    let mut out = Vec::with_capacity(paths.len());
    for raw in paths {
        let path = normalize_claim_path(raw)
            .expect("command paths must be normalized before reaching handlers");
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }
    out
}

/// Normalize an absolute path by collapsing `.` and `..`.
fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut saw_root = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                normalized.push(component.as_os_str());
                saw_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|component| matches!(component, Component::Normal(_)))
                {
                    normalized.pop();
                } else if !saw_root {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

/// Canonicalize the longest existing prefix of a path while preserving the suffix.
fn canonicalize_existing_prefix(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut suffix = Vec::new();

    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            break;
        };
        suffix.push(name.to_owned());
        let Some(parent) = existing.parent() else {
            break;
        };
        existing = parent;
    }

    let mut canonical = existing
        .canonicalize()
        .unwrap_or_else(|_| existing.to_path_buf());
    for component in suffix.iter().rev() {
        canonical.push(component);
    }
    canonical
}

/// Resolve an input path to a normalized repo-relative path.
pub(crate) fn resolve_repo_relative_path(
    raw: &str,
    current_dir: &Path,
    repo_root: &Path,
) -> anyhow::Result<String> {
    let candidate = if Path::new(raw).is_absolute() {
        canonicalize_existing_prefix(Path::new(raw))
    } else {
        current_dir.join(raw)
    };
    let normalized = normalize_absolute_path(&candidate);
    let relative = normalized
        .strip_prefix(repo_root)
        .map_err(|_| anyhow::anyhow!("path must stay within repository: {raw}"))?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    let normalized = normalize_claim_path(&relative)?;
    anyhow::ensure!(
        !normalized.is_empty(),
        "path must not resolve to the repository root"
    );
    Ok(normalized)
}

/// Resolve multiple input paths to normalized repo-relative paths.
fn resolve_repo_relative_paths(
    paths: Vec<String>,
    current_dir: &Path,
    repo_root: &Path,
) -> anyhow::Result<Vec<String>> {
    paths
        .into_iter()
        .map(|path| resolve_repo_relative_path(&path, current_dir, repo_root))
        .collect()
}

/// Normalize runtime paths for commands that accept repo paths.
pub(crate) fn normalize_command_paths(
    cmd: Cmd,
    current_dir: &Path,
    repo_root: &Path,
) -> anyhow::Result<Cmd> {
    Ok(match cmd {
        Cmd::Acquire {
            paths,
            ttl,
            strict,
            dry_run,
            format,
        } => Cmd::Acquire {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            ttl,
            strict,
            dry_run,
            format,
        },
        Cmd::Intent {
            scope,
            tags,
            surface,
            paths,
        } => Cmd::Intent {
            scope,
            tags,
            surface,
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
        },
        Cmd::Declare {
            scope,
            tags,
            surface,
            paths,
        } => Cmd::Declare {
            scope,
            tags,
            surface,
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
        },
        Cmd::Block {
            paths,
            reason,
            mode,
            ttl,
        } => Cmd::Block {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            reason,
            mode,
            ttl,
        },
        Cmd::Ack {
            target_agent_id,
            paths,
            note,
        } => Cmd::Ack {
            target_agent_id,
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            note,
        },
        other => other,
    })
}

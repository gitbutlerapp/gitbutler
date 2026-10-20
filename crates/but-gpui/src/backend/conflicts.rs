//! Resolving conflicts without leaving the workspace, a conflicted commit's or those a merge
//! left in uncommitted files: each keeps one side, the other, or text written for it.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context as _, Result};
use but_api::resolve::{self, HunkResolution, ResolutionSpec};

use super::ops::rewritten;
use super::{CommitId, Rewritten, checkout, markers, open};

/// What a conflict keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keep {
    NewBase,
    Commit,
    Text(String),
}

/// What resolving left.
pub struct Resolved {
    pub resolved: usize,
    /// Conflicts still to resolve, lines and whole files.
    pub remaining: usize,
    /// The resolutions dropped all the commit's changes.
    pub emptied: bool,
}

/// Resolve conflicts of `commit`, each by its file and its place among the file's conflicts.
/// The rest stay conflicted. The commit is rewritten either way.
pub fn resolve(
    path: &Path,
    commit: CommitId,
    keep: Vec<(String, usize, Keep)>,
) -> Result<(Rewritten, Resolved)> {
    let specs = (keep.into_iter())
        .map(|(file, index, keep)| ResolutionSpec {
            path: file,
            hunk: index + 1,
            resolution: match keep {
                Keep::NewBase => HunkResolution::Ours,
                Keep::Commit => HunkResolution::Theirs,
                Keep::Text(text) => HunkResolution::Content(text),
            },
        })
        .collect();
    let outcome = resolve::resolve_commit_conflict_hunks(&mut open(path)?, commit.0, specs)?;
    let mut rewrites = rewritten(&outcome.workspace);
    rewrites.insert(commit, CommitId(outcome.new_commit));
    let remaining = (outcome.remaining.iter())
        .map(|file| file.hunks)
        .sum::<usize>()
        + outcome.manual.len();
    Ok((
        rewrites,
        Resolved {
            resolved: outcome.resolved,
            remaining,
            emptied: outcome.commit_emptied,
        },
    ))
}

/// Resolve conflicts a merge left in uncommitted files of the checkout `worktree` names, or
/// the main one, each by its file and id: what's kept replaces the conflict's markers, and a
/// file with none left is marked resolved. Returns how many files were. Recorded for undo
/// first in the main checkout, as discarding is; the oplog can't restore a linked worktree,
/// nor a commit checked out for editing.
pub fn resolve_markers(
    path: &Path,
    worktree: Option<&str>,
    keep: Vec<(String, String, Keep)>,
) -> Result<usize> {
    use but_oplog::legacy::OperationKind;
    let mut ctx = open(path)?;
    // Asked before taking the worktree, as asking takes it too.
    let editing = super::edit::editing(&ctx)?.is_some();
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let repo = checkout(&ctx, worktree)?;
    let workdir = repo
        .workdir()
        .context("The checkout has no files")?
        .to_owned();
    let mut by_file: BTreeMap<String, Vec<(String, Keep)>> = BTreeMap::new();
    for (file, id, keep) in keep {
        by_file.entry(file).or_default().push((id, keep));
    }
    // Every file's picks are found before any is written: a file changed since its conflicts
    // were shown stops them all, rather than leave the ones before it half done.
    let mut writes = Vec::new();
    for (file, picks) in by_file {
        let on_disk = workdir.join(&file);
        let text =
            std::fs::read_to_string(&on_disk).with_context(|| format!("Couldn't read {file}"))?;
        let lines = markers::lines(&text);
        let blocks = markers::blocks(&lines);
        let ids = markers::ids(&lines, &blocks);
        let mut kept = HashMap::new();
        for (id, keep) in picks {
            let ix = (ids.iter().position(|shown| *shown == id)).with_context(|| {
                format!("{file} changed since its conflicts were shown. Pick again.")
            })?;
            let block = &blocks[ix];
            let text = match keep {
                Keep::NewBase => markers::text(&lines, block.ours.clone()),
                Keep::Commit => markers::text(&lines, block.theirs.clone()),
                Keep::Text(text) => text,
            };
            kept.insert(ix, text);
        }
        let whole = kept.len() == blocks.len();
        writes.push((
            file,
            on_disk,
            markers::replace(&lines, &blocks, &kept),
            whole,
        ));
    }
    // Recorded once the picks are found, so a refused Apply leaves nothing to undo.
    if worktree.is_none() && !editing {
        super::ops::record_undo(&ctx, OperationKind::ResolveConflicts, perm);
    }
    let mut resolved = Vec::new();
    for (file, on_disk, text, whole) in writes {
        std::fs::write(&on_disk, text)?;
        if whole {
            resolved.push(file.into());
        }
    }
    let count = resolved.len();
    if count > 0 {
        but_workspace::resolve_worktree_conflicts(&repo, resolved)?;
    }
    Ok(count)
}

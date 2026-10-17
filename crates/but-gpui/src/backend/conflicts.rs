//! Resolving a conflicted commit's conflicts without leaving the workspace: each one keeps
//! what its new base has there, what the commit has, or text written for it.

use std::path::Path;

use anyhow::Result;
use but_api::resolve::{self, HunkResolution, ResolutionSpec};

use super::ops::rewritten;
use super::{CommitId, Rewritten, open};

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

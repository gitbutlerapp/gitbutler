//! A conflicted commit's conflicts, and resolving them hunk by hunk without leaving the
//! workspace. "Current" is what the commit now sits on, "incoming" the commit's own
//! change, as in a rebase.

use std::path::Path;

use anyhow::Result;
use but_api::resolve::{self, HunkResolution, ResolutionSpec};

use super::ops::rewritten;
use super::{CommitId, Rewritten, open};

#[derive(Debug, Clone)]
pub struct Conflicts {
    pub files: Vec<ConflictedFile>,
    /// Files with no hunks to pick between, like a side deleting the file, each with why.
    /// Only edit mode can resolve those.
    pub manual: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct ConflictedFile {
    pub path: String,
    pub hunks: Vec<ConflictHunk>,
}

#[derive(Debug, Clone)]
pub struct ConflictHunk {
    pub before: String,
    pub current: String,
    pub incoming: String,
    pub after: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Current,
    Incoming,
    Both,
}

pub fn conflicts(path: &Path, commit: CommitId) -> Result<Conflicts> {
    let found = resolve::commit_conflicts(&open(path)?, commit.0)?;
    Ok(Conflicts {
        files: found
            .files
            .into_iter()
            .map(|file| ConflictedFile {
                path: file.path,
                hunks: file
                    .hunks
                    .into_iter()
                    .map(|hunk| ConflictHunk {
                        before: hunk.context_before,
                        current: hunk.ours,
                        incoming: hunk.theirs,
                        after: hunk.context_after,
                    })
                    .collect(),
            })
            .collect(),
        manual: found
            .manual
            .into_iter()
            .map(|file| (file.path, file.reason))
            .collect(),
    })
}

/// What resolving left.
pub struct Resolved {
    pub resolved: usize,
    /// Conflicts still to resolve, hunks and whole files.
    pub remaining: usize,
    /// The resolutions dropped all the commit's changes.
    pub emptied: bool,
}

/// Resolve the picked hunks of `commit`, each by path and 0-based index. The rest stay
/// conflicted. The commit is rewritten either way.
pub fn resolve(
    path: &Path,
    commit: CommitId,
    picks: &[(String, usize, Pick)],
    hunks: &Conflicts,
) -> Result<(Rewritten, Resolved)> {
    let specs = picks
        .iter()
        .map(|(file, ix, pick)| {
            let resolution = match pick {
                Pick::Current => HunkResolution::Ours,
                Pick::Incoming => HunkResolution::Theirs,
                Pick::Both => {
                    let hunk = hunks
                        .files
                        .iter()
                        .find(|f| f.path == *file)
                        .and_then(|f| f.hunks.get(*ix));
                    HunkResolution::Content(hunk.map_or_else(String::new, |hunk| {
                        let mut both = hunk.current.clone();
                        if !both.is_empty() && !both.ends_with('\n') {
                            both.push('\n');
                        }
                        both + &hunk.incoming
                    }))
                }
            };
            ResolutionSpec {
                path: file.clone(),
                hunk: ix + 1,
                resolution,
            }
        })
        .collect();
    let outcome = resolve::resolve_commit_conflict_hunks(&mut open(path)?, commit.0, specs)?;
    let mut rewrites = rewritten(&outcome.workspace);
    rewrites.insert(commit, CommitId(outcome.new_commit));
    let remaining = outcome
        .remaining
        .iter()
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

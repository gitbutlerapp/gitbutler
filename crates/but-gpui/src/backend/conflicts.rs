//! A conflicted commit's conflicts, and resolving them hunk by hunk without leaving the
//! workspace. The app calls a hunk's "ours" side current, what the commit now sits on, and
//! its "theirs" side incoming, the commit's own change, as in a rebase.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use but_api::resolve::{self, HunkResolution, ResolutionSpec};
pub use but_api::resolve::{CommitConflicts, ConflictHunk};

use super::ops::rewritten;
use super::{CommitId, Rewritten, open};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Current,
    Incoming,
    Both,
    /// Let the AI set up in Settings merge the two.
    Ai,
}

pub fn conflicts(path: &Path, commit: CommitId) -> Result<CommitConflicts> {
    resolve::commit_conflicts(&open(path)?, commit.0)
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
    picks: &HashMap<(String, usize), Pick>,
    hunks: &CommitConflicts,
) -> Result<(Rewritten, Resolved)> {
    let specs = picks
        .iter()
        .map(|((file, ix), pick)| {
            let resolution = match pick {
                Pick::Ai => HunkResolution::Ai,
                Pick::Current => HunkResolution::Ours,
                Pick::Incoming => HunkResolution::Theirs,
                Pick::Both => {
                    let hunk = hunks
                        .files
                        .iter()
                        .find(|f| f.path == *file)
                        .and_then(|f| f.hunks.get(*ix));
                    HunkResolution::Content(hunk.map_or_else(String::new, |hunk| {
                        let mut both = hunk.ours.clone();
                        if !both.is_empty() && !both.ends_with('\n') {
                            both.push('\n');
                        }
                        both + &hunk.theirs
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

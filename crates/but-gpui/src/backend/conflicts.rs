//! Resolving conflicts without leaving the workspace, a conflicted commit's or those a merge
//! left in uncommitted files: each keeps one side, the other, or text written for it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context as _, Result};
use bstr::{BString, ByteSlice as _};
use but_api::resolve::{self, HunkResolution, ResolutionSpec};

use super::ops::rewritten;
use super::{CommitId, Rewritten, checkout, markers, open};

/// What a conflict keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keep {
    NewBase,
    Commit,
    Text(BString),
}

/// What resolving left.
pub struct Resolved {
    pub resolved: usize,
    /// Conflicts still to resolve, lines and whole files.
    pub remaining: usize,
    /// The resolutions dropped all the commit's changes.
    pub emptied: bool,
    /// The commits above it that its new changes left conflicted, by their new ids.
    pub conflicted: HashSet<CommitId>,
}

/// Resolve conflicts of `commit`, each by its file and its place among the file's conflicts.
/// The rest stay conflicted. The commit is rewritten either way.
pub fn resolve(
    path: &Path,
    commit: CommitId,
    keep: Vec<(String, usize, Keep)>,
) -> Result<(Rewritten, Resolved)> {
    let specs = (keep.into_iter())
        .map(|(file, index, keep)| {
            let resolution = match keep {
                Keep::NewBase => HunkResolution::Ours,
                Keep::Commit => HunkResolution::Theirs,
                Keep::Text(text) => HunkResolution::Content(
                    String::from_utf8(text.into())
                        .with_context(|| format!("What {file} keeps isn't UTF-8 text"))?,
                ),
            };
            Ok(ResolutionSpec {
                path: file,
                hunk: index + 1,
                resolution,
            })
        })
        .collect::<Result<_>>()?;
    let mut ctx = open(path)?;
    // Rewriting the commit checks the workspace out again, which Git can't do over a file a
    // merge left conflicted, and Git's own words for that name none.
    {
        let repo = ctx.repo.get()?;
        let index = repo.index_or_empty()?;
        if let Some(entry) = (index.entries().iter())
            .find(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
        {
            anyhow::bail!(
                "Resolve the conflicts in {} first: applying checks the workspace out again, \
                 which Git can't do over a conflicted file",
                entry.path(&index)
            );
        }
    }
    let outcome = resolve::resolve_commit_conflict_hunks(&mut ctx, commit.0, specs)?;
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
            conflicted: (outcome.workspace.conflicted_commits.iter())
                .map(|(id, _)| CommitId(*id))
                .collect(),
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
    worktree: Option<&bstr::BStr>,
    keep: Vec<(BString, String, Keep)>,
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
    let mut by_file: BTreeMap<BString, Vec<(String, Keep)>> = BTreeMap::new();
    for (file, id, keep) in keep {
        by_file.entry(file).or_default().push((id, keep));
    }
    // Every file's picks are found before any is written: a file changed since its conflicts
    // were shown stops them all, rather than leave the ones before it half done.
    let mut writes = Vec::new();
    for (file, picks) in by_file {
        let on_disk = workdir.join(gix::path::from_bstr(file.as_bstr())?);
        let text = std::fs::read(&on_disk).with_context(|| format!("Couldn't read {file}"))?;
        let lines = markers::lines(&text);
        let blocks = markers::blocks(&lines);
        let ids = markers::ids(&lines, &blocks);
        let mut kept = HashMap::new();
        for (id, keep) in picks {
            let (ix, block) = (ids.iter().zip(&blocks).enumerate())
                .find_map(|(ix, (shown, block))| (*shown == id).then_some((ix, block)))
                .with_context(|| {
                    format!("{file} changed since its conflicts were shown. Pick again.")
                })?;
            let text = match keep {
                Keep::NewBase => markers::text(&lines, block.ours.clone()),
                Keep::Commit => markers::text(&lines, block.theirs.clone()),
                Keep::Text(text) => text,
            };
            kept.insert(ix, text);
        }
        let whole = kept.len() == blocks.len();
        let text = markers::replace(&lines, &blocks, &kept);
        // JSON, as a notebook is, that keeps both sides of a conflict can stop parsing, which
        // nothing shows until it's opened: refused, while the file parses with one side kept.
        if whole && (file.ends_with(b".json") || file.ends_with(b".ipynb")) {
            let parses = |text: &[u8]| serde_json::from_slice::<serde_json::Value>(text);
            let ours = (blocks.iter().enumerate())
                .map(|(ix, block)| (ix, markers::text(&lines, block.ours.clone())))
                .collect();
            if parses(&markers::replace(&lines, &blocks, &ours)).is_ok()
                && let Err(err) = parses(&text)
            {
                anyhow::bail!(
                    "That leaves {file} not valid JSON ({err}). Keep one side, or edit it."
                );
            }
        }
        writes.push((file, on_disk, text, whole));
    }
    // Recorded once the picks are found, so a refused Apply leaves nothing to undo.
    if worktree.is_none() && !editing {
        super::ops::record_undo(&ctx, OperationKind::ResolveConflicts, perm);
    }
    let mut resolved = Vec::new();
    for (file, on_disk, text, whole) in writes {
        std::fs::write(&on_disk, text)?;
        if whole {
            resolved.push(file);
        }
    }
    let count = resolved.len();
    if count > 0 {
        but_workspace::resolve_worktree_conflicts(&repo, resolved)?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::{Keep, markers, resolve_markers};
    use crate::backend::ops::tests::{git, repo_with};

    #[test]
    fn keeping_both_sides_of_a_json_conflict_is_refused_when_it_stops_parsing() {
        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            let write = |side: &str| {
                std::fs::write(path.join("cell.json"), format!("[\n  \"{side}\"\n]\n")).unwrap()
            };
            write("a");
            git(path, &["add", "cell.json"]);
            git(path, &["commit", "-qm", "cell"]);
            git(path, &["checkout", "-qb", "theirs"]);
            write("b");
            git(path, &["commit", "-qam", "b"]);
            git(path, &["checkout", "-q", "main"]);
            write("c");
            git(path, &["commit", "-qam", "c"]);
            let merged = (Command::new("git").current_dir(path))
                .args(["merge", "-q", "theirs"])
                .output()
                .unwrap();
            assert!(!merged.status.success(), "the merge conflicts");

            let text = std::fs::read(path.join("cell.json")).unwrap();
            let lines = markers::lines(&text);
            let blocks = markers::blocks(&lines);
            let id = markers::ids(&lines, &blocks)[0].clone();
            let mut both = markers::text(&lines, blocks[0].ours.clone());
            both.extend_from_slice(&markers::text(&lines, blocks[0].theirs.clone()));
            let err = resolve_markers(
                path,
                None,
                vec![("cell.json".into(), id.clone(), Keep::Text(both))],
            )
            .expect_err("two array items with no comma between them");
            assert!(err.to_string().contains("not valid JSON"), "{err}");
            assert_eq!(
                std::fs::read(path.join("cell.json")).unwrap(),
                text,
                "nothing is written"
            );
            let kept = resolve_markers(path, None, vec![("cell.json".into(), id, Keep::NewBase)]);
            assert_eq!(
                kept.unwrap(),
                1,
                "one side parses, and the file is resolved"
            );
        })
    }
}

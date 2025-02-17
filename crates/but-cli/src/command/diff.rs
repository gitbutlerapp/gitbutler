use crate::command::{debug_print, project_from_path, project_repo};
use gix::bstr::BString;
use itertools::Itertools;
use std::path::Path;

pub fn commit_changes(
    current_dir: &Path,
    current_commit: &str,
    previous_commit: Option<&str>,
    unified_diff: bool,
) -> anyhow::Result<()> {
    let repo = project_repo(current_dir)?;
    let previous_commit = previous_commit
        .map(|revspec| repo.rev_parse_single(revspec))
        .transpose()?;
    let commit = repo.rev_parse_single(current_commit)?;
    let changes =
        but_core::diff::commit_changes(&repo, previous_commit.map(Into::into), commit.into())?;

    if unified_diff {
        debug_print(unified_diff_for_changes(&repo, changes)?)
    } else {
        debug_print(changes)
    }
}

pub fn status(current_dir: &Path, unified_diff: bool) -> anyhow::Result<()> {
    let repo = project_repo(current_dir)?;
    let worktree = but_core::diff::worktree_changes(&repo)?;
    if unified_diff {
        debug_print((
            unified_diff_for_changes(&repo, worktree.changes)?,
            worktree.ignored_changes,
        ))
    } else {
        debug_print(worktree)
    }
}

pub fn locks(current_dir: &Path) -> anyhow::Result<()> {
    let project = project_from_path(current_dir)?;
    let repo = gix::open(project.worktree_path())?;
    let worktree_changes = but_core::diff::worktree_changes(&repo)?;
    let input_stacks = but_hunk_dependency::workspace_stacks_to_input_stacks(
        &repo,
        &but_workspace::stacks(&project.gb_dir())?,
        but_workspace::common_merge_base_with_target_branch(&project.gb_dir())?,
    )?;
    let ranges = but_hunk_dependency::WorkspaceRanges::try_from_stacks(input_stacks)?;
    debug_print(intersect_workspace_ranges(
        &repo,
        ranges,
        worktree_changes.changes,
    )?)
}

fn unified_diff_for_changes(
    repo: &gix::Repository,
    changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<Vec<(but_core::TreeChange, but_core::UnifiedDiff)>> {
    changes
        .into_iter()
        .map(|tree_change| {
            tree_change
                .unified_diff(repo, 3)
                .map(|diff| (tree_change, diff))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn intersect_workspace_ranges(
    repo: &gix::Repository,
    ranges: but_hunk_dependency::WorkspaceRanges,
    worktree_changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<LockInfo> {
    let mut intersections_by_path = Vec::new();
    let mut missed_hunks = Vec::new();
    for change in worktree_changes {
        let unidiff = change.unified_diff(repo, 0)?;
        let but_core::UnifiedDiff::Patch { hunks } = unidiff else {
            continue;
        };
        let mut intersections = Vec::new();
        for hunk in hunks {
            if let Some(hunk_ranges) =
                ranges.intersection(&change.path, hunk.old_start, hunk.old_lines)
            {
                intersections.push(HunkIntersection {
                    hunk,
                    commit_intersections: hunk_ranges.into_iter().copied().collect(),
                });
            } else {
                missed_hunks.push((change.path.clone(), hunk));
            }
        }
        if !intersections.is_empty() {
            intersections_by_path.push((change.path, intersections));
        }
    }

    Ok(LockInfo {
        intersections_by_path,
        missed_hunks,
        ranges_by_path: ranges
            .ranges_by_path_map()
            .iter()
            .sorted_by(|a, b| a.0.cmp(b.0))
            .map(|(path, ranges)| (path.to_owned(), ranges.to_vec()))
            .collect(),
    })
}

/// A structure that has stable content so it can be asserted on, showing the hunk-ranges that intersect with each of the input ranges.
#[derive(Debug)]
#[allow(dead_code)]
pub struct LockInfo {
    /// All available ranges for a tracked path, basically all changes seen over a set of commits.
    pub ranges_by_path: Vec<(BString, Vec<but_hunk_dependency::HunkRange>)>,
    /// The ranges that intersected with an input hunk.
    pub intersections_by_path: Vec<(BString, Vec<HunkIntersection>)>,
    /// Hunks that didn't have a matching intersection, with the filepath mentioned per hunk as well.
    pub missed_hunks: Vec<(BString, but_core::unified_diff::DiffHunk)>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct HunkIntersection {
    /// The hunk that was used for the intersection.
    pub hunk: but_core::unified_diff::DiffHunk,
    /// The hunks that touch `hunk` in the commit-diffs.
    pub commit_intersections: Vec<but_hunk_dependency::HunkRange>,
}

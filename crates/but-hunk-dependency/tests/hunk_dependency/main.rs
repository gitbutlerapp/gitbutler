use but_core::{TreeStatusKind, UnifiedDiff};
use but_hunk_dependency::{HunkRange, InputCommit, InputDiffHunk, InputFile, InputStack};
use gix::bstr::BString;

#[test]
fn change_2_to_two_in_second_commit() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_two")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: It should find a commit, but doesn't.
    insta::assert_debug_snapshot!(&digest, @r#"
    WorkspaceDigest {
        intersections_by_path: [],
        missed_hunks: [
            (
                "file",
                DiffHunk {
                    old_start: 2,
                    old_lines: 1,
                    new_start: 2,
                    new_lines: 1,
                    diff: "@@ -2,1 +2,1 @@\n-2\n\n+two\n\n",
                },
            ),
        ],
    }
    "#);
    Ok(())
}

#[test]
fn change_2_to_two_in_second_commit_after_file_rename() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_renamed-two")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: It should find a commit, but doesn't.
    insta::assert_debug_snapshot!(&digest, @r#"
    WorkspaceDigest {
        intersections_by_path: [],
        missed_hunks: [
            (
                "file-renamed",
                DiffHunk {
                    old_start: 2,
                    old_lines: 1,
                    new_start: 2,
                    new_lines: 1,
                    diff: "@@ -2,1 +2,1 @@\n-2\n\n+two\n\n",
                },
            ),
        ],
    }
    "#);
    Ok(())
}

#[test]
fn change_2_to_two_in_second_commit_after_shift_by_two() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10-shift_two")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: It should find a commit, but doesn't.
    insta::assert_debug_snapshot!(&digest, @r#"
    WorkspaceDigest {
        intersections_by_path: [],
        missed_hunks: [
            (
                "file",
                DiffHunk {
                    old_start: 4,
                    old_lines: 1,
                    new_start: 4,
                    new_lines: 1,
                    diff: "@@ -4,1 +4,1 @@\n-2\n\n+two\n\n",
                },
            ),
        ],
    }
    "#);
    Ok(())
}

#[test]
fn add_single_line() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_add-five")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: It should find a commit, but doesn't.
    insta::assert_debug_snapshot!(&digest, @r#"
    WorkspaceDigest {
        intersections_by_path: [],
        missed_hunks: [
            (
                "file",
                DiffHunk {
                    old_start: 6,
                    old_lines: 0,
                    new_start: 6,
                    new_lines: 1,
                    diff: "@@ -6,0 +6,1 @@\n+5.5\n\n",
                },
            ),
        ],
    }
    "#);
    Ok(())
}

#[test]
fn remove_single_line() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_remove-five")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: this commit is the parent of the one it should have found, and doesn't even contain `5` yet.
    insta::assert_debug_snapshot!(&digest, @r#"
    WorkspaceDigest {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk {
                            old_start: 5,
                            old_lines: 1,
                            new_start: 5,
                            new_lines: 0,
                            diff: "@@ -5,1 +5,0 @@\n-5\n\n",
                        },
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(057464a39d876b24320b23690f91bfc6de697b0a),
                                start: 5,
                                lines: 1,
                                line_shift: 1,
                            },
                        ],
                    },
                ],
            ),
        ],
        missed_hunks: [],
    }
    "#);
    Ok(())
}

fn repo(name: &str) -> anyhow::Result<gix::Repository> {
    let worktree_dir = gix_testtools::scripted_fixture_read_only("branch-states.sh")
        .map_err(anyhow::Error::from_boxed)?
        .join(name);
    Ok(gix::open_opts(
        worktree_dir,
        gix::open::Options::isolated(),
    )?)
}

/// A structure that has stable content so it can be asserted on, showing the hunk-ranges that intersect with each of the input ranges.
#[derive(Debug)]
#[allow(dead_code)]
struct WorkspaceDigest {
    intersections_by_path: Vec<(BString, Vec<HunkIntersection>)>,
    /// Hunks that didn't have a matching intersection, with the filepath mentioned per hunk as well.
    missed_hunks: Vec<(BString, but_core::unified_diff::DiffHunk)>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct HunkIntersection {
    /// The hunk that was used for the intersection.
    hunk: but_core::unified_diff::DiffHunk,
    /// The hunks that touch `hunk` in the commit-diffs.
    commit_intersections: Vec<StableHunkRange>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct StableHunkRange {
    change_type: TreeStatusKind,
    commit_id: gix::ObjectId,
    start: u32,
    lines: u32,
    line_shift: i32,
}

impl From<HunkRange> for StableHunkRange {
    fn from(
        HunkRange {
            change_type,
            stack_id: _,
            commit_id,
            start,
            lines,
            line_shift,
        }: HunkRange,
    ) -> Self {
        StableHunkRange {
            change_type,
            commit_id,
            start,
            lines,
            line_shift,
        }
    }
}

fn workspace_ranges_digest_for_worktree_changes(
    repo: &gix::Repository,
) -> anyhow::Result<WorkspaceDigest> {
    let input_stack = branch_input_stack(repo, "HEAD")?;
    let ranges = but_hunk_dependency::WorkspaceRanges::try_from_stacks(input_stack)?;
    let worktree_changes = but_core::diff::worktree_changes(repo)?.changes;
    intersect_workspace_ranges(repo, ranges, worktree_changes)
}

/// Returns the simulated Stack for a single branch.
fn branch_input_stack(repo: &gix::Repository, branch: &str) -> anyhow::Result<Vec<InputStack>> {
    let branch_tip = repo.rev_parse_single(branch)?;

    let mut commits = Vec::new();
    for commit in branch_tip.ancestors().all()? {
        let commit = commit?;
        assert!(
            commit.parent_ids().count() < 2,
            "For now we probably can't handle the non-linear case correctly"
        );
        let commit_changes = but_core::diff::commit_changes(
            repo,
            commit.parent_ids.iter().next().copied(),
            commit.id,
        )?;

        let mut files = Vec::new();
        for change in commit_changes {
            let diff = change.unified_diff(repo, 0)?;
            let UnifiedDiff::Patch { hunks } = diff else {
                unreachable!("Test repos don't have file-size issuse")
            };
            let change_type = change.status.kind();
            files.push(InputFile {
                path: change.path,
                hunks: hunks.iter().map(InputDiffHunk::from_unified_diff).collect(),
                change_type,
            })
        }
        let commit = InputCommit {
            commit_id: commit.id,
            files,
        };
        commits.push(commit);
    }
    let stack = InputStack {
        stack_id: Default::default(),
        commits,
    };
    Ok(vec![stack])
}

fn intersect_workspace_ranges(
    repo: &gix::Repository,
    ranges: but_hunk_dependency::WorkspaceRanges,
    worktree_changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<WorkspaceDigest> {
    let mut intersections_by_path = Vec::new();
    let mut missed_hunks = Vec::new();
    for change in worktree_changes {
        let unidiff = change.unified_diff(repo, 0)?;
        let UnifiedDiff::Patch { hunks } = unidiff else {
            continue;
        };
        let mut intersections = Vec::new();
        for hunk in hunks {
            if let Some(hunk_ranges) =
                ranges.intersection(&change.path, hunk.old_start, hunk.old_lines)
            {
                let hunk_ranges: Vec<_> =
                    hunk_ranges.into_iter().copied().map(Into::into).collect();
                intersections.push(HunkIntersection {
                    hunk,
                    commit_intersections: hunk_ranges,
                });
            } else {
                missed_hunks.push((change.path.clone(), hunk));
            }
        }
        if !intersections.is_empty() {
            intersections_by_path.push((change.path, intersections));
        }
    }
    Ok(WorkspaceDigest {
        intersections_by_path,
        missed_hunks,
    })
}

#[test]
fn change_2_to_two_in_second_commit() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_two")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: not sure about the line shift, expected to be 0.
    insta::assert_debug_snapshot!(digest.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -2,1 +2,1 @@
                        -2
                        +two
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(ca5567e4be81f1ee69b3d5ac5410d5010bcea756),
                                start: 2,
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

#[test]
fn change_2_to_two_in_second_commit_after_file_rename() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_renamed-two")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: It should find a commit, probably due to rename tracking - it would have to start searching for the new path name.
    insta::assert_debug_snapshot!(digest.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [],
        missed_hunks: [
            (
                "file-renamed",
                DiffHunk("@@ -2,1 +2,1 @@
                -2
                +two
                "),
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
    // TODO: It finds the correct commit, but `line_shift` should be 2.
    insta::assert_debug_snapshot!(digest.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -4,1 +4,1 @@
                        -2
                        +two
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(ca5567e4be81f1ee69b3d5ac5410d5010bcea756),
                                start: 4,
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

#[test]
fn add_single_line() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_add-five")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: It finds the right commit, but the hunk numbers seem off, should be inserted at line 6, and it's just one line with no `line_shift`.
    insta::assert_debug_snapshot!(digest.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -6,0 +6,1 @@
                        +5.5
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(ab311c05e6ca309fb01bcba46e9ab6ba652e0012),
                                start: 4,
                                lines: 7,
                                line_shift: 7,
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

#[test]
fn remove_single_line() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_remove-five")?;
    let digest = workspace_ranges_digest_for_worktree_changes(&repo)?;
    // TODO: It finds the right commit, but the hunk numbers seem off, removal should be at 5, without line shift.
    insta::assert_debug_snapshot!(digest.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -5,1 +5,0 @@
                        -5
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(ab311c05e6ca309fb01bcba46e9ab6ba652e0012),
                                start: 4,
                                lines: 7,
                                line_shift: 7,
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

mod util {
    use but_core::ref_metadata::StackId;
    use but_hunk_dependency::{InputCommit, InputStack, tree_changes_to_input_files};

    use crate::{WorkspaceDigest, intersect_workspace_ranges};

    pub fn repo(name: &str) -> anyhow::Result<gix::Repository> {
        let worktree_dir = gix_testtools::scripted_fixture_read_only("branch-states.sh")
            .map_err(anyhow::Error::from_boxed)?
            .join(name);
        Ok(gix::open_opts(
            worktree_dir,
            gix::open::Options::isolated(),
        )?)
    }

    pub fn workspace_ranges_digest_for_worktree_changes(
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
            let commit_changes = but_core::diff::tree_changes(
                repo,
                commit.parent_ids.iter().next().copied(),
                commit.id,
            )?;

            let files = tree_changes_to_input_files(repo, commit_changes)?;
            let commit = InputCommit {
                commit_id: commit.id,
                files,
            };
            commits.push(commit);
        }
        commits.reverse();
        let stack = InputStack {
            stack_id: StackId::generate(),
            commits_from_base_to_tip: commits,
        };
        Ok(vec![stack])
    }
}
use util::{repo, workspace_ranges_digest_for_worktree_changes};

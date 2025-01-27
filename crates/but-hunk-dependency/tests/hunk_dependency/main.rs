use but_core::UnifiedDiff;
use but_hunk_dependency::{InputCommit, InputDiffHunk, InputFile, InputStack};

#[test]
fn change_2_to_two_in_second_commit() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10_two")?;
    let worktree_changes = but_core::diff::worktree_changes(&repo)?.changes;
    let stacks = branch_input_stack(&repo, "HEAD")?;
    let res = but_hunk_dependency::calculate(worktree_changes, stacks, &repo)?;
    dbg!(&res);
    Ok(())
}

#[test]
fn change_2_to_two_in_second_commit_after_shift_by_two() -> anyhow::Result<()> {
    let repo = repo("1-2-3-10-shift_two")?;
    let worktree_changes = but_core::diff::worktree_changes(&repo)?.changes;
    let stacks = branch_input_stack(&repo, "HEAD")?;
    let res = but_hunk_dependency::calculate(worktree_changes, stacks, &repo)?;
    dbg!(&res);
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
            &repo,
            commit.parent_ids.iter().next().copied(),
            commit.id,
        )?;

        let mut files = Vec::new();
        for change in commit_changes {
            let diff = change.unified_diff(repo)?;
            let UnifiedDiff::Patch { hunks } = diff else {
                unreachable!("Test repos don't have file-size issuse")
            };
            let status_kind = change.status.kind();
            files.push(InputFile {
                path: gix::path::from_bstring(change.path),
                hunks: hunks
                    .iter()
                    .map(|hunk| InputDiffHunk::from_unified_diff(hunk, status_kind))
                    .collect(),
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

use anyhow::Result;
use but_core::{DiffSpec, DryRun, HunkHeader};

use crate::support::{checkout_branch_in_linked_worktree, repo_with_feature_branch, write_file};

#[test]
fn strict_worktree_amend_rejects_before_materializing() -> Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    let worktree_dir = linked.path().join("wt");
    write_file(&worktree_dir, "file.txt", "linked worktree change\n")?;
    write_file(&worktree_dir, "other.txt", "other change\n")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    ctx.settings.feature_flags.worktree_manipulation = true;
    ctx.worktrees_with_state()?;
    but_api::worktrees::worktree_set_archived(&mut ctx, "wt".into(), false)?;

    let old_tip = ctx.repo.get()?.rev_parse_single("feature")?.detach();
    let mut specs: Vec<DiffSpec> =
        but_api::worktrees::linked_worktree_changes(&mut ctx, "wt".into())?
            .changes
            .into_iter()
            .filter(|change| change.path == "file.txt")
            .map(|change| DiffSpec::from(but_core::TreeChange::from(change)))
            .collect();
    specs.push(DiffSpec {
        path: "other.txt".into(),
        hunk_headers: vec![HunkHeader {
            old_start: 99,
            old_lines: 1,
            new_start: 99,
            new_lines: 1,
        }],
        ..Default::default()
    });

    let err = match but_api::worktrees::worktree_commit_amend_all(
        &mut ctx,
        "wt".into(),
        old_tip,
        specs,
        None,
        DryRun::No,
    ) {
        Ok(_) => panic!("one rejected spec must abort the entire amend"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("Couldn't amend all linked-worktree changes"),
        "{err}"
    );

    let repo = ctx.repo.get()?;
    assert_eq!(repo.rev_parse_single("feature")?.detach(), old_tip);
    assert!(
        but_testsupport::git_status_at_dir(&worktree_dir)?.contains("file.txt"),
        "the valid change must remain uncommitted when another spec is rejected"
    );
    Ok(())
}

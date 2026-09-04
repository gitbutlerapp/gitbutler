use but_core::DryRun;
use but_testsupport::gix_testtools::tempfile::TempDir;
use but_testsupport::{git_status, visualize_commit_graph_all};
use gix::bstr::ByteSlice;

use crate::support::{persist_default_target, writable_scenario};

/// `feature` carries two commits above the target, and `base.txt` is dirty in the worktree.
///
/// The dirty hunk deliberately has no persisted assignment row: that is the state in which
/// uncommit has to tell "hunk that was already here" apart from "hunk the uncommit surfaced".
fn context_with_two_commits_and_a_dirty_file() -> anyhow::Result<(but_ctx::Context, TempDir)> {
    let (repo, tmp) = writable_scenario("two-commits-above-target-with-dirty-base");
    persist_default_target(&repo)?;
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    Ok((ctx, tmp))
}

/// The subject of the uncommit: the single `feature` stack and its tip commit.
fn feature_stack_and_tip(
    ctx: &but_ctx::Context,
) -> anyhow::Result<(but_core::ref_metadata::StackId, gix::ObjectId)> {
    let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
    let stack = ws.stacks.first().expect("`feature` is the only stack");
    let stack_id = stack.id.expect("a workspace stack carries an id");
    let tip = repo.rev_parse_single("refs/heads/feature")?.detach();
    Ok((stack_id, tip))
}

/// One `path: branch` line per persisted hunk assignment, sorted by path, with `-` for unassigned.
fn hunk_assignments(ctx: &but_ctx::Context) -> anyhow::Result<String> {
    let db = ctx.db.get_cache()?;
    let mut rows = db.hunk_assignments().list_all()?;
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(rows
        .into_iter()
        .map(|row| {
            let branch = row
                .branch_ref_bytes
                .map_or("-".into(), |branch| branch.to_str_lossy().into_owned());
            format!("{}: {branch}\n", row.path)
        })
        .collect())
}

/// `assign_to` must claim only the hunks the uncommit surfaced.
///
/// The two hunk-assignment passes either side of the rebase compare freshly minted assignment
/// ids, so the first pass has to be visible to the second. When it is not, every worktree hunk
/// without a persisted row looks new and gets swept into the target stack.
#[test]
fn uncommit_assigns_only_the_surfaced_hunks_to_the_target_stack() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_two_commits_and_a_dirty_file()?;
    let (stack_id, subject_commit_id) = feature_stack_and_tip(&ctx)?;
    {
        let repo = ctx.repo.get()?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* d87b45f (HEAD -> feature) second
* f9a2b9d first
* 1d69e3f (origin/main, main) base

"#]]
        );
        snapbox::assert_data_eq!(
            git_status(&repo)?,
            snapbox::str![[r#"
 M base.txt

"#]]
        );
    }

    but_api::commit::uncommit::commit_uncommit(
        &mut ctx,
        vec![subject_commit_id],
        Some(stack_id),
        DryRun::No,
    )?;

    let repo = ctx.repo.get()?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f9a2b9d (HEAD -> feature) first
* 1d69e3f (origin/main, main, gitbutler/target) base

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 M base.txt
A  second.txt

"#]]
    );
    // Only `second.txt`, surfaced by the uncommit, is assigned; the pre-existing `base.txt` edit
    // stays where the user left it.
    snapbox::assert_data_eq!(
        hunk_assignments(&ctx)?,
        snapbox::str![[r#"
base.txt: -
second.txt: refs/heads/feature

"#]]
    );
    Ok(())
}

/// A dry run must leave the assignment table exactly as it found it.
#[test]
fn uncommit_dry_run_persists_no_assignments() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_two_commits_and_a_dirty_file()?;
    let (stack_id, subject_commit_id) = feature_stack_and_tip(&ctx)?;

    but_api::commit::uncommit::commit_uncommit(
        &mut ctx,
        vec![subject_commit_id],
        Some(stack_id),
        DryRun::Yes,
    )?;

    let repo = ctx.repo.get()?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* d87b45f (HEAD -> feature) second
* f9a2b9d first
* 1d69e3f (origin/main, main) base

"#]]
    );
    snapbox::assert_data_eq!(
        git_status(&repo)?,
        snapbox::str![[r#"
 M base.txt

"#]]
    );
    snapbox::assert_data_eq!(hunk_assignments(&ctx)?, snapbox::str![""]);
    Ok(())
}

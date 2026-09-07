use anyhow::Result;
use but_api::worktrees::{
    ListedWorktree, WorktreeListing, worktree_remove, worktree_set_archived, worktrees_list,
};
use but_testsupport::gix_testtools::tempfile::TempDir;
use but_testsupport::{CommandExt, git_at_dir};
use gix::bstr::ByteSlice;
use snapbox::ToDebug;

use crate::support::{repo_with_feature_branch, writable_scenario_slow};

/// A flag-on context around the `worktree-listing` scenario.
///
/// Its worktrees exist before the first flag-on read, so adoption archives all of them.
fn flag_on_ctx_with_listing_worktrees() -> Result<(but_ctx::Context, TempDir)> {
    let (repo, tmp) = writable_scenario_slow("worktree-listing");
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    ctx.settings.feature_flags.worktree_manipulation = true;
    Ok((ctx, tmp))
}

fn render(listing: &WorktreeListing) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for (title, worktrees) in [("active", &listing.active), ("archived", &listing.archived)] {
        writeln!(out, "{title}:").expect("enough memory");
        for ListedWorktree {
            name,
            ref_name,
            updated_at_ms,
            ..
        } in worktrees
        {
            let updated_at = updated_at_ms.map_or("no reflog".to_string(), |ms| {
                chrono::DateTime::from_timestamp_millis(ms)
                    .expect("reflog timestamps are in range")
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string()
            });
            let ref_name = ref_name
                .as_ref()
                .map_or("detached".to_string(), |ref_name| ref_name.to_string());
            writeln!(
                out,
                "  {name:<10}  {updated_at:<20}  {ref_name}",
                name = name.to_str_lossy()
            )
            .expect("enough memory");
        }
    }
    out
}

#[test]
fn listing_is_sorted_by_reflog_recency_then_name() -> Result<()> {
    let (mut ctx, _tmp) = flag_on_ctx_with_listing_worktrees()?;

    // The first read adopts, archiving every worktree the scenario made. Either section sorts
    // the same way: newest reflog entry first, ties by name, and no reflog at all last.
    snapbox::assert_data_eq!(
        render(&worktrees_list(&mut ctx)?),
        snapbox::str![[r#"
active:
archived:
  newer       2000-01-04T00:00:00Z  refs/heads/newer
  older       2000-01-03T00:00:00Z  refs/heads/older
  same-day-a  2000-01-02T00:00:00Z  refs/heads/same-day-a
  same-day-b  2000-01-02T00:00:00Z  refs/heads/same-day-b
  nolog       no reflog             refs/heads/nolog

"#]]
    );

    for name in ["newer", "older", "same-day-a", "same-day-b", "nolog"] {
        worktree_set_archived(&mut ctx, name.into(), false)?;
    }
    snapbox::assert_data_eq!(
        render(&worktrees_list(&mut ctx)?),
        snapbox::str![[r#"
active:
  newer       2000-01-04T00:00:00Z  refs/heads/newer
  older       2000-01-03T00:00:00Z  refs/heads/older
  same-day-a  2000-01-02T00:00:00Z  refs/heads/same-day-a
  same-day-b  2000-01-02T00:00:00Z  refs/heads/same-day-b
  nolog       no reflog             refs/heads/nolog
archived:

"#]]
    );

    worktree_set_archived(&mut ctx, "older".into(), true)?;
    snapbox::assert_data_eq!(
        render(&worktrees_list(&mut ctx)?),
        snapbox::str![[r#"
active:
  newer       2000-01-04T00:00:00Z  refs/heads/newer
  same-day-a  2000-01-02T00:00:00Z  refs/heads/same-day-a
  same-day-b  2000-01-02T00:00:00Z  refs/heads/same-day-b
  nolog       no reflog             refs/heads/nolog
archived:
  older       2000-01-03T00:00:00Z  refs/heads/older

"#]]
    );
    Ok(())
}

#[test]
fn remove_needs_force_for_a_dirty_checkout_and_forgets_the_archived_state() -> Result<()> {
    let (mut ctx, tmp) = flag_on_ctx_with_listing_worktrees()?;
    let path = tmp.path().join("worktrees").join("older");

    let err = worktree_remove(&mut ctx, "older".into(), false).unwrap_err();
    snapbox::assert_data_eq!(
        err.to_string(),
        snapbox::str![
            "fatal: '[..]/worktrees/older' contains modified or untracked files, use --force to delete it"
        ]
    );
    // The refused removal leaves the checkout, and its adopted archived row, alone.
    snapbox::assert_data_eq!(
        render(&worktrees_list(&mut ctx)?),
        snapbox::str![[r#"
active:
archived:
  newer       2000-01-04T00:00:00Z  refs/heads/newer
  older       2000-01-03T00:00:00Z  refs/heads/older
  same-day-a  2000-01-02T00:00:00Z  refs/heads/same-day-a
  same-day-b  2000-01-02T00:00:00Z  refs/heads/same-day-b
  nolog       no reflog             refs/heads/nolog

"#]]
    );

    worktree_remove(&mut ctx, "older".into(), true)?;
    assert!(!path.exists(), "the checkout is deleted from disk");
    snapbox::assert_data_eq!(
        render(&worktrees_list(&mut ctx)?),
        snapbox::str![[r#"
active:
archived:
  newer       2000-01-04T00:00:00Z  refs/heads/newer
  same-day-a  2000-01-02T00:00:00Z  refs/heads/same-day-a
  same-day-b  2000-01-02T00:00:00Z  refs/heads/same-day-b
  nolog       no reflog             refs/heads/nolog

"#]]
    );

    // Re-creating the name yields an active worktree: the archived row went with the checkout,
    // while the branch stayed, as it does with `git worktree remove` - its day-3 reflog entry
    // still dates the listing.
    git_at_dir(tmp.path())
        .args(["worktree", "add"])
        .arg(&path)
        .arg("older")
        .run();
    snapbox::assert_data_eq!(
        render(&worktrees_list(&mut ctx)?),
        snapbox::str![[r#"
active:
  older       2000-01-03T00:00:00Z  refs/heads/older
archived:
  newer       2000-01-04T00:00:00Z  refs/heads/newer
  same-day-a  2000-01-02T00:00:00Z  refs/heads/same-day-a
  same-day-b  2000-01-02T00:00:00Z  refs/heads/same-day-b
  nolog       no reflog             refs/heads/nolog

"#]]
    );

    let err = worktree_remove(&mut ctx, "missing".into(), true).unwrap_err();
    snapbox::assert_data_eq!(
        err.to_string(),
        snapbox::str!["Worktree missing does not exist"]
    );
    Ok(())
}

#[test]
fn everything_is_gated_on_the_feature_flag() -> Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let errors = [
        worktrees_list(&mut ctx).map(drop).unwrap_err(),
        worktree_set_archived(&mut ctx, "wt".into(), true).unwrap_err(),
        worktree_remove(&mut ctx, "wt".into(), true).unwrap_err(),
    ]
    .map(|err| err.to_string());
    snapbox::assert_data_eq!(
        errors.to_debug(),
        snapbox::str![[r#"
[
    "worktree manipulation is not enabled (featureFlags.worktreeManipulation)",
    "worktree manipulation is not enabled (featureFlags.worktreeManipulation)",
    "worktree manipulation is not enabled (featureFlags.worktreeManipulation)",
]

"#]]
    );
    Ok(())
}

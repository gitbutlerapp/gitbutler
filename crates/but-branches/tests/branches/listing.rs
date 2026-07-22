//! Tests for [`but_branches::list()`].

use std::fmt::Write as _;

use but_branches::BranchListing;
use but_testsupport::visualize_commit_graph_all;

use crate::utils::{
    StackState, add_stack_with_segments, named_read_only_in_memory_scenario,
    project_meta_with_target,
};

/// A compact, deterministic rendering of the listing: one line per stack,
/// one indented line per branch.
fn listing_snapshot(listing: &BranchListing) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "target: {}, incomplete: {}",
        listing
            .target_ref
            .as_ref()
            .map(|name| name.as_bstr().to_string())
            .unwrap_or_else(|| "none".into()),
        listing.incomplete
    );
    for stack in &listing.stacks {
        let tip = stack
            .branches
            .first()
            .map(|branch| branch.display_name.to_string())
            .unwrap_or_default();
        let _ = writeln!(out, "{:?} [{tip}]", stack.status);
        for branch in &stack.branches {
            let locality = match (branch.has_local, !branch.remote_refs.is_empty()) {
                (true, true) => "local+remote",
                (true, false) => "local",
                (false, true) => "remote",
                (false, false) => "none",
            };
            let remotes = branch
                .remote_refs
                .iter()
                .map(|name| name.as_bstr().to_string())
                .collect::<Vec<_>>()
                .join(",");
            let divergence = branch
                .commits_ahead_of_target
                .map(|ahead| format!("+{ahead}"))
                .unwrap_or_else(|| "n/a".into());
            let commit_count = branch
                .commit_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "?".into());
            let _ = writeln!(
                out,
                "  {} ({locality}{}{remotes}) {} commits={commit_count} target({divergence})",
                branch.display_name,
                if remotes.is_empty() { "" } else { ": " },
                &branch.tip.to_string()[..7],
            );
        }
    }
    out
}

/// Listing options for a repository whose target is `refs/remotes/origin/main`.
fn options_with_target(repo: &gix::Repository) -> anyhow::Result<but_branches::Options> {
    Ok(but_branches::Options {
        project_meta: project_meta_with_target(repo)?,
        hard_limit: None,
    })
}

/// Listing options for a repository without a configured target, as in one
/// GitButler never initialized.
fn options_without_target() -> but_branches::Options {
    but_branches::Options::default()
}

#[test]
fn grouped_and_classified() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario("branch-listing", "")?;
    // The fixture provides every classification the listing knows: an applied and
    // an unapplied stack, a standalone chain, a fork, aliases, a lagging remote,
    // and a remote-only branch.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c20d83a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 2febd1f (applied-top) applied-top-1
* 9528084 (applied-bottom) applied-bottom-1
| * bb8e41e (remote-behind) remote-behind-2
| * a420aec (origin/remote-behind) remote-behind-1
|/  
| * 10f85fd (origin/remote-only) remote-only-1
|/  
| * fbd11c1 (fork-a) fork-a-1
| | * 3b63a74 (fork-b) fork-b-1
| |/  
| * 841b659 fork-shared
|/  
| * ccdbbc8 (chain-top) chain-top-1
| * cfaafa9 (chain-bottom) chain-bottom-1
|/  
| * 2215f76 (origin/standalone, standalone-alias, standalone) standalone-2
| * 6d7ddfc standalone-1
|/  
| * 645e91c (unapplied-top) unapplied-top-1
| * 06d2867 (unapplied-bottom) unapplied-bottom-1
|/  
* 353b35c (origin/main, main) init

"#]]
    );
    add_stack_with_segments(
        &mut meta,
        1,
        "applied-top",
        StackState::InWorkspace,
        &["applied-bottom"],
    );
    add_stack_with_segments(
        &mut meta,
        2,
        "unapplied-top",
        StackState::Inactive,
        &["unapplied-bottom"],
    );

    let listing = but_branches::list(&repo, &*meta, options_with_target(&repo)?)?;
    // Stacks come back purely by recency regardless of status, so the workspace ones
    // land last here on the oldest commits; counts are each branch's exclusive
    // commits, ahead-counts are relative to the target.
    snapbox::assert_data_eq!(
        listing_snapshot(&listing),
        snapbox::str![[r#"
target: refs/remotes/origin/main, incomplete: false
Standalone [remote-behind]
  remote-behind (local+remote: refs/remotes/origin/remote-behind) bb8e41e commits=2 target(+2)
Standalone [remote-only]
  remote-only (remote: refs/remotes/origin/remote-only) 10f85fd commits=1 target(+1)
Standalone [fork-a]
  fork-a (local) fbd11c1 commits=1 target(+2)
Standalone [fork-b]
  fork-b (local) 3b63a74 commits=1 target(+2)
Standalone [chain-top]
  chain-top (local) ccdbbc8 commits=1 target(+2)
  chain-bottom (local) cfaafa9 commits=1 target(+1)
Standalone [standalone]
  standalone (local+remote: refs/remotes/origin/standalone) 2215f76 commits=2 target(+2)
Standalone [standalone-alias]
  standalone-alias (local) 2215f76 commits=2 target(+2)
Unapplied [unapplied-top]
  unapplied-top (local) 645e91c commits=1 target(+2)
  unapplied-bottom (local) 06d2867 commits=1 target(+1)
Applied [applied-top]
  applied-top (local) 2febd1f commits=1 target(+2)
  applied-bottom (local) 9528084 commits=1 target(+1)
Target [main]
  main (local+remote: refs/remotes/origin/main) 353b35c commits=0 target(+0)

"#]]
    );
    Ok(())
}

#[test]
fn ordinary_repository_without_workspace() -> anyhow::Result<()> {
    let (repo, meta) =
        named_read_only_in_memory_scenario("no-ws-ref-no-ws-commit-two-branches", "")?;
    // Every ref, including the target, points at one single commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e5d0542 (HEAD -> main, origin/main, B, A) A

"#]]
    );

    let listing = but_branches::list(&repo, &*meta, options_with_target(&repo)?)?;
    // Stacks come back purely by recency regardless of status, so the workspace ones
    // land last here on the oldest commits; counts are each branch's exclusive
    // commits, ahead-counts are relative to the target.
    snapbox::assert_data_eq!(
        listing_snapshot(&listing),
        snapbox::str![[r#"
target: refs/remotes/origin/main, incomplete: false
Standalone [A]
  A (local) e5d0542 commits=0 target(+0)
Standalone [B]
  B (local) e5d0542 commits=0 target(+0)
Applied [main]
  main (local+remote: refs/remotes/origin/main) e5d0542 commits=0 target(+0)

"#]]
    );
    Ok(())
}

#[test]
fn hard_limit_marks_incomplete_and_hides_clipped_counts() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario("branch-listing", "")?;
    add_stack_with_segments(
        &mut meta,
        1,
        "applied-top",
        StackState::InWorkspace,
        &["applied-bottom"],
    );
    add_stack_with_segments(
        &mut meta,
        2,
        "unapplied-top",
        StackState::Inactive,
        &["unapplied-bottom"],
    );

    let unlimited = but_branches::list(&repo, &*meta, options_with_target(&repo)?)?;
    let mut options = options_with_target(&repo)?;
    options.hard_limit = Some(6);
    let limited = but_branches::list(&repo, &*meta, options)?;

    assert!(
        !unlimited.incomplete,
        "an unbounded traversal is never incomplete"
    );
    assert!(
        limited.incomplete,
        "a hit hard limit must be reported as an incomplete listing"
    );

    // The exact clipping depends on traversal order, which may change with the
    // graph implementation. What must hold regardless: every branch is still
    // listed, and every count the limited listing does report agrees with the
    // unlimited truth - clipped branches say `None` rather than a wrong number.
    let truth: std::collections::BTreeMap<_, _> = unlimited
        .stacks
        .iter()
        .flat_map(|stack| stack.branches.iter())
        .map(|branch| {
            (
                branch.display_name.clone(),
                (branch.commit_count, branch.commits_ahead_of_target),
            )
        })
        .collect();
    let limited_branches: Vec<_> = limited
        .stacks
        .iter()
        .flat_map(|stack| stack.branches.iter())
        .collect();
    assert_eq!(
        limited_branches.len(),
        truth.len(),
        "a traversal limit must not drop branches from the listing"
    );
    for branch in &limited_branches {
        let (true_count, true_ahead) = &truth[&branch.display_name];
        if branch.commit_count.is_some() {
            assert_eq!(
                &branch.commit_count, true_count,
                "a reported commit count must match the unlimited truth for {}",
                branch.display_name
            );
        }
        if branch.commits_ahead_of_target.is_some() {
            assert_eq!(
                &branch.commits_ahead_of_target, true_ahead,
                "a reported ahead count must match the unlimited truth for {}",
                branch.display_name
            );
        }
    }
    assert!(
        limited_branches
            .iter()
            .any(|branch| branch.commit_count.is_none()),
        "a limit this small must actually clip some counts"
    );
    Ok(())
}

#[test]
fn no_target_configured_degrades_gracefully() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario("branch-listing", "")?;
    add_stack_with_segments(
        &mut meta,
        1,
        "applied-top",
        StackState::InWorkspace,
        &["applied-bottom"],
    );
    add_stack_with_segments(
        &mut meta,
        2,
        "unapplied-top",
        StackState::Inactive,
        &["unapplied-bottom"],
    );

    let listing = but_branches::list(&repo, &*meta, options_without_target())?;
    // Everything still lists and stacks; only target-relative data is absent:
    // no target stack, every ahead-count is n/a, and commit counts fall back to
    // fork-point boundaries instead of integrated history.
    snapbox::assert_data_eq!(
        listing_snapshot(&listing),
        snapbox::str![[r#"
target: none, incomplete: false
Standalone [remote-behind]
  remote-behind (local+remote: refs/remotes/origin/remote-behind) bb8e41e commits=2 target(n/a)
Standalone [remote-only]
  remote-only (remote: refs/remotes/origin/remote-only) 10f85fd commits=1 target(n/a)
Standalone [fork-a]
  fork-a (local) fbd11c1 commits=1 target(n/a)
Standalone [fork-b]
  fork-b (local) 3b63a74 commits=1 target(n/a)
Standalone [chain-top]
  chain-top (local) ccdbbc8 commits=1 target(n/a)
  chain-bottom (local) cfaafa9 commits=1 target(n/a)
Standalone [standalone]
  standalone (local+remote: refs/remotes/origin/standalone) 2215f76 commits=2 target(n/a)
Standalone [standalone-alias]
  standalone-alias (local) 2215f76 commits=2 target(n/a)
Unapplied [unapplied-top]
  unapplied-top (local) 645e91c commits=1 target(n/a)
  unapplied-bottom (local) 06d2867 commits=1 target(n/a)
Applied [applied-top]
  applied-top (local) 2febd1f commits=1 target(n/a)
  applied-bottom (local) 9528084 commits=1 target(n/a)
  main (local+remote: refs/remotes/origin/main) 353b35c commits=1 target(n/a)

"#]]
    );
    Ok(())
}

#[test]
fn unborn_head_still_lists_remote_branches() -> anyhow::Result<()> {
    let (repo, meta) = named_read_only_in_memory_scenario("unborn-head-with-remotes", "")?;
    let listing = but_branches::list(&repo, &*meta, options_without_target())?;
    // An unborn HEAD must not hide the branches that already exist; the traversal
    // starts from an enumerated tip instead, and only target-relative data is absent.
    snapbox::assert_data_eq!(
        listing_snapshot(&listing),
        snapbox::str![[r#"
target: none, incomplete: false
Standalone [feature]
  feature (remote: refs/remotes/origin/feature) a8bbeb6 commits=2 target(n/a)

"#]]
    );
    Ok(())
}

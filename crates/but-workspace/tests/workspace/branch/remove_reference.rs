use but_core::RefMetadata;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use but_workspace::branch::remove_reference;
use gix::refs::{Category, transaction::PreviousValue};

use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments,
        named_writable_scenario_with_args_and_description_and_graph,
        named_writable_scenario_with_description_and_graph,
    },
    utils::r,
};

#[test]
fn no_errors_due_to_idempotency_in_empty_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) =
        named_writable_scenario_with_args_and_description_and_graph(
            "single-branch-no-ws-commit-no-target",
            ["A", "B"],
            |_| {},
        )?;
    insta::assert_snapshot!(desc, @"Single commit, no main remote/target, no ws commit, but ws-reference");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, B, A) M1");
    let ws = graph.into_workspace()?;
    // the workspace is empty.
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");

    for name in ["A", "B", "gitbutler/workspace", "main", "nonexisting"] {
        assert!(
            but_workspace::branch::remove_reference(
                Category::LocalBranch.to_full_name(name)?.as_ref(),
                &repo,
                &ws,
                &mut meta,
                remove_reference::Options {
                    keep_metadata: true,
                    ..Default::default()
                },
            )?
            .is_none()
        );

        assert!(
            but_workspace::branch::remove_reference(
                Category::LocalBranch.to_full_name(name)?.as_ref(),
                &repo,
                &ws,
                &mut meta,
                remove_reference::Options {
                    keep_metadata: false,
                    ..Default::default()
                },
            )?
            .is_none()
        );
    }

    // repo and workspace should still look like before.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, B, A) M1");
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");

    Ok(())
}

#[test]
fn journey_single_branch_no_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) = named_writable_scenario_with_description_and_graph(
        "single-branch-3-commits-no-ws-commit-more-branches",
        |meta| {
            add_stack_with_segments(meta, 0, "A", StackState::InWorkspace, &[]);
        },
    )?;
    insta::assert_snapshot!(desc, @"Single commit, target, no ws commit, but ws-reference and a named segment, and branches on each commit");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c2878fb (HEAD -> gitbutler/workspace, A2, A) A2
    * 49d4b34 (A1) A1
    * 3183e43 (origin/main, main) M1
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡📙:3:A on 3183e43 {0}
        ├── 📙:3:A
        │   └── ·c2878fb (🏘️) ►A2
        └── :4:A1
            └── ·49d4b34 (🏘️)
    ");

    // It's OK to delete all segment names of a stack
    for name in ["A", "A2", "A1"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                // This is what allows us to delete everything.
                avoid_anonymous_stacks: false,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }

    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡:3:anon: on 3183e43
        └── :3:anon:
            ├── ·c2878fb (🏘️)
            └── ·49d4b34 (🏘️)
    ");

    Ok(())
}

#[test]
fn journey_single_branch_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) = named_writable_scenario_with_description_and_graph(
        "single-branch-4-commits-more-branches",
        |meta| {
            add_stack_with_segments(
                meta,
                0,
                "A",
                StackState::InWorkspace,
                &["A2-3", "A2-2", "A2-1", "A1-1", "A1-2", "A1-3"],
            );
        },
    )?;

    insta::assert_snapshot!(desc, @"Two commits in main, target setup, ws commit, many more usable branches");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 43f9472 (A2-3, A2-2, A2-1, A) A2
    * 6fdab32 (A1-3, A1-2, A1-1) A1
    * bce0c5e (origin/main, main) M2
    * 3183e43 M1
    ");
    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡📙:5:A on bce0c5e {0}
        ├── 📙:5:A
        ├── 📙:6:A2-3
        ├── 📙:7:A2-2
        ├── 📙:8:A2-1
        │   └── ·43f9472 (🏘️)
        ├── 📙:9:A1-1
        ├── 📙:10:A1-2
        └── 📙:11:A1-3
            └── ·6fdab32 (🏘️)
    ");

    // Delete a whole segment to see how it pulls up to the top of the stack a branch from below
    for name in ["A2-1", "A2-3", "A", "A2-2"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                // This causes "A1-1" to become the top of the stack.
                avoid_anonymous_stacks: true,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡📙:3:A1-1 on bce0c5e {0}
        ├── 📙:3:A1-1
        │   └── ·43f9472 (🏘️)
        ├── 📙:5:A1-2
        └── 📙:6:A1-3
            └── ·6fdab32 (🏘️)
    ");

    for name in ["A1-1", "A1-2"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                avoid_anonymous_stacks: true,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }
    // Just one segment left.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡📙:3:A1-3 on bce0c5e {0}
        └── 📙:3:A1-3
            ├── ·43f9472 (🏘️)
            └── ·6fdab32 (🏘️)
    ");

    let err = but_workspace::branch::remove_reference(
        r("refs/heads/A1-3"),
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options {
            avoid_anonymous_stacks: true,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to delete last named segment 'A1-3' as it would leave an anonymous segment",
        "won't allow to create anon segment by deleting the last one."
    );

    Ok(())
}

#[test]
fn journey_no_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) =
        named_writable_scenario_with_args_and_description_and_graph(
            "single-branch-no-ws-commit-no-target",
            ["A", "B", "C", "D", "E"],
            |meta| {
                add_stack_with_segments(meta, 0, "A", StackState::InWorkspace, &["B", "C"]);
                add_stack_with_segments(meta, 1, "D", StackState::InWorkspace, &["E"]);
            },
        )?;
    insta::assert_snapshot!(desc, @"Single commit, no main remote/target, no ws commit, but ws-reference");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, E, D, C, B, A) M1");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:5:D on 3183e43 {1}
    │   ├── 📙:5:D
    │   └── 📙:6:E
    └── ≡📙:2:A on 3183e43 {0}
        ├── 📙:2:A
        ├── 📙:3:B
        └── 📙:4:C
    ");

    let ref_name = r("refs/heads/A");
    let ws = but_workspace::branch::remove_reference(
        ref_name,
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options {
            keep_metadata: true,
            ..Default::default()
        },
    )?
    .expect("we deleted something");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:4:D on 3183e43 {1}
    │   ├── 📙:4:D
    │   └── 📙:5:E
    └── ≡📙:2:B on 3183e43 {0}
        ├── 📙:2:B
        └── 📙:3:C
    ");

    let main_id = repo.head_id()?;
    repo.reference(
        ref_name,
        main_id,
        PreviousValue::Any,
        "recreate ref to show metadata is present and unchanged",
    )?;

    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:5:D on 3183e43 {1}
    │   ├── 📙:5:D
    │   └── 📙:6:E
    └── ≡📙:2:A on 3183e43 {0}
        ├── 📙:2:A
        ├── 📙:3:B
        └── 📙:4:C
    ");

    let mut ws = but_workspace::branch::remove_reference(
        ref_name,
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options::default(),
    )?
    .expect("we deleted something");
    repo.reference(
        ref_name,
        main_id,
        PreviousValue::Any,
        "recreate ref - this time it's not visible as it lacks metadata",
    )?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:4:D on 3183e43 {1}
    │   ├── 📙:4:D
    │   └── 📙:5:E
    └── ≡📙:2:B on 3183e43 {0}
        ├── 📙:2:B
        └── 📙:3:C
    ");

    // Try to delete it again, just to see that it doesn't try to touch it as it's outside the workspace.
    assert!(
        but_workspace::branch::remove_reference(
            ref_name,
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options::default(),
        )?
        .is_none()
    );
    assert!(
        repo.find_reference(ref_name).is_ok(),
        "The reference still exist as we only remove what's in the workspace, nothing arbitrary"
    );

    // We can delete everything.
    for name in ["D", "B", "E", "C"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                // This has no effect
                avoid_anonymous_stacks: true,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }

    // A remains as we recreated it.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, A) M1");
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    // The workspace is completely empty.
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");

    assert_eq!(
        meta.iter().count(),
        0,
        "nothing is left in the metadata either"
    );

    Ok(())
}

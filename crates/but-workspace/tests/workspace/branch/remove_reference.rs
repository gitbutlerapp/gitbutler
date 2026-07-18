use but_core::RefMetadata;
use but_testsupport::{graph_workspace, id_by_rev, visualize_commit_graph_all};
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
    snapbox::assert_data_eq!(
        desc,
        snapbox::str![[r#"
Single commit, no main remote/target, no ws commit, but ws-reference

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, main, B, A) M1

"#]]
    );
    let ws = graph.into_workspace()?;
    // the workspace is empty.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
    );

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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, main, B, A) M1

"#]]
    );
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:3:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
    );

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
    snapbox::assert_data_eq!(
        desc,
        snapbox::str![[r#"
Single commit, target, no ws commit, but ws-reference and a named segment, and branches on each commit

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c2878fb (HEAD -> gitbutler/workspace, A2, A) A2
* 49d4b34 (A1) A1
* 3183e43 (origin/main, main) M1

"#]]
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:7:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:5:A on 3183e43 {0}
    ├── 📙:5:A
    │   └── ·c2878fb (🏘️) ►A2, ►gitbutler/workspace[🌳]
    └── :8:A1
        └── ·49d4b34 (🏘️)

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡:1:anon on 3183e43
    └── :1:anon
        ├── ·c2878fb (🏘️) ►gitbutler/workspace[🌳]
        └── ·49d4b34 (🏘️)

"#]]
    );

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

    snapbox::assert_data_eq!(
        desc,
        snapbox::str![[r#"
Two commits in main, target setup, ws commit, many more usable branches

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 43f9472 (A2-3, A2-2, A2-1, A) A2
* 6fdab32 (A1-3, A1-2, A1-1) A1
* bce0c5e (origin/main, main) M2
* 3183e43 M1

"#]]
    );
    let retained_commits = [
        id_by_rev(&repo, ":/A2").detach(),
        id_by_rev(&repo, ":/A1").detach(),
    ];
    let mut ws = graph.into_workspace()?;

    // Delete the reference nodes while preserving the commits they used to delimit.
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
    // `avoid_anonymous_stacks` no longer enforces the legacy "last named segment" rule.
    // Removing the final branch reference is valid because the commit nodes remain in the workspace.
    ws = but_workspace::branch::remove_reference(
        r("refs/heads/A1-3"),
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options {
            avoid_anonymous_stacks: true,
            ..Default::default()
        },
    )?
    .expect("the final named reference was deleted");
    assert_eq!(
        ws.stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.commits)
            .map(|commit| commit.id)
            .collect::<Vec<_>>(),
        retained_commits,
        "deleting the reference nodes keeps both commits projected in the workspace"
    );
    assert!(repo.try_find_reference("refs/heads/A1-3")?.is_none());

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
    snapbox::assert_data_eq!(
        desc,
        snapbox::str![[r#"
Single commit, no main remote/target, no ws commit, but ws-reference

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, main, E, D, C, B, A) M1

"#]]
    );

    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:6:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:1:A on 3183e43 {0}
│   ├── 📙:1:A
│   ├── 📙:2:B
│   └── 📙:3:C
└── ≡📙:4:D on 3183e43 {1}
    ├── 📙:4:D
    └── 📙:5:E

"#]]
    );

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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:1:B on 3183e43 {0}
│   ├── 📙:1:B
│   └── 📙:2:C
└── ≡📙:3:D on 3183e43 {1}
    ├── 📙:3:D
    └── 📙:4:E

"#]]
    );

    let main_id = repo.head_id()?;
    repo.reference(
        ref_name,
        main_id,
        PreviousValue::Any,
        "recreate ref to show metadata is present and unchanged",
    )?;

    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:6:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:1:A on 3183e43 {0}
│   ├── 📙:1:A
│   ├── 📙:2:B
│   └── 📙:3:C
└── ≡📙:4:D on 3183e43 {1}
    ├── 📙:4:D
    └── 📙:5:E

"#]]
    );

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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:5:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:1:B on 3183e43 {0}
│   ├── 📙:1:B
│   └── 📙:2:C
└── ≡📙:3:D on 3183e43 {1}
    ├── 📙:3:D
    └── 📙:4:E

"#]]
    );

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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, main, A) M1

"#]]
    );
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    // The workspace is completely empty.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:2:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
    );

    assert_eq!(
        meta.iter().count(),
        0,
        "nothing is left in the metadata either"
    );

    Ok(())
}

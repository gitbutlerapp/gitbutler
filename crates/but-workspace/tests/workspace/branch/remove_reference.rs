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
    let (_tmp, ws, repo, mut meta, desc) =
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
    // the workspace is empty.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43

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
    let ws = ws.redo(&repo, &meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
    );

    Ok(())
}

#[test]
fn journey_single_branch_no_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, desc) = named_writable_scenario_with_description_and_graph(
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    ├── 📙:A
    │   └── ·c2878fb (🏘️) ►A2
    └── :A1
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

    let ws = ws.redo(&repo, &meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡:anon: on 3183e43
    └── :anon:
        ├── ·c2878fb (🏘️)
        └── ·49d4b34 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn journey_single_branch_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, mut ws, repo, mut meta, desc) = named_writable_scenario_with_description_and_graph(
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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:A on bce0c5e {0}
    ├── 📙:A
    ├── 📙:A2-3
    ├── 📙:A2-2
    ├── 📙:A2-1
    │   └── ·43f9472 (🏘️)
    ├── 📙:A1-1
    ├── 📙:A1-2
    └── 📙:A1-3
        └── ·6fdab32 (🏘️)

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:A1-1 on bce0c5e {0}
    ├── 📙:A1-1
    │   └── ·43f9472 (🏘️)
    ├── 📙:A1-2
    └── 📙:A1-3
        └── ·6fdab32 (🏘️)

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:A1-3 on bce0c5e {0}
    └── 📙:A1-3
        ├── ·43f9472 (🏘️)
        └── ·6fdab32 (🏘️)

"#]]
    );

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
    let (_tmp, ws, repo, mut meta, desc) =
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

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:A on 3183e43 {0}
│   ├── 📙:A
│   ├── 📙:B
│   └── 📙:C
└── ≡📙:D on 3183e43 {1}
    ├── 📙:D
    └── 📙:E

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
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:B on 3183e43 {0}
│   ├── 📙:B
│   └── 📙:C
└── ≡📙:D on 3183e43 {1}
    ├── 📙:D
    └── 📙:E

"#]]
    );

    let main_id = repo.head_id()?;
    repo.reference(
        ref_name,
        main_id,
        PreviousValue::Any,
        "recreate ref to show metadata is present and unchanged",
    )?;

    let ws = ws.redo(&repo, &meta, Default::default())?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:A on 3183e43 {0}
│   ├── 📙:A
│   ├── 📙:B
│   └── 📙:C
└── ≡📙:D on 3183e43 {1}
    ├── 📙:D
    └── 📙:E

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
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43
├── ≡📙:B on 3183e43 {0}
│   ├── 📙:B
│   └── 📙:C
└── ≡📙:D on 3183e43 {1}
    ├── 📙:D
    └── 📙:E

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
    let ws = ws.redo(&repo, &meta, Default::default())?;
    // The workspace is completely empty.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
    );

    assert_eq!(
        meta.iter().count(),
        0,
        "nothing is left in the metadata either"
    );

    Ok(())
}

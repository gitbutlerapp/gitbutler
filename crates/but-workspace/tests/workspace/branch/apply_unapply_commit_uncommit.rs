use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_read_only_in_memory_scenario,
    named_writable_scenario_with_description_and_graph,
};
use crate::utils::r;
use but_graph::init::Options;
use but_testsupport::{graph_workspace, id_at, visualize_commit_graph_all};
use but_workspace::branch::apply::{
    IntegrationMode, OnWorkspaceConflict, WorkspaceReferenceNaming,
};
use but_workspace::branch::checkout::UncommitedWorktreeChanges;

#[test]
fn operation_denied_on_improper_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-ws-advanced",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0d01196 (HEAD -> gitbutler/workspace) O1
    * 4979833 GitButler Workspace Commit
    * 3183e43 (main, B, A) M1
    ");
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on 3183e43
    └── ≡:2:anon: on 3183e43
        └── :2:anon:
            ├── ·0d01196 (🏘️)
            └── ·4979833 (🏘️)
    ");

    let err = but_workspace::branch::apply(
        r("refs/heads/B"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to work on workspace whose workspace commit isn't at the top",
        "cannot apply on a workspace that isn't proper"
    );

    let err = but_workspace::branch::apply(r("HEAD"), &ws, &mut repo, &mut meta, default_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply symbolic ref 'HEAD' due to potential ambiguity"
    );

    // TODO: unapply, commit, uncommit
    Ok(())
}

#[test]
#[ignore = "TBD - needs fix so entrypoint change doesn't affect artificial stacks"]
fn ws_ref_no_ws_commit_two_stacks_on_same_commit() -> anyhow::Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542");

    // Put "A" into the workspace, yielding a single branch.
    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");
    insta::assert_snapshot!(graph_workspace(&out.graph.to_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542");
    // A ws commit was created as it's needed for the current commit implementation.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let out = but_workspace::branch::apply(
        r("refs/heads/B"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");
    insta::assert_snapshot!(graph_workspace(&out.graph.to_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542");

    // TODO: commit/uncommit
    // TODO: unapply

    Ok(())
}

#[test]
#[ignore = "TBD"]
fn new_workspace_exists_elsewhere_and_to_be_applied_branch_exists_there() -> anyhow::Result<()> {
    let (_tmp, ws_graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    // The default workspace, it's empty as target is set to `main`.
    insta::assert_snapshot!(graph_workspace(&ws_graph.to_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542");

    // Pretend "B" is checked out (it's at the right state independently of that)
    let (b_id, b_ref) = id_at(&repo, "B");
    let graph = but_graph::Graph::from_commit_traversal(b_id, b_ref, &meta, Default::default())?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:B <> ✓!
    └── ≡:0:B
        └── :0:B
            └── ·e5d0542 ►A, ►main
    ");

    // Put "A" into the workspace, hence we want "A" and "B" in it.
    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    // HEAD must now point to the workspace (that already existed)
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    Ok(())
}

#[test]
#[ignore = "TBD"]
fn detached_head_journey() -> anyhow::Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "detached-with-multiple-branches",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 49d4b34 (A) A1
    | * f57c528 (B) B1
    |/  
    | * aaa195b (HEAD, C) C1
    |/  
    * 3183e43 (main) M1
    ");
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:DETACHED <> ✓!
    └── ≡:0:anon:
        ├── :0:anon:
        │   └── ·aaa195b ►C
        └── :1:main
            └── ·3183e43 (✓)
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");
    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_flat() -> anyhow::Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:3:B on e5d0542
    │   └── 📙:3:B
    └── ≡📙:2:A on e5d0542
        └── 📙:2:A
    ");

    // Apply the workspace ref itself, it's a no-op
    let out = but_workspace::branch::apply(
        r("refs/heads/gitbutler/workspace"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    let (b_id, b_ref) = id_at(&repo, "B");
    let graph =
        but_graph::Graph::from_commit_traversal(b_id, b_ref.clone(), &meta, Default::default())?;
    let ws = graph.to_workspace()?;
    // TODO: fix this - the entrypoint shouldn't alter the stack setup.
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓!
    └── ≡👉📙:0:B
        ├── 👉📙:0:B
        └── 📙:2:A
            └── ·e5d0542 (🏘️) ►main
    ");

    // Already applied (the HEAD points to it, it literally IS the workspace).
    let out =
        but_workspace::branch::apply(b_ref.as_ref(), &ws, &mut repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    // To apply, we just checkout the surrounding workspace.
    // TODO: doesn't work because A isn't a separate stack like it should.
    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");
    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_with_commits() -> anyhow::Result<()> {
    let (_tmp, graph, mut repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4
    │   └── 📙:4:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    // Apply the workspace ref itself, it's a no-op
    let ws_ref = r("refs/heads/gitbutler/workspace");
    let out = but_workspace::branch::apply(ws_ref, &ws, &mut repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    let (b_id, b_ref) = id_at(&repo, "B");
    let graph =
        but_graph::Graph::from_commit_traversal(b_id, b_ref.clone(), &meta, Default::default())?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:1:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡👉📙:0:B on 85efbe4
    │   └── 👉📙:0:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:4:A on 85efbe4
        └── 📙:4:A
            └── ·09d8e52 (🏘️)
    ");

    // Already applied (the HEAD points to it, it literally IS the workspace).
    let out =
        but_workspace::branch::apply(b_ref.as_ref(), &ws, &mut repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    let err = but_workspace::branch::apply(ws_ref, &ws, &mut repo, &mut meta, default_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply a reference that already is a workspace: 'gitbutler/workspace'",
        "it's never good to merge one managed workspace into another, and we just disallow it.\
         Note that we could also check it out."
    );

    // To apply, we just checkout the surrounding workspace.
    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &mut repo,
        &mut meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");

    let ws = out.graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4
    │   └── 📙:4:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");
    Ok(())
}

#[test]
#[ignore = "TBD"]
fn apply_nonexisting_branch_failure() -> anyhow::Result<()> {
    let (mut repo, mut meta) =
        named_read_only_in_memory_scenario("ws-ref-no-ws-commit-one-stack-one-branch", "")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓!
    └── ≡:1:anon:
        └── :1:anon:
            └── ·e5d0542 (🏘️) ►A, ►B, ►main
    ");

    let err = but_workspace::branch::apply(
        r("refs/heads/does-not-exist"),
        &ws,
        &mut repo,
        &mut *meta,
        default_options(),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "TBD");

    // Nothing should be changed
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    Ok(())
}

#[test]
#[ignore = "TBD"]
fn unapply_nonexisting_branch_failure() -> anyhow::Result<()> {
    Ok(())
}

#[test]
fn unborn_apply_needs_base() -> anyhow::Result<()> {
    let (mut repo, mut meta) =
        named_read_only_in_memory_scenario("unborn-empty-detached-remote", "unborn")?;
    // Depending on the Git version it produces`* 3183e43 (orphan/main, orphan/HEAD) M1` on CI,
    // so a comment is used as reference.
    // insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (orphan/main) M1");

    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
    ");

    // Idempotency in ad-hoc workspace
    let out = but_workspace::branch::apply(
        r("refs/heads/main"),
        &ws,
        &mut repo,
        &mut *meta,
        default_options(),
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    // Cannot apply branch without a base.
    let err = but_workspace::branch::apply(
        r("refs/remotes/orphan/main"),
        &ws,
        &mut repo,
        &mut *meta,
        default_options(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot create reference on unborn branch 'main'"
    );
    Ok(())
}

fn default_options() -> but_workspace::branch::apply::Options {
    but_workspace::branch::apply::Options {
        integration_mode: IntegrationMode::MergeIfNeeded,
        on_workspace_conflict: OnWorkspaceConflict::AbortAndReportConflictingStack,
        workspace_reference_naming: WorkspaceReferenceNaming::Default,
        uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
        order: None,
    }
}

#[test]
#[ignore = "TBD"]
fn apply_branch_resting_on_base() -> anyhow::Result<()> {
    // THis can't work, but should fail gracefully.
    Ok(())
}

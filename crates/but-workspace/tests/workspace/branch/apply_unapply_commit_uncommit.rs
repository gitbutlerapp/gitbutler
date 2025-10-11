use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_read_only_in_memory_scenario,
    named_writable_scenario_with_description_and_graph,
};
use crate::utils::r;
use but_core::RefMetadata;
use but_graph::init::{Options, Overlay};
use but_testsupport::{graph_workspace, id_at, visualize_commit_graph_all};
use but_workspace::branch::OnWorkspaceMergeConflict;
use but_workspace::branch::apply::{IntegrationMode, WorkspaceReferenceNaming};
use but_workspace::branch::checkout::UncommitedWorktreeChanges;

#[test]
fn operation_denied_on_improper_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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

    let err =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, default_options())
            .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to work on workspace whose workspace commit isn't at the top",
        "cannot apply on a workspace that isn't proper"
    );

    let err = but_workspace::branch::apply(r("HEAD"), &ws, &repo, &mut meta, default_options())
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply symbolic ref 'HEAD' due to potential ambiguity"
    );

    // TODO: unapply, commit, uncommit
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_two_virtual_stacks_on_same_commit_apply_dependent_first()
-> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::Inactive, &["B"]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    // We know a stack, but nothing is actually in the workspace.
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542");

    // Put "B" into the workspace, even though it's the dependent branch of A.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    └── ≡📙:2:B on e5d0542
        └── 📙:2:B
    ");

    // Applying A is always a new stack then.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_snapshot!(graph_workspace(&out.graph.to_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:3:A on e5d0542
    │   └── 📙:3:A
    └── ≡📙:2:B on e5d0542
        └── 📙:2:B
    ");

    // It's all virtual.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    Ok(())
}

#[test]
fn ws_ref_no_ws_commit_two_stacks_on_same_commit() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-no-ws-commit-one-stack-one-branch",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542");

    // Put "A" into the workspace, yielding a single branch.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    let graph = out.graph;
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    └── ≡📙:2:A on e5d0542
        └── 📙:2:A
    ");
    // No commit was created, as it's not enabled by default.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    // Note how it will create a new stack (to keep it simple),
    // in theory we could also add B as dependent branch.
    insta::assert_snapshot!(graph_workspace(&out.graph.to_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:3:B on e5d0542
    │   └── 📙:3:B
    └── ≡📙:2:A on e5d0542
        └── 📙:2:A
    ");

    // Nothing changed visibly, still, it's all in the metadata.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    // TODO: create
    // TODO: commit/uncommit
    // TODO: unapply

    Ok(())
}

#[test]
fn no_ws_ref_no_ws_commit_two_stacks_on_same_commit_ad_hoc_workspace_without_target_branch()
-> anyhow::Result<()> {
    let (_tmp, _, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-no-ws-commit-two-branches",
            |_meta| {},
        )?;

    // Delete the target branch.
    {
        let mut ws_md = meta.workspace("refs/heads/gitbutler/workspace".try_into().unwrap())?;
        assert!(ws_md.target_ref.is_some());
        ws_md.target_ref.take();
        meta.set_workspace(&ws_md)?;
        let ws_md = meta.workspace("refs/heads/gitbutler/workspace".try_into().unwrap())?;
        assert!(
            ws_md.target_ref.is_none(),
            "we just deleted it, it should be transferred"
        );
    }
    let graph = but_graph::Graph::from_head(&repo, &meta, standard_traversal_options())?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> main, origin/main, B, A) A");
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
            └── ·e5d0542 ►A, ►B
    ");

    // Put "A" into the workspace, creating the workspace ref, but never put a branch related to the target in as well,
    // which is currently checked out with `main`.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
    }
    ");

    let graph = out.graph;
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:3:A on e5d0542
    │   └── 📙:3:A
    └── ≡📙:2:main on e5d0542
        └── 📙:2:main
    ");

    // No commit was created, as it's not enabled by default, but a ws-ref was created, and it's checked out.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    let out = but_workspace::branch::apply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // Make it appear in place of A, in the center.
            order: Some(1),
            ..default_options()
        },
    )?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:4:A on e5d0542
    │   └── 📙:4:A
    ├── ≡📙:3:B on e5d0542
    │   └── 📙:3:B
    └── ≡📙:2:main on e5d0542
        └── 📙:2:main
    ");

    // Nothing changed visibly, still, it's all in the metadata.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    // Reset the workspace to 'unapply', but keep the per-branch metadata.
    let mut ws_md = meta.workspace(ws.ref_name().expect("proper gb workspace"))?;
    for stack in &mut ws_md.stacks {
        stack.in_workspace = false;
    }
    meta.set_workspace(&ws_md)?;

    let graph = graph.redo_traversal_with_overlay(&repo, &meta, Overlay::default())?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓!
    └── ≡:1:anon:
        └── :1:anon:
            └── ·e5d0542 (🏘️) ►A, ►B, ►main
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            integration_mode: IntegrationMode::AlwaysMerge,
            ..default_options()
        },
    )?;
    // A workspace commit was created, even though it does nothing.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 5169839 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * e5d0542 (origin/main, main, B, A) A
    ");

    let ws = out.graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓!
    └── ≡📙:2:A
        └── 📙:2:A
            └── ·e5d0542 (🏘️) ►B, ►main
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/B"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            integration_mode: IntegrationMode::AlwaysMerge,
            ..default_options()
        },
    )?;

    // It's idempotent, but has to update the workspace commit nonetheless for the comment, which depends on the stacks.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4f21fe4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\
    * e5d0542 (origin/main, main, B, A) A
    ");

    let ws = out.graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:3:A on e5d0542
    │   └── 📙:3:A
    └── ≡📙:2:B on e5d0542
        └── 📙:2:B
    ");

    Ok(())
}

#[test]
fn no_ws_ref_no_ws_commit_two_stacks_on_same_commit_ad_hoc_workspace_with_target()
-> anyhow::Result<()> {
    let (_tmp, _, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-no-ws-commit-two-branches",
            |_meta| {},
        )?;

    let graph = but_graph::Graph::from_head(&repo, &meta, standard_traversal_options())?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> main, origin/main, B, A) A");
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
            └── ·e5d0542 ►A, ►B
    ");

    // Put "A" into the workspace, creating the workspace ref, but never put a branch related to the target in as well,
    // which is currently checked out with `main`.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
    }
    ");

    let graph = out.graph;
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on e5d0542
    └── ≡📙:3:A on e5d0542
        └── 📙:3:A
    ");

    // No commit was created, as it's not enabled by default, but a ws-ref was created, and it's checked out.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on e5d0542
    ├── ≡📙:4:B on e5d0542
    │   └── 📙:4:B
    └── ≡📙:3:A on e5d0542
        └── 📙:3:A
    ");

    // Nothing changed visibly, still, it's all in the metadata.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, origin/main, main, B, A) A");

    // Cannot put local tracking branch of target into workspace that has it configured.
    for branch in ["refs/heads/main", "refs/remotes/origin/main"] {
        let err = but_workspace::branch::apply(r(branch), &ws, &repo, &mut meta, default_options())
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("Cannot add the target '{branch}' branch to its own workspace")
        );
    }

    // TODO: commit/uncommit
    // TODO: unapply

    Ok(())
}

#[test]
fn new_workspace_exists_elsewhere_and_to_be_applied_branch_exists_there() -> anyhow::Result<()> {
    let (_tmp, ws_graph, repo, mut meta, _description) =
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
    // Note how the existing `gitbutler/workspace` disappears, which is expected here.
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:1:B <> ✓!
    └── ≡:1:B
        └── :1:B
            └── ·e5d0542 ►A, ►main
    ");

    // Put "A" into the workspace, hence we want "A" and "B" in it.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");

    let ws = out.graph.to_workspace()?;
    // This apply brings both branches into the existing workspace, and it's where HEAD points to.
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:3:A on e5d0542
    │   └── 📙:3:A
    └── ≡📙:2:B on e5d0542
        └── 📙:2:B
    ");

    // HEAD must now point to the workspace (that already existed)
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    Ok(())
}

#[test]
fn detached_head_journey() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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

    let out =
        but_workspace::branch::apply(r("refs/heads/C"), &ws, &repo, &mut meta, default_options())?;

    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
    }
    ");

    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on 3183e43
    └── ≡📙:2:C on 3183e43
        └── 📙:2:C
            └── ·aaa195b (🏘️)
    ");
    // A new workspace reference was created, and checked out, without enforcing a workspace commit
    // as there is no need.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 49d4b34 (A) A1
    | * f57c528 (B) B1
    |/  
    | * aaa195b (HEAD -> gitbutler/workspace, C) C1
    |/  
    * 3183e43 (main) M1
    ");

    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, default_options())?;

    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f2d8a20 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * f57c528 (B) B1
    * | aaa195b (C) C1
    |/  
    | * 49d4b34 (A) A1
    |/  
    * 3183e43 (main) M1
    ");

    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓! on 3183e43
    ├── ≡📙:3:B on 3183e43
    │   └── 📙:3:B
    │       └── ·f57c528 (🏘️)
    └── ≡📙:2:C on 3183e43
        └── 📙:2:C
            └── ·aaa195b (🏘️)
    ");

    let out = but_workspace::branch::apply(
        r("refs/heads/A"),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            // Make 'A' appear at the front.
            order: Some(0),
            ..default_options()
        },
    )?;

    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓! on 3183e43
    ├── ≡📙:4:B on 3183e43
    │   └── 📙:4:B
    │       └── ·f57c528 (🏘️)
    ├── ≡📙:3:C on 3183e43
    │   └── 📙:3:C
    │       └── ·aaa195b (🏘️)
    └── ≡📙:2:A on 3183e43
        └── 📙:2:A
            └── ·49d4b34 (🏘️)
    ");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   40e102f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * f57c528 (B) B1
    | * | aaa195b (C) C1
    | |/  
    * / 49d4b34 (A) A1
    |/  
    * 3183e43 (main) M1
    ");
    Ok(())
}

#[test]
fn apply_two_ambiguous_stacks_with_target_with_dependent_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-stack-and-dependent-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "C", StackState::Inactive, &["E"]);
                add_stack_with_segments(meta, 2, "B", StackState::Inactive, &["D"]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (HEAD -> main, origin/main) M
    ");

    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
            └── ·85efbe4
    ");

    // Apply the dependent branch, to bring in only the dependent branch
    let out =
        but_workspace::branch::apply(r("refs/heads/E"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
    }
    ");

    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:E on 85efbe4
        └── 📙:4:E
            └── ·7076dee (🏘️) ►D
    ");

    // Apply the former tip of the stack, to create a new stack. Note how it won't double-list the
    // other stack.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), &ws, &repo, &mut meta, default_options())?;
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:C on 7076dee
    │   └── 📙:5:C
    │       └── ·f084d61 (🏘️) ►A, ►B
    └── ≡📙:6:E on 85efbe4
        └── 📙:6:E
            └── ·7076dee (🏘️) ►D
    ");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   ef9bcae (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * f084d61 (C, B, A) A2
    |/  
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    // Adding `B` as tip of an unapplied stack brings in the whole stack.
    // BUT: Currently it overrides the previous stack C, which points to the same commit, and avoids any merge!
    // Accepting this behaviour for now as it's quite rare to have such ambiguity, even though I'd love if one day
    // for this to just work as people might intuitively want, even if that means the same commit is used multiple times.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, default_options())?;
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:6:B on 7076dee
    │   └── 📙:6:B
    │       └── ·f084d61 (🏘️) ►A, ►C
    └── ≡📙:5:E on 85efbe4
        └── 📙:5:E
            └── ·7076dee (🏘️) ►D
    ");

    // Applying C again… works, but it's creating a dependent stack.
    // This is what happens because we notice that C can't be applied as independent stack due to the graph algorithm,
    // and then it tries it a dependent stack, which should always work.
    let out =
        but_workspace::branch::apply(r("refs/heads/C"), &ws, &repo, &mut meta, default_options())?;
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:C on 7076dee
    │   ├── 📙:5:C
    │   └── 📙:6:B
    │       └── ·f084d61 (🏘️) ►A
    └── ≡📙:7:E on 85efbe4
        └── 📙:7:E
            └── ·7076dee (🏘️) ►D
    ");

    Ok(())
}

#[test]
fn apply_two_ambiguous_stacks_with_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "no-ws-ref-stack-and-dependent-branch",
            |_meta| {},
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (HEAD -> main, origin/main) M
    ");

    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
            └── ·85efbe4
    ");

    // Apply `A` first.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: true,
    }
    ");
    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:A on 85efbe4
        └── 📙:3:A
            ├── ·f084d61 (🏘️) ►B, ►C
            └── ·7076dee (🏘️) ►D, ►E
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8444317 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    // Apply `B` - the only sane way is to make it its own stack, but allow it to diverge.
    let out =
        but_workspace::branch::apply(r("refs/heads/B"), &ws, &repo, &mut meta, default_options())
            .expect("apply actually works");
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");

    let graph = out.graph;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4
        ├── 📙:4:B
        └── 📙:5:A
            ├── ·f084d61 (🏘️) ►C
            └── ·7076dee (🏘️) ►D, ►E
    ");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 102321c (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f084d61 (C, B, A) A2
    * 7076dee (E, D) A1
    * 85efbe4 (origin/main, main) M
    ");

    // TODO: add all other dependent branches as well.
    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_flat() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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
        &repo,
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
    let graph = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        standard_traversal_options_with_extra_target(&repo),
    )?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡👉📙:3:B on e5d0542
    │   └── 👉📙:3:B
    └── ≡📙:2:A on e5d0542
        └── 📙:2:A
    ");
    // Already applied (the HEAD points to it, it literally IS the workspace).
    let out =
        but_workspace::branch::apply(b_ref.as_ref(), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    // To apply A, we just checkout the surrounding workspace, as it's contained there.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");

    // Now the workspace ref itself is checked out.
    let ws = out.graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    ├── ≡📙:3:B on e5d0542
    │   └── 📙:3:B
    └── ≡📙:2:A on e5d0542
        └── 📙:2:A
    ");
    // Even though the real repo seemingly didn't change, after all, our entrypoint was just 'virtual'.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* e5d0542 (HEAD -> gitbutler/workspace, main, B, A) A");

    // make "A" an applied dependent branch that is included in B so apply will do nothing.
    meta.data_mut().branches.clear();
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &["A"]);

    let (b_id, b_ref) = id_at(&repo, "B");
    let graph = but_graph::Graph::from_commit_traversal(
        b_id,
        b_ref.clone(),
        &meta,
        standard_traversal_options_with_extra_target(&repo),
    )?;

    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️⚠️:1:gitbutler/workspace <> ✓! on e5d0542
    └── ≡👉📙:2:B on e5d0542
        ├── 👉📙:2:B
        └── 📙:3:A
    ");

    // Nothing changed, the desired branch was already applied.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    // There is no known branch, and adding it will just add metadata.
    meta.data_mut().branches.clear();
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        standard_traversal_options_with_extra_target(&repo),
    )?;
    // There is nothing yet.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542");

    // Apply the first branch, it must be independent.
    let ws = graph.to_workspace()?;
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: true,
        workspace_ref_created: false,
    }
    ");
    let graph = out.graph;
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
    📕🏘️⚠️:0:gitbutler/workspace <> ✓! on e5d0542
    └── ≡📙:2:A on e5d0542
        └── 📙:2:A
    ");

    // TODO: apply second branch as new stack.

    // NOTE: we could also do it as independent branch, but that just adds complexity and it's unclear when this will be used.
    Ok(())
}

#[test]
fn auto_checkout_of_enclosing_workspace_with_commits() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
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
    let out = but_workspace::branch::apply(ws_ref, &ws, &repo, &mut meta, default_options())?;
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
        but_workspace::branch::apply(b_ref.as_ref(), &ws, &repo, &mut meta, default_options())?;
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        workspace_changed: false,
        workspace_ref_created: false,
    }
    ");

    let err =
        but_workspace::branch::apply(ws_ref, &ws, &repo, &mut meta, default_options()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to apply a reference that already is a workspace: 'gitbutler/workspace'",
        "it's never good to merge one managed workspace into another, and we just disallow it.\
         Note that we could also check it out."
    );

    // To apply, we just checkout the surrounding workspace.
    let out =
        but_workspace::branch::apply(r("refs/heads/A"), &ws, &repo, &mut meta, default_options())?;
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
fn apply_nonexisting_branch_failure() -> anyhow::Result<()> {
    let (repo, mut meta) =
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
        &repo,
        &mut *meta,
        default_options(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot apply non-existing branch 'does-not-exist'"
    );

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
    let (repo, mut meta) =
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
        &repo,
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
        &repo,
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
        on_workspace_conflict: OnWorkspaceMergeConflict::AbortAndReportConflictingStacks,
        workspace_reference_naming: WorkspaceReferenceNaming::Default,
        uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
        order: None,
    }
}

mod utils {
    pub fn standard_traversal_options() -> but_graph::init::Options {
        but_graph::init::Options {
            collect_tags: true,
            commits_limit_hint: None,
            commits_limit_recharge_location: vec![],
            hard_limit: None,
            extra_target_commit_id: None,
            dangerously_skip_postprocessing_for_debugging: false,
        }
    }

    pub fn standard_traversal_options_with_extra_target(
        repo: &gix::Repository,
    ) -> but_graph::init::Options {
        but_graph::init::Options {
            extra_target_commit_id: Some(repo.rev_parse_single("main").expect("present").detach()),
            ..standard_traversal_options()
        }
    }
}
use utils::{standard_traversal_options, standard_traversal_options_with_extra_target};

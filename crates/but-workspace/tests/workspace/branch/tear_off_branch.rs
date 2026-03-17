use but_core::RefMetadata;
use but_rebase::graph_rebase::GraphExt;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use gitbutler_stack::StackId;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn tear_off_top_most_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | 09bc93e (C) C
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·09bc93e (🏘️)
    │   └── 📙:5:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Tear off C from the stack.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(
            editor,
            &ws,
            "refs/heads/C".try_into()?,
            Some(StackId::from_number_for_testing(3)),
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   18e6497 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * 09d8e52 (A) A
    | * | c813d8d (B) B
    | |/  
    * / 9f14615 (C) C
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:C on 85efbe4 {3}
    │   └── 📙:5:C
    │       └── ·9f14615 (🏘️)
    ├── ≡📙:4:B on 85efbe4 {2}
    │   └── 📙:4:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn tear_off_bottom_most_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | 09bc93e (C) C
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·09bc93e (🏘️)
    │   └── 📙:5:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(
            editor,
            &ws,
            "refs/heads/B".try_into()?,
            Some(StackId::from_number_for_testing(3)),
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   f365cbc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\ \  
    | | * 9f14615 (C) C
    | * | 09d8e52 (A) A
    | |/  
    * / c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:B on 85efbe4 {3}
    │   └── 📙:5:B
    │       └── ·c813d8d (🏘️)
    ├── ≡📙:4:C on 85efbe4 {2}
    │   └── 📙:4:C
    │       └── ·9f14615 (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn tear_off_only_branch_in_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | 09bc93e (C) C
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·09bc93e (🏘️)
    │   └── 📙:5:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Tear off A from the stack. Should be a no-op.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(
            editor,
            &ws,
            "refs/heads/A".try_into()?,
            Some(StackId::from_number_for_testing(3)),
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   290169a (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09bc93e (C) C
    | * c813d8d (B) B
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·09bc93e (🏘️)
    │   └── 📙:5:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn tear_off_from_single_stack_in_ws_top() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-ref-ws-commit-one-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * d69fe94 (B) B
    * 09d8e52 (A) A
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {2}
        ├── 📙:4:B
        │   └── ·d69fe94 (🏘️)
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(
            editor,
            &ws,
            "refs/heads/B".try_into()?,
            Some(StackId::from_number_for_testing(3)),
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   40098a7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | 5dab59a (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {2}
    │   └── 📙:4:B
    │       └── ·5dab59a (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn tear_off_from_single_stack_in_ws_bottom() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-ref-ws-commit-one-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * d69fe94 (B) B
    * 09d8e52 (A) A
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {2}
        ├── 📙:4:B
        │   └── ·d69fe94 (🏘️)
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Tear off A from the stack.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(
            editor,
            &ws,
            "refs/heads/A".try_into()?,
            Some(StackId::from_number_for_testing(3)),
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   ce8d25d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 5dab59a (B) B
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:B on 85efbe4 {2}
        └── 📙:3:B
            └── ·5dab59a (🏘️)
    ");

    Ok(())
}

#[test]
fn tear_off_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-with-empty-top-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["A"]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * d990875 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 09d8e52 (B, A) A
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {1}
        ├── 📙:4:B
        └── 📙:5:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Tear off B from the stack.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(
            editor,
            &ws,
            "refs/heads/B".try_into()?,
            Some(StackId::from_number_for_testing(3)),
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   de62bba (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {3}
    │   └── 📙:4:B
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn tear_off_non_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-one-stack-with-empty-top-branch",
            |meta| {
                add_stack_with_segments(meta, 1, "B", StackState::InWorkspace, &["A"]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * d990875 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 09d8e52 (B, A) A
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {1}
        ├── 📙:4:B
        └── 📙:5:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Tear off A from the stack.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(
            editor,
            &ws,
            "refs/heads/A".try_into()?,
            Some(StackId::from_number_for_testing(3)),
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   4b2d718 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {3}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:4:B on 85efbe4 {1}
        └── 📙:4:B
    ");

    Ok(())
}

fn set_workspace_metadata(
    meta: &mut impl RefMetadata,
    ws: &but_graph::projection::Workspace,
    ws_meta: Option<but_core::ref_metadata::Workspace>,
) -> anyhow::Result<()> {
    if let Some((ws_meta, ref_name)) = ws_meta.zip(ws.ref_name()) {
        let mut md = meta.workspace(ref_name)?;
        *md = ws_meta;
        meta.set_workspace(&md)?;
    }
    Ok(())
}

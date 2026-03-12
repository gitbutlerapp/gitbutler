use but_core::RefMetadata;
use but_rebase::graph_rebase::GraphExt;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn move_top_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
    // Put C on top of A
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            &ws,
            "refs/heads/C".try_into()?,
            "refs/heads/A".try_into()?,
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   26fdd46 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * c813d8d (B) B
    * | 3db8c14 (C) C
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:B on 85efbe4 {2}
    │   └── 📙:5:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:4:C on 85efbe4 {1}
        ├── 📙:4:C
        │   └── ·3db8c14 (🏘️)
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_bottom_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            &ws,
            "refs/heads/B".try_into()?,
            "refs/heads/A".try_into()?,
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   ac869ab (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f14615 (C) C
    * | 698ccd3 (B) B
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   └── 📙:4:C
    │       └── ·9f14615 (🏘️)
    └── ≡📙:5:B on 85efbe4 {1}
        ├── 📙:5:B
        │   └── ·698ccd3 (🏘️)
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_single_branch_to_top_of_another_stack() -> anyhow::Result<()> {
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
    // Put A on top of C
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            &ws,
            "refs/heads/A".try_into()?,
            "refs/heads/C".try_into()?,
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 263392f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 8dbfefa (A) A
    * 09bc93e (C) C
    * c813d8d (B) B
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:A on 85efbe4 {1}
        ├── 📙:3:A
        │   └── ·8dbfefa (🏘️)
        ├── 📙:4:C
        │   └── ·09bc93e (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    Ok(())
}

#[test]
fn reorder_branch_in_stack() -> anyhow::Result<()> {
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
    // Put B on top of C
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            &ws,
            "refs/heads/B".try_into()?,
            "refs/heads/C".try_into()?,
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   82661e2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 2c58ac6 (B) B
    | * 9f14615 (C) C
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:B on 85efbe4 {2}
    │   ├── 📙:5:B
    │   │   └── ·2c58ac6 (🏘️)
    │   └── 📙:4:C
    │       └── ·9f14615 (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn insert_branch_in_the_middle_of_a_stack() -> anyhow::Result<()> {
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
    // Put A on top of B, and below C
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            &ws,
            "refs/heads/A".try_into()?,
            "refs/heads/B".try_into()?,
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 35a28f3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 531d8aa (C) C
    * 3df48f1 (A) A
    * c813d8d (B) B
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:C on 85efbe4 {2}
        ├── 📙:4:C
        │   └── ·531d8aa (🏘️)
        ├── 📙:3:A
        │   └── ·3df48f1 (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    Ok(())
}

#[test]
fn move_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {2}
    │   └── 📙:4:B
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Put B on top of A
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            &ws,
            "refs/heads/B".try_into()?,
            "refs/heads/A".try_into()?,
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * ee3cff8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 09d8e52 (B, A) A
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:4:B on 85efbe4 {2}
        ├── 📙:4:B
        └── 📙:5:A
            └── ·09d8e52 (🏘️)
    ");
    Ok(())
}

#[test]
fn move_branch_on_top_of_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {2}
    │   └── 📙:4:B
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Put A on top of B
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            &ws,
            "refs/heads/A".try_into()?,
            "refs/heads/B".try_into()?,
        )?;

    // Materialize the operation
    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * ee3cff8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 09d8e52 (A) A
    * 85efbe4 (origin/main, main, B) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:A on 85efbe4 {1}
        ├── 📙:3:A
        │   └── ·09d8e52 (🏘️)
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

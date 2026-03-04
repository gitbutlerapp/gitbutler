use but_rebase::graph_rebase::GraphExt;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn move_top_branch_to_top_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::Inactive, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::Inactive, &["B"]);
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
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:C on 85efbe4 {2}
        ├── 📙:3:C
        │   └── ·09bc93e (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Put C on top of A
    let out = but_workspace::branch::move_branch(
        &ws,
        editor,
        "refs/heads/C".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    out.rebase.materialize()?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   30f5dcd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 3db8c14 (C) C
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·3db8c14 (🏘️)
    │   └── 📙:5:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:B on 85efbe4
        └── 📙:3:B
            └── ·c813d8d (🏘️)
    ");

    Ok(())
}

#[test]
fn move_bottom_branch_to_top_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::Inactive, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::Inactive, &["B"]);
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
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:C on 85efbe4 {2}
        ├── 📙:3:C
        │   └── ·09bc93e (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    let out = but_workspace::branch::move_branch(
        &ws,
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    )?;

    // Materialize the operation
    out.rebase.materialize()?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   4750b1b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 698ccd3 (B) B
    | * 09d8e52 (A) A
    * | 9f14615 (C) C
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {1}
    │   ├── 📙:4:B
    │   │   └── ·698ccd3 (🏘️)
    │   └── 📙:5:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:C on 85efbe4 {2}
        └── 📙:3:C
            └── ·9f14615 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_single_branch_to_top_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::Inactive, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::Inactive, &["B"]);
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
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:C on 85efbe4 {2}
        ├── 📙:3:C
        │   └── ·09bc93e (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Put A on top of C
    let out = but_workspace::branch::move_branch(
        &ws,
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/C".try_into()?,
    )?;

    // Materialize the operation
    out.rebase.materialize()?;
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
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::Inactive, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::Inactive, &["B"]);
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
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:C on 85efbe4 {2}
        ├── 📙:3:C
        │   └── ·09bc93e (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Put B on top of C
    let out = but_workspace::branch::move_branch(
        &ws,
        editor,
        "refs/heads/B".try_into()?,
        "refs/heads/C".try_into()?,
    )?;

    // Materialize the operation
    out.rebase.materialize()?;
    ws.refresh_from_head(&repo, &meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   d1c848d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | 2c58ac6 (B) B
    * | 9f14615 (C) C
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:B on 85efbe4 {2}
        ├── 📙:3:B
        │   └── ·2c58ac6 (🏘️)
        └── 📙:5:C
            └── ·9f14615 (🏘️)
    ");

    Ok(())
}

#[test]
fn insert_branch_in_the_middle_of_a_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::Inactive, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::Inactive, &["B"]);
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
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:3:C on 85efbe4 {2}
        ├── 📙:3:C
        │   └── ·09bc93e (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    let editor = ws.graph.to_editor(&repo)?;
    // Put A on top of B, and below C
    let out = but_workspace::branch::move_branch(
        &ws,
        editor,
        "refs/heads/A".try_into()?,
        "refs/heads/B".try_into()?,
    )?;

    // Materialize the operation
    out.rebase.materialize()?;
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
    └── ≡📙:3:C on 85efbe4 {2}
        ├── 📙:3:C
        │   └── ·531d8aa (🏘️)
        ├── 📙:4:A
        │   └── ·3df48f1 (🏘️)
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    Ok(())
}

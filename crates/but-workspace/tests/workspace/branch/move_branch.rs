use but_core::RefMetadata;
use but_core::ref_metadata::StackKind;
use but_rebase::graph_rebase::Editor;
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

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put C on top of A
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
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
    └── ≡📙:3:C on 85efbe4 {1}
        ├── 📙:3:C
        │   └── ·3db8c14 (🏘️)
        └── 📙:4:A
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

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
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
    ├── ≡📙:5:C on 85efbe4 {2}
    │   └── 📙:5:C
    │       └── ·9f14615 (🏘️)
    └── ≡📙:3:B on 85efbe4 {1}
        ├── 📙:3:B
        │   └── ·698ccd3 (🏘️)
        └── 📙:4:A
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

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put A on top of C
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
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
    └── ≡📙:3:A on 85efbe4 {2}
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

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put B on top of C
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
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
    ├── ≡📙:4:B on 85efbe4 {2}
    │   ├── 📙:4:B
    │   │   └── ·2c58ac6 (🏘️)
    │   └── 📙:5:C
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

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put A on top of B, and below C
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
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

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put B on top of A
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
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
    └── ≡📙:4:B on 85efbe4 {1}
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

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put A on top of B
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
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
    └── ≡📙:3:A on 85efbe4 {2}
        ├── 📙:3:A
        │   └── ·09d8e52 (🏘️)
        └── 📙:4:B
    ");
    Ok(())
}

#[test]
fn non_empty_move_updates_metadata_but_still_desyncs_display_order() -> anyhow::Result<()> {
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
    let before_display_order = stack_display_order(&ws);
    let before_metadata_order = metadata_stack_order(&ws);
    assert_ne!(before_display_order, before_metadata_order);

    // Move non-empty C on top of non-empty A.
    // This now rewrites metadata as intended, but display order is still inverted
    // relative to metadata in this two-stack scenario.
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            "refs/heads/C".try_into()?,
            "refs/heads/A".try_into()?,
        )?;

    let updated_metadata_order = ws_meta
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::Applied))
        .unwrap_or_default();

    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    let after_display_order = stack_display_order(&ws);

    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);
    assert_ne!(after_display_order, updated_metadata_order);

    insta::assert_snapshot!(format!("{before_display_order:#?}"), @r#"
    [
        "refs/heads/C",
        "refs/heads/A",
    ]
    "#);

    insta::assert_snapshot!(format!("{after_display_order:#?}"), @r#"
    [
        "refs/heads/B",
        "refs/heads/C",
    ]
    "#);

    Ok(())
}

#[test]
fn empty_move_keeps_display_order_aligned_with_metadata() -> anyhow::Result<()> {
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
    let before_display_order = stack_display_order(&ws);
    let before_metadata_order = metadata_stack_order(&ws);
    assert_ne!(before_display_order, before_metadata_order);

    // Move empty B on top of non-empty A.
    // This path rewrites metadata and keeps display + metadata aligned.
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            "refs/heads/B".try_into()?,
            "refs/heads/A".try_into()?,
        )?;

    let updated_metadata_order = ws_meta
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::AppliedAndUnapplied))
        .unwrap_or_default();

    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    ws.refresh_from_head(&repo, &meta)?;

    let after_display_order = stack_display_order(&ws);

    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);
    assert_eq!(after_display_order, updated_metadata_order);

    insta::assert_snapshot!(format!("{before_display_order:#?}"), @r#"
    [
        "refs/heads/B",
        "refs/heads/A",
    ]
    "#);

    insta::assert_snapshot!(format!("{after_display_order:#?}"), @r#"
    [
        "refs/heads/B",
    ]
    "#);

    Ok(())
}

fn stack_display_order(ws: &but_graph::projection::Workspace) -> Vec<String> {
    ws.stacks
        .iter()
        .filter_map(|stack| stack.ref_name())
        .map(|name| name.to_string())
        .collect()
}

fn metadata_stack_order(ws: &but_graph::projection::Workspace) -> Vec<String> {
    ws.metadata
        .as_ref()
        .map(|ws_meta| workspace_metadata_stack_order(ws_meta, StackKind::Applied))
        .unwrap_or_default()
}

fn workspace_metadata_stack_order(
    ws_meta: &but_core::ref_metadata::Workspace,
    kind: StackKind,
) -> Vec<String> {
    ws_meta
        .stacks(kind)
        .filter_map(|stack| stack.name())
        .map(|name| name.to_string())
        .collect()
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

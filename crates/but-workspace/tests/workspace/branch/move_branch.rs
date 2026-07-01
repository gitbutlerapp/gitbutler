use but_core::RefMetadata;
use but_core::ref_metadata::StackKind;
use but_graph::init::Options;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_workspace, invoke_bash, visualize_commit_graph_all};

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description,
    named_writable_scenario_with_description_and_graph,
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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   bdcbf64 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * c813d8d (B) B
    * | f2cc60d (C) C
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:C on 85efbe4 {1}
    │   ├── 📙:3:C
    │   │   └── ·f2cc60d (🏘️)
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:5:B on 85efbe4 {2}
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    Ok(())
}

#[test]
fn moving_branch_onto_itself_fails_without_changing_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;

    let mut ws = graph.into_workspace()?;
    let before = graph_workspace(&ws).to_string();
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let err = but_workspace::branch::move_branch(
        editor,
        "refs/heads/C".try_into()?,
        "refs/heads/C".try_into()?,
    )
    .expect_err("moving a branch onto itself should fail before graph mutation");

    assert_eq!(
        err.to_string(),
        "Cannot move branch refs/heads/C onto itself"
    );
    assert_eq!(
        graph_workspace(&ws).to_string(),
        before,
        "workspace projection should stay unchanged after rejected self-move"
    );

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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   9c6a201 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * f9061ed (B) B
    | * 09d8e52 (A) A
    * | 8e00332 (C) C
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:C on 85efbe4 {2}
    │   └── 📙:5:C
    │       └── ·8e00332 (🏘️)
    └── ≡📙:3:B on 85efbe4 {1}
        ├── 📙:3:B
        │   └── ·f9061ed (🏘️)
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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 4c58dd4 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 148f8f3 (A) A
    * 09bc93e (C) C
    * c813d8d (B) B
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:A on 85efbe4 {2}
        ├── 📙:3:A
        │   └── ·148f8f3 (🏘️)
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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c6b8b22 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | de0581e (B) B
    * | 8e00332 (C) C
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {2}
    │   ├── 📙:4:B
    │   │   └── ·de0581e (🏘️)
    │   └── 📙:5:C
    │       └── ·8e00332 (🏘️)
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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 0c5cde5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 3e7ff55 (C) C
    * 4dfe841 (A) A
    * c813d8d (B) B
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    └── ≡📙:3:C on 85efbe4 {2}
        ├── 📙:3:C
        │   └── ·3e7ff55 (🏘️)
        ├── 📙:4:A
        │   └── ·4dfe841 (🏘️)
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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
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
fn move_empty_branch_on_top_of_empty_branch_in_same_stack() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("empty-workspace-target-advanced")?;
    invoke_bash(
        "git branch A gitbutler/target\ngit branch B gitbutler/target\n",
        &repo,
    );
    add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &["A"]);

    let project_meta = meta
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?
        .project_meta();
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta,
        Options {
            extra_target_commit_id: repo
                .rev_parse_single("gitbutler/target")
                .ok()
                .map(|id| id.detach()),
            ..Options::limited()
        },
    )?;

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    └── ≡📙:4:B on 3183e43 {1}
        ├── 📙:4:B
        └── 📙:5:A
    ");

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            "refs/heads/A".try_into()?,
            "refs/heads/B".try_into()?,
        )?;

    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    └── ≡📙:4:A on 3183e43 {1}
        ├── 📙:4:A
        └── 📙:5:B
    ");

    Ok(())
}

#[test]
fn move_empty_branch_on_top_of_empty_branch_across_stacks() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("empty-workspace-target-advanced")?;
    invoke_bash(
        "git branch A gitbutler/target\ngit branch B gitbutler/target\n",
        &repo,
    );
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

    let project_meta = meta
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?
        .project_meta();
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta,
        Options {
            extra_target_commit_id: repo
                .rev_parse_single("gitbutler/target")
                .ok()
                .map(|id| id.detach()),
            ..Options::limited()
        },
    )?;

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    ├── ≡📙:4:A on 3183e43 {1}
    │   └── 📙:4:A
    └── ≡📙:5:B on 3183e43 {2}
        └── 📙:5:B
    ");

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            "refs/heads/A".try_into()?,
            "refs/heads/B".try_into()?,
        )?;

    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
    └── ≡📙:4:A on 3183e43 {2}
        ├── 📙:4:A
        └── 📙:5:B
    ");

    Ok(())
}

#[test]
fn non_empty_move_display_order_follows_workspace_parents() -> anyhow::Result<()> {
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
    let before_display_order = stack_display_order(&ws);
    let before_metadata_order = metadata_stack_order(&ws);
    // Stack order comes from the workspace-commit parent array, not metadata. This
    // fixture bakes a parent order that differs from the injected metadata order, so
    // the two differ before the move — and the parent array is what wins.
    assert_ne!(before_display_order, before_metadata_order);

    // Move non-empty C on top of non-empty A. The displayed order then follows the
    // rebuilt workspace-commit parent array.
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

    insta::assert_snapshot!(graph_workspace(&ws), "before refreshing `ws` the pure-virtual change isn't visible (should be fixed once meta is in db!)", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·f2cc60d (🏘️)
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:5:B on 85efbe4
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), "after the refresh the workspace is finally uptodate (this will probably be an issue unless callers know that)", @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:C on 85efbe4 {1}
    │   ├── 📙:3:C
    │   │   └── ·f2cc60d (🏘️)
    │   └── 📙:4:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:5:B on 85efbe4 {2}
        └── 📙:5:B
            └── ·c813d8d (🏘️)
    ");

    let after_display_order = stack_display_order(&ws);

    // The move changes both the stored metadata order and the displayed order.
    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);
    // Stack order is taken from the workspace-commit parent array (the source of
    // truth), not metadata order: after the move the parents are [B, C] while
    // metadata is [C, B]. The order snapshots below capture the parent-array order.

    insta::assert_snapshot!(format!("{before_display_order:#?}"), @r#"
    [
        "refs/heads/C",
        "refs/heads/A",
    ]
    "#);

    insta::assert_snapshot!(format!("{after_display_order:#?}"), @r#"
    [
        "refs/heads/C",
        "refs/heads/B",
    ]
    "#);

    Ok(())
}

#[test]
fn empty_move_display_order_follows_workspace_parents() -> anyhow::Result<()> {
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
    // Stack order comes from the workspace-commit parent array, not metadata. The
    // empty lane sits at the base (parent 0) and so leads here; `explicit-empty-branches`
    // will give empty lanes their own distinct parent positions.
    assert_ne!(before_display_order, before_metadata_order);

    // Move empty B on top of non-empty A. The displayed order then follows the
    // rebuilt workspace-commit parent array.
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
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    let after_display_order = stack_display_order(&ws);

    // The move changes both the stored metadata order and the displayed order; the
    // displayed order is taken from the workspace-commit parent array (see snapshot).
    assert_ne!(updated_metadata_order, before_metadata_order);
    assert_ne!(after_display_order, before_display_order);

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

#[test]
fn move_branch_when_base_segment_has_no_ref_name() -> anyhow::Result<()> {
    // When origin/main advances past the fork point, the old fork commit becomes
    // an unnamed base segment. Moving a branch should still work by falling back
    // to selecting by the segment's tip commit.
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-two-stacks-advanced-remote",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   a236c53 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * c813d8d (B) B
    * | 09d8e52 (A) A
    |/  
    | * 148c87a (origin/main) M2
    |/  
    * 85efbe4 (main) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:4:B on 85efbe4 {2}
        └── 📙:4:B
            └── ·c813d8d (🏘️)
    ");

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Move B on top of A — the base segment at the old fork point has no ref name.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            "refs/heads/B".try_into()?,
            "refs/heads/A".try_into()?,
        )?;

    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 148c87a (origin/main) M2
    | * 0db3c2f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * f9061ed (B) B
    | * 09d8e52 (A) A
    |/  
    * 85efbe4 (main) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
    └── ≡📙:3:B on 85efbe4 {1}
        ├── 📙:3:B
        │   └── ·f9061ed (🏘️)
        └── 📙:4:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_empty_branch_onto_non_empty_branch_with_advanced_target() -> anyhow::Result<()> {
    // Regression: when the target branch (local `main`/`origin/main`) is ahead of the workspace
    // base, the merge-base segment is represented in the editor graph by the `gitbutler/target`
    // reference node sitting above the base commit. Selecting the base by commit would point one
    // hop too far and fail the direct-parent check. Moving the empty branch onto the non-empty one
    // must still succeed.
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-with-empty-stack-target-advanced",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    |/  
    | * e1bbad3 (origin/main, main) add X
    |/  
    * 85efbe4 (gitbutler/target, B) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:5:B on 85efbe4 {2}
        └── 📙:5:B
    ");

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put empty B on top of non-empty A.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            "refs/heads/B".try_into()?,
            "refs/heads/A".try_into()?,
        )?;

    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 09d8e52 (B, A) A
    | * e1bbad3 (origin/main, main) add X
    |/  
    * 85efbe4 (gitbutler/target) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
    └── ≡📙:5:B on 85efbe4 {1}
        ├── 📙:5:B
        └── 📙:6:A
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_non_empty_branch_onto_empty_branch_with_advanced_target() -> anyhow::Result<()> {
    // Same setup as the empty-onto-non-empty regression, but the subject is the non-empty branch
    // and the target is the empty one. Both directions must succeed when the target is ahead.
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-with-empty-stack-target-advanced",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    |/  
    | * e1bbad3 (origin/main, main) add X
    |/  
    * 85efbe4 (gitbutler/target, B) M
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:5:B on 85efbe4 {2}
        └── 📙:5:B
    ");

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Put non-empty A on top of empty B.
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(
            editor,
            "refs/heads/A".try_into()?,
            "refs/heads/B".try_into()?,
        )?;

    rebase.materialize()?;
    set_workspace_metadata(&mut meta, &ws, ws_meta)?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 2c820f0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 09d8e52 (A) A
    | * e1bbad3 (origin/main, main) add X
    |/  
    * 85efbe4 (gitbutler/target, B) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 85efbe4
    └── ≡📙:3:A on 85efbe4 {2}
        ├── 📙:3:A
        │   └── ·09d8e52 (🏘️)
        └── 📙:5:B
    ");

    Ok(())
}

fn stack_display_order(ws: &but_graph::Workspace) -> Vec<String> {
    ws.stacks
        .iter()
        .filter_map(|stack| stack.ref_name())
        .map(|name| name.to_string())
        .collect()
}

fn metadata_stack_order(ws: &but_graph::Workspace) -> Vec<String> {
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
    ws: &but_graph::Workspace,
    ws_meta: Option<but_core::ref_metadata::Workspace>,
) -> anyhow::Result<()> {
    if let Some((ws_meta, ref_name)) = ws_meta.zip(ws.ref_name()) {
        let mut md = meta.workspace(ref_name)?;
        *md = ws_meta;
        md.set_project_meta(ws.graph.project_meta.clone());
        meta.set_workspace(&md)?;
    }
    Ok(())
}

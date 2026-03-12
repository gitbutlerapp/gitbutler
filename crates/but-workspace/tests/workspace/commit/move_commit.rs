use but_rebase::graph_rebase::{GraphExt, mutate::InsertSide};
use but_testsupport::{graph_workspace, visualize_commit_graph_all};

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn move_top_commit_to_top_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
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
    let a_commit = repo.rev_parse_single("A")?.detach();
    let a_commit_selector = editor.select_commit(a_commit)?;
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();
    let c_commit_selector = editor.select_commit(c_commit)?;

    // Put C commit at the top of A
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        c_commit_selector,
        a_commit_selector,
        InsertSide::Above,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize()?;
    let commit_mapping = materialization.history.commit_mappings();
    ws.refresh_from_head(&repo, &meta)?;

    let new_c_commit = commit_mapping.get(&c_commit);
    let tip_of_a_branch = repo.rev_parse_single("A")?.detach();
    let tip_of_c_branch = repo.rev_parse_single("C")?.detach();

    assert_eq!(
        Some(&tip_of_a_branch),
        new_c_commit,
        "The tip of A should be the C commit"
    );

    assert_eq!(
        tip_of_c_branch, b_commit,
        "The tip of C should be the B commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   26fdd46 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * c813d8d (C, B) B
    * | 3db8c14 (A) C
    * | 09d8e52 A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:C on 85efbe4 {2}
    │   ├── 📙:5:C
    │   └── 📙:6:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            ├── ·3db8c14 (🏘️)
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_bottom_commit_to_top_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
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
    let a_commit = repo.rev_parse_single("A")?.detach();
    let a_commit_selector = editor.select_commit(a_commit)?;
    let b_commit = repo.rev_parse_single("B")?.detach();
    let b_commit_selector = editor.select_commit(b_commit)?;
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put B commit at the top of A
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        b_commit_selector,
        a_commit_selector,
        InsertSide::Above,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize()?;
    let commit_mapping = materialization.history.commit_mappings();
    ws.refresh_from_head(&repo, &meta)?;

    let new_b_commit = commit_mapping.get(&b_commit);
    let new_c_commit = commit_mapping.get(&c_commit);
    let tip_of_a_branch = repo.rev_parse_single("A")?.detach();
    let tip_of_c_branch = repo.rev_parse_single("C")?.detach();

    assert_eq!(
        Some(&tip_of_a_branch),
        new_b_commit,
        "The tip of A should be the B commit"
    );

    assert_eq!(
        Some(&tip_of_c_branch),
        new_c_commit,
        "The tip of C should be the the rebased C commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   ac869ab (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f14615 (C) C
    * | 698ccd3 (A) B
    * | 09d8e52 A
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·9f14615 (🏘️)
    │   └── 📙:5:B
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            ├── ·698ccd3 (🏘️)
            └── ·09d8e52 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_top_commit_to_bottom_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
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
    let a_commit = repo.rev_parse_single("A")?.detach();
    let a_commit_selector = editor.select_commit(a_commit)?;
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();
    let c_commit_selector = editor.select_commit(c_commit)?;

    // Put C commit below the A commit
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        c_commit_selector,
        a_commit_selector,
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize()?;
    let commit_mapping = materialization.history.commit_mappings();
    ws.refresh_from_head(&repo, &meta)?;

    let new_a_commit = commit_mapping.get(&a_commit);
    let tip_of_a_branch = repo.rev_parse_single("A")?.detach();
    let tip_of_c_branch = repo.rev_parse_single("C")?.detach();

    assert_eq!(
        Some(&tip_of_a_branch),
        new_a_commit,
        "The tip of A should be the rebased A commit"
    );

    assert_eq!(
        tip_of_c_branch, b_commit,
        "The tip of C should be the B commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   45976fd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * c813d8d (C, B) B
    * | 9bb60db (A) A
    * | 9f14615 C
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:C on 85efbe4 {2}
    │   ├── 📙:5:C
    │   └── 📙:6:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            ├── ·9bb60db (🏘️)
            └── ·9f14615 (🏘️)
    ");

    Ok(())
}

#[test]
fn move_bottom_commit_to_bottom_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
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
    let a_commit = repo.rev_parse_single("A")?.detach();
    let a_commit_selector = editor.select_commit(a_commit)?;
    let b_commit = repo.rev_parse_single("B")?.detach();
    let b_commit_selector = editor.select_commit(b_commit)?;
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put B commit below the A commit
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        b_commit_selector,
        a_commit_selector,
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize()?;
    let commit_mapping = materialization.history.commit_mappings();
    ws.refresh_from_head(&repo, &meta)?;

    let new_a_commit = commit_mapping.get(&a_commit);
    let new_c_commit = commit_mapping.get(&c_commit);
    let tip_of_a_branch = repo.rev_parse_single("A")?.detach();
    let tip_of_c_branch = repo.rev_parse_single("C")?.detach();

    assert_eq!(
        Some(&tip_of_a_branch),
        new_a_commit,
        "The tip of A should be the rebased A commit"
    );

    assert_eq!(
        Some(&tip_of_c_branch),
        new_c_commit,
        "The tip of C should be the the rebased C commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   dbabd50 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f14615 (C) C
    * | 3df48f1 (A) A
    * | c813d8d B
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·9f14615 (🏘️)
    │   └── 📙:5:B
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            ├── ·3df48f1 (🏘️)
            └── ·c813d8d (🏘️)
    ");

    Ok(())
}

#[test]
fn move_single_commit_to_the_top_of_another_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
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
    let a_commit = repo.rev_parse_single("A")?.detach();
    let a_commit_selector = editor.select_commit(a_commit)?;
    let c_commit = repo.rev_parse_single("C")?.detach();
    let c_commit_selector = editor.select_commit(c_commit)?;

    // Put A commit at the top of the branch C
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        a_commit_selector,
        c_commit_selector,
        InsertSide::Above,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize()?;
    let commit_mapping = materialization.history.commit_mappings();
    ws.refresh_from_head(&repo, &meta)?;

    let new_a_commit = commit_mapping.get(&a_commit);
    let tip_of_c_branch = repo.rev_parse_single("C")?.detach();

    assert_eq!(
        Some(&tip_of_c_branch),
        new_a_commit,
        "The tip of C should be the rebased A commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   9b0c3a5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 8dbfefa (C) A
    | * 09bc93e C
    | * c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main, A) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:C on 85efbe4 {2}
    │   ├── 📙:3:C
    │   │   ├── ·8dbfefa (🏘️)
    │   │   └── ·09bc93e (🏘️)
    │   └── 📙:4:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:5:A on 85efbe4 {1}
        └── 📙:5:A
    ");

    Ok(())
}

#[test]
fn move_single_commit_to_the_bottom_of_another_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
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
    let a_commit = repo.rev_parse_single("A")?.detach();
    let a_commit_selector = editor.select_commit(a_commit)?;
    let b_commit = repo.rev_parse_single("B")?.detach();
    let b_commit_selector = editor.select_commit(b_commit)?;
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put A commit below the B commit
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        a_commit_selector,
        b_commit_selector,
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize()?;
    let commit_mapping = materialization.history.commit_mappings();
    ws.refresh_from_head(&repo, &meta)?;

    let new_b_commit = commit_mapping.get(&b_commit);
    let new_c_commit = commit_mapping.get(&c_commit);
    let tip_of_b_branch = repo.rev_parse_single("B")?.detach();
    let tip_of_c_branch = repo.rev_parse_single("C")?.detach();

    assert_eq!(
        Some(&tip_of_b_branch),
        new_b_commit,
        "The tip of B should be the rebased B commit"
    );

    assert_eq!(
        Some(&tip_of_c_branch),
        new_c_commit,
        "The tip of C should be the rebased C commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   202b05f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * f603807 (C) C
    | * 698ccd3 (B) B
    | * 09d8e52 A
    |/  
    * 85efbe4 (origin/main, main, A) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:C on 85efbe4 {2}
    │   ├── 📙:3:C
    │   │   └── ·f603807 (🏘️)
    │   └── 📙:4:B
    │       ├── ·698ccd3 (🏘️)
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:5:A on 85efbe4 {1}
        └── 📙:5:A
    ");

    Ok(())
}

#[test]
fn move_commit_to_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &["B"]);
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
    let a_commit = repo.rev_parse_single("A")?.detach();
    let a_commit_selector = editor.select_commit(a_commit)?;
    let b_ref_name = "refs/heads/B".try_into()?;
    let b_ref_selector = editor.select_reference(b_ref_name)?;

    // Put A commit in branch B
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        a_commit_selector,
        b_ref_selector,
        InsertSide::Below,
    )?;

    // Materialize the operation
    rebase.materialize()?;
    ws.refresh_from_head(&repo, &meta)?;

    let tip_of_b_branch = repo.rev_parse_single("B")?.detach();

    assert_eq!(
        tip_of_b_branch, a_commit,
        "The tip of B should be the rebased A commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (B) A
    |/  
    * 85efbe4 (origin/main, main, A) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:B on 85efbe4 {2}
    │   └── 📙:3:B
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:4:A on 85efbe4 {1}
        └── 📙:4:A
    ");

    Ok(())
}

#[test]
fn move_commit_in_non_managed_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:three[🌳] <> ✓!
    └── ≡:0:three[🌳] {1}
        ├── :0:three[🌳]
        │   └── ·c9f444c
        ├── :1:two <> origin/two →:2:
        │   └── ❄️16fd221
        └── :3:one
            └── ❄8b426d0
    ");

    let editor = ws.graph.to_editor(&repo)?;
    let three_commit = repo.rev_parse_single("three")?.detach();
    let three_commit_selector = editor.select_commit(three_commit)?;
    let two_ref_name = "refs/heads/two".try_into()?;
    let two_ref_selector = editor.select_reference(two_ref_name)?;

    // Put commit three at the top of branch two
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        three_commit_selector,
        two_ref_selector,
        InsertSide::Below,
    )?;

    // Materialize the operation
    rebase.materialize()?;
    ws.refresh_from_head(&repo, &meta)?;

    let tip_of_three_branch = repo.rev_parse_single("three")?.detach();
    let tip_of_two_branch = repo.rev_parse_single("two")?.detach();

    assert_eq!(
        tip_of_three_branch, three_commit,
        "The tip of 'three' should be the three commit"
    );

    assert_eq!(
        tip_of_two_branch, three_commit,
        "The tip of 'two' should be the three commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three, two) commit three
    * 16fd221 (origin/two) commit two
    * 8b426d0 (one) commit one
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:three[🌳] <> ✓!
    └── ≡:0:three[🌳] {1}
        ├── :0:three[🌳]
        │   ├── ·c9f444c ►two
        │   └── ·16fd221
        └── :3:one
            └── ·8b426d0
    ");

    Ok(())
}

#[test]
fn reorder_commit_in_non_managed_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, meta, _description) =
        named_writable_scenario_with_description_and_graph("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:three[🌳] <> ✓!
    └── ≡:0:three[🌳] {1}
        ├── :0:three[🌳]
        │   └── ·c9f444c
        ├── :1:two <> origin/two →:2:
        │   └── ❄️16fd221
        └── :3:one
            └── ❄8b426d0
    ");

    let editor = ws.graph.to_editor(&repo)?;
    let three_commit = repo.rev_parse_single("three")?.detach();
    let three_commit_selector = editor.select_commit(three_commit)?;
    let two_commit = repo.rev_parse_single("two")?.detach();
    let two_commit_selector = editor.select_commit(two_commit)?;

    // Put commit three below commit two
    let rebase = but_workspace::commit::move_commit(
        editor,
        &ws,
        three_commit_selector,
        two_commit_selector,
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize()?;
    let commit_mappings = materialization.history.commit_mappings();
    ws.refresh_from_head(&repo, &meta)?;

    let new_commit_two = commit_mappings.get(&two_commit);
    let tip_of_three_branch = repo.rev_parse_single("three")?.detach();
    let tip_of_two_branch = repo.rev_parse_single("two")?.detach();

    assert_eq!(
        Some(&tip_of_three_branch),
        new_commit_two,
        "The tip of 'three' should be the rebased two commit"
    );

    assert_eq!(
        Some(&tip_of_two_branch),
        new_commit_two,
        "The tip of 'two' should be the rebased two commit"
    );

    // Branches 'three' and 'two' now point to the updated 'two' commit,
    // which is now a child of three.
    // The origin two branch has not been yet updated and still points to the original 'two' commit.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * b8c85c3 (HEAD -> three, two) commit two
    * fc5e5e6 commit three
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:three[🌳] <> ✓!
    └── ≡:0:three[🌳] {1}
        ├── :0:three[🌳]
        │   ├── ·b8c85c3 ►two
        │   └── ·fc5e5e6
        └── :2:one
            └── ·8b426d0
    ");

    Ok(())
}

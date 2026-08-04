use bstr::ByteSlice;
use but_rebase::graph_rebase::{
    Editor,
    mutate::{InsertSide, RelativeTo},
};
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use snapbox::IntoData;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

fn parent_subjects(repo: &gix::Repository, rev: &str) -> anyhow::Result<Vec<String>> {
    let commit = repo.find_commit(repo.rev_parse_single(rev)?.detach())?;
    commit
        .parent_ids()
        .map(|parent_id| {
            let parent = repo.find_commit(parent_id.detach())?;
            Ok(parent.message_raw()?.trim_end().to_str_lossy().to_string())
        })
        .collect()
}

#[test]
fn move_top_commit_to_top_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put C commit at the top of A
    let rebase = but_workspace::commit::move_commits(
        editor,
        [c_commit],
        RelativeTo::Commit(a_commit),
        InsertSide::Above,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize(Default::default())?;
    let commit_mapping = materialization.history.commit_mappings();
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   0ffeac6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f2cc60d (A) C
| * 09d8e52 A
* | c813d8d (C, B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       ├── ·f2cc60d (🏘️)
│       └── ·09d8e52 (🏘️)
└── ≡📙:5:C on 85efbe4 {2}
    ├── 📙:5:C
    └── 📙:6:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_bottom_commit_to_top_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put B commit at the top of A
    let rebase = but_workspace::commit::move_commits(
        editor,
        [b_commit],
        RelativeTo::Commit(a_commit),
        InsertSide::Above,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize(Default::default())?;
    let commit_mapping = materialization.history.commit_mappings();
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   9c6a201 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * f9061ed (A) B
| * 09d8e52 A
* | 8e00332 (C) C
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       ├── ·f9061ed (🏘️)
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·8e00332 (🏘️)
    └── 📙:5:B

"#]]
    );

    Ok(())
}

#[test]
fn move_top_commit_to_bottom_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put C commit below the A commit
    let rebase = but_workspace::commit::move_commits(
        editor,
        [c_commit],
        RelativeTo::Commit(a_commit),
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize(Default::default())?;
    let commit_mapping = materialization.history.commit_mappings();
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   b3f0cfc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 2506923 (A) A
| * 8e00332 C
* | c813d8d (C, B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       ├── ·2506923 (🏘️)
│       └── ·8e00332 (🏘️)
└── ≡📙:5:C on 85efbe4 {2}
    ├── 📙:5:C
    └── 📙:6:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_bottom_commit_to_bottom_of_another_stack() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put B commit below the A commit
    let rebase = but_workspace::commit::move_commits(
        editor,
        [b_commit],
        RelativeTo::Commit(a_commit),
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize(Default::default())?;
    let commit_mapping = materialization.history.commit_mappings();
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   2410103 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 4dfe841 (A) A
| * c813d8d B
* | 8e00332 (C) C
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       ├── ·4dfe841 (🏘️)
│       └── ·c813d8d (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·8e00332 (🏘️)
    └── 📙:5:B

"#]]
    );

    Ok(())
}

#[test]
fn move_single_commit_to_the_top_of_another_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let a_commit = repo.rev_parse_single("A")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put A commit at the top of the branch C
    let rebase = but_workspace::commit::move_commits(
        editor,
        [a_commit],
        RelativeTo::Commit(c_commit),
        InsertSide::Above,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize(Default::default())?;
    let commit_mapping = materialization.history.commit_mappings();
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    let new_a_commit = commit_mapping.get(&a_commit);
    let tip_of_c_branch = repo.rev_parse_single("C")?.detach();

    assert_eq!(
        Some(&tip_of_c_branch),
        new_a_commit,
        "The tip of C should be the rebased A commit"
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   3f51cff (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | 148f8f3 (C) A
* | 09bc93e C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main, A) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:5:A on 85efbe4 {1}
│   └── 📙:5:A
└── ≡📙:3:C on 85efbe4 {2}
    ├── 📙:3:C
    │   ├── ·148f8f3 (🏘️)
    │   └── ·09bc93e (🏘️)
    └── 📙:4:B
        └── ·c813d8d (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_single_commit_to_the_bottom_of_another_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "ws-ref-ws-commit-single-stack-double-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
            },
        )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f3e1bf2 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
* | 09bc93e (C) C
* | c813d8d (B) B
|/  
* 85efbe4 (origin/main, main) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:C on 85efbe4 {2}
    ├── 📙:4:C
    │   └── ·09bc93e (🏘️)
    └── 📙:5:B
        └── ·c813d8d (🏘️)

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();

    // Put A commit below the B commit
    let rebase = but_workspace::commit::move_commits(
        editor,
        [a_commit],
        RelativeTo::Commit(b_commit),
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize(Default::default())?;
    let commit_mapping = materialization.history.commit_mappings();
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   61c8521 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | ad476a8 (C) C
* | f9061ed (B) B
* | 09d8e52 A
|/  
* 85efbe4 (origin/main, main, A) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:5:A on 85efbe4 {1}
│   └── 📙:5:A
└── ≡📙:3:C on 85efbe4 {2}
    ├── 📙:3:C
    │   └── ·ad476a8 (🏘️)
    └── 📙:4:B
        ├── ·f9061ed (🏘️)
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_commit_to_empty_branch() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("ws-with-empty-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &["B"]);
        })?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   6d5c23e (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 09d8e52 (A) A
|/  
* 85efbe4 (origin/main, main, B) M

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:3:A on 85efbe4 {1}
│   └── 📙:3:A
│       └── ·09d8e52 (🏘️)
└── ≡📙:4:B on 85efbe4 {2}
    └── 📙:4:B

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let a_commit = repo.rev_parse_single("A")?.detach();

    // Put A commit in branch B
    let rebase = but_workspace::commit::move_commits(
        editor,
        [a_commit],
        RelativeTo::Reference("refs/heads/B".try_into()?),
        InsertSide::Below,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    let tip_of_b_branch = repo.rev_parse_single("B")?.detach();

    assert_eq!(
        tip_of_b_branch, a_commit,
        "The tip of B should be the rebased A commit"
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   e16ce30 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | 09d8e52 (B) A
|/  
* 85efbe4 (origin/main, main, A) M

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
├── ≡📙:4:A on 85efbe4 {1}
│   └── 📙:4:A
└── ≡📙:3:B on 85efbe4 {2}
    └── 📙:3:B
        └── ·09d8e52 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn move_commit_in_non_managed_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("reword-three-commits", |_| {})?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:three[🌳] <> ✓!
└── ≡:0:three[🌳] {1}
    ├── :0:three[🌳]
    │   └── ·c9f444c
    ├── :1:two <> origin/two →:2:
    │   └── ❄️16fd221
    └── :3:one
        └── ❄8b426d0

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let three_commit = repo.rev_parse_single("three")?.detach();

    // Put commit three at the top of branch two
    let rebase = but_workspace::commit::move_commits(
        editor,
        [three_commit],
        RelativeTo::Reference("refs/heads/two".try_into()?),
        InsertSide::Below,
    )?;

    // Materialize the operation
    rebase.materialize(Default::default())?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three, two) commit three
* 16fd221 (origin/two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:three[🌳] <> ✓!
└── ≡:0:three[🌳] {1}
    ├── :0:three[🌳]
    │   ├── ·c9f444c ►two
    │   └── ·16fd221
    └── :3:one
        └── ·8b426d0

"#]]
    );

    Ok(())
}

#[test]
fn reorder_merge_commit_above_keeps_child_commits_visible() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("gb-1525-reorder-merge-commit", |_| {})?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 32c8bda (HEAD -> child-stack, C2) C2: add other.txt
* 64dace5 (C1) C1: add child-1.txt
*   197bdf1 (M) M: merge feature-parent
|\  
| * b54108c (feature-parent) update parent.txt (2)
| * 1b1a64f update parent.txt (1)
| * 40bcd70 add parent.txt
* | aa67ae0 (origin/main, main-advanced, main) update main.txt (1)
|/  
* 7674a5e (tag: base) base

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:child-stack[🌳] <> ✓refs/remotes/origin/main on aa67ae0
└── ≡:0:child-stack[🌳] on aa67ae0 {1}
    ├── :0:child-stack[🌳]
    │   └── ·32c8bda ►C2
    ├── :3:C1
    │   └── ·64dace5
    └── :4:M
        └── ·197bdf1

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let merge_commit = repo.rev_parse_single("M")?.detach();
    let c1_commit = repo.rev_parse_single("C1")?.detach();

    let rebase = but_workspace::commit::move_commits(
        editor,
        [merge_commit],
        RelativeTo::Commit(c1_commit),
        InsertSide::Above,
    )?;

    rebase.materialize(Default::default())?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    let post_move_graph = visualize_commit_graph_all(&repo)?;
    assert_eq!(
        parent_subjects(&repo, "C1")?,
        vec![
            "C1: add child-1.txt".to_string(),
            "update parent.txt (2)".to_string()
        ],
        "moving the merge commit above C1 should preserve the visible first-parent lane"
    );
    snapbox::assert_data_eq!(
        post_move_graph,
        snapbox::str![[r#"
* 1fa67f9 (HEAD -> child-stack, C2) C2: add other.txt
*   88f8bb5 (C1) M: merge feature-parent
|\  
| * b54108c (feature-parent) update parent.txt (2)
| * 1b1a64f update parent.txt (1)
| * 40bcd70 add parent.txt
* | 40eca7d C1: add child-1.txt
* | aa67ae0 (origin/main, main-advanced, main, M) update main.txt (1)
|/  
* 7674a5e (tag: base) base

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn reorder_merge_commit_below_keeps_child_commits_visible() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("gb-1525-reorder-merge-commit", |_| {})?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 32c8bda (HEAD -> child-stack, C2) C2: add other.txt
* 64dace5 (C1) C1: add child-1.txt
*   197bdf1 (M) M: merge feature-parent
|\  
| * b54108c (feature-parent) update parent.txt (2)
| * 1b1a64f update parent.txt (1)
| * 40bcd70 add parent.txt
* | aa67ae0 (origin/main, main-advanced, main) update main.txt (1)
|/  
* 7674a5e (tag: base) base

"#]]
        .raw()
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:child-stack[🌳] <> ✓refs/remotes/origin/main on aa67ae0
└── ≡:0:child-stack[🌳] on aa67ae0 {1}
    ├── :0:child-stack[🌳]
    │   └── ·32c8bda ►C2
    ├── :3:C1
    │   └── ·64dace5
    └── :4:M
        └── ·197bdf1

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let merge_commit = repo.rev_parse_single("M")?.detach();
    let main_commit = repo.rev_parse_single("main")?.detach();

    let rebase = but_workspace::commit::move_commits(
        editor,
        [merge_commit],
        RelativeTo::Commit(main_commit),
        InsertSide::Below,
    )?;

    rebase.materialize(Default::default())?;
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

    let post_move_graph = visualize_commit_graph_all(&repo)?;
    assert_eq!(
        parent_subjects(&repo, "HEAD~3")?,
        vec!["base".to_string(), "update parent.txt (2)".to_string()],
        "moving the merge commit below main should make main's first parent the merge commit's first parent"
    );
    snapbox::assert_data_eq!(
        post_move_graph,
        snapbox::str![[r#"
* 3cf4ba4 (HEAD -> child-stack, C2) C2: add other.txt
* 7673ad4 (C1) C1: add child-1.txt
* 8a192c0 (main-advanced, main, M) update main.txt (1)
*   ed12786 M: merge feature-parent
|\  
| * b54108c (feature-parent) update parent.txt (2)
| * 1b1a64f update parent.txt (1)
| * 40bcd70 add parent.txt
|/  
| * aa67ae0 (origin/main) update main.txt (1)
|/  
* 7674a5e (tag: base) base

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn reorder_commit_in_non_managed_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("reword-three-commits", |_| {})?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:three[🌳] <> ✓!
└── ≡:0:three[🌳] {1}
    ├── :0:three[🌳]
    │   └── ·c9f444c
    ├── :1:two <> origin/two →:2:
    │   └── ❄️16fd221
    └── :3:one
        └── ❄8b426d0

"#]]
    );

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let three_commit = repo.rev_parse_single("three")?.detach();
    let two_commit = repo.rev_parse_single("two")?.detach();

    // Put commit three below commit two
    let rebase = but_workspace::commit::move_commits(
        editor,
        [three_commit],
        RelativeTo::Commit(two_commit),
        InsertSide::Below,
    )?;

    // Materialize the operation
    let materialization = rebase.materialize(Default::default())?;
    let commit_mappings = materialization.history.commit_mappings();
    let project_meta = ws.graph.project_meta.clone();
    ws.refresh_from_head(&repo, &meta, project_meta)?;

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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 09ad3ca (HEAD -> three, two) commit two
* 0c38dd9 commit three
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:three[🌳] <> ✓!
└── ≡:0:three[🌳] {1}
    ├── :0:three[🌳]
    │   ├── ·09ad3ca ►two
    │   └── ·0c38dd9
    └── :2:one
        └── ·8b426d0

"#]]
    );

    Ok(())
}
#[test]
fn move_mixed_main_and_worktree_commits_to_another_worktree() -> anyhow::Result<()> {
    use but_graph::Graph;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_testsupport::git_status_at_dir;

    let (repo, _tmp) = crate::utils::writable_scenario_slow("worktree-move-mixed");
    let mut meta = std::mem::ManuallyDrop::new(VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?);
    let mut options = but_graph::init::Options::limited();
    for (name, branch) in [("wt", "feat"), ("other", "other")] {
        options.worktree_tips.push(but_graph::init::WorktreeTip {
            name: name.into(),
            ref_name: Some(format!("refs/heads/{branch}").try_into()?),
            id: repo.find_reference(branch)?.peel_to_id()?.detach(),
        });
    }
    let graph = Graph::from_head(&repo, &*meta, Default::default(), options)?.validated()?;

    // `main` stacks two commits above `stack-base`; `feat` is a linked worktree with one
    // commit; `other`, `stable` and `target` are all co-located at the base commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 57d0038 (feat) worktree source
| * baa5a4c (HEAD -> main) workspace source
| * 4119f49 (stack-base) stack base
|/  
* 35b8235 (target, stable, other) base

"#]]
        .raw()
    );

    let main = repo.rev_parse_single("main")?.detach();
    let feat = repo.rev_parse_single("feat")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    but_workspace::commit::move_commits(
        editor,
        [main, feat],
        RelativeTo::Reference("refs/heads/other".try_into()?),
        InsertSide::Below,
    )?
    .materialize(Default::default())?;

    // Both moved commits land on `other` and `main` falls back onto `stack-base`. The
    // placeholder left where `feat`'s commit sat keeps `feat` directly above `other`, so
    // it resolves through the placeholder onto the moved commit; `stable` and `target`
    // sit below the insertion point and stay on the base commit.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 0529812 (HEAD -> main, stack-base) stack base
* 9a81bea (other, feat) worktree source
* 6f6bfe8 workspace source
* 35b8235 (target, stable) base

"#]]
        .raw()
    );

    let workdir = repo.workdir().expect("non-bare repo");
    assert_eq!(
        git_status_at_dir(workdir.join("wt"))?,
        "",
        "the source linked checkout follows its moved ref"
    );
    assert_eq!(
        git_status_at_dir(workdir.join("other"))?,
        "",
        "the destination linked checkout follows its rewritten ref"
    );
    assert!(
        workdir.join("wt/wt-file").exists(),
        "`feat` tracks its commit to the destination, so the source linked checkout has it too"
    );
    assert_eq!(
        std::fs::read_to_string(workdir.join("other/ws-file"))?,
        "workspace\n",
        "the destination checkout contains the main source"
    );
    assert_eq!(
        std::fs::read_to_string(workdir.join("other/wt-file"))?,
        "worktree\n",
        "the destination checkout contains the worktree source"
    );
    Ok(())
}

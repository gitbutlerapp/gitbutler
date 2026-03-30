use anyhow::Result;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use but_workspace::commit::discard_commit;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario_with_description_and_graph,
};

#[test]
fn discard_middle_commit_in_non_managed_workspace() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let one = repo.rev_parse_single("one")?;
    let two = repo.rev_parse_single("two")?;
    let three = repo.rev_parse_single("three")?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = discard_commit(editor, two)?;

    outcome.materialize()?;

    let tip_of_two = repo.rev_parse_single("two")?;
    assert_eq!(tip_of_two, one, "The tip of two should now point to one");

    let tip_of_three = repo.rev_parse_single("three")?;
    assert_ne!(tip_of_three, three, "three should have been rewritten");

    let rewritten_three = repo.find_commit(tip_of_three)?;
    let parent_ids: Vec<_> = rewritten_three.parent_ids().collect();
    assert_eq!(parent_ids, vec![one], "three should now have one as parent");

    assert!(
        repo.rev_parse_single(format!("{tip_of_three}:one.txt").as_str())
            .is_ok()
    );
    assert!(
        repo.rev_parse_single(format!("{tip_of_three}:three.txt").as_str())
            .is_ok()
    );
    assert!(
        repo.rev_parse_single(format!("{tip_of_three}:two.txt").as_str())
            .is_err(),
        "discarding two should remove its introduced changes from descendants"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * fc5e5e6 (HEAD -> three) commit three
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (two, one) commit one
    ");

    Ok(())
}

#[test]
fn discard_tip_commit_in_workspace_stack() -> Result<()> {
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

    let b = repo.rev_parse_single("B")?;
    let c = repo.rev_parse_single("C")?;

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
    let outcome = discard_commit(editor, c)?;

    let outcome = outcome.materialize()?;
    insta::assert_snapshot!(graph_workspace(outcome.workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:5:C on 85efbe4 {2}
    │   ├── 📙:5:C
    │   └── 📙:6:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let tip_of_c = repo.rev_parse_single("C")?;
    assert_eq!(tip_of_c, b, "The C ref should now point to B");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   5e34c6d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * c813d8d (C, B) B
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    Ok(())
}

#[test]
fn discard_bottom_commit_in_workspace_stack() -> Result<()> {
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

    let b = repo.rev_parse_single("B")?;
    let c = repo.rev_parse_single("C")?;
    let main = repo.rev_parse_single("main")?;

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
    let outcome = discard_commit(editor, b)?;

    let outcome = outcome.materialize()?;
    insta::assert_snapshot!(graph_workspace(outcome.workspace), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:C on 85efbe4 {2}
    │   ├── 📙:4:C
    │   │   └── ·9f14615 (🏘️)
    │   └── 📙:5:B
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let tip_of_b = repo.rev_parse_single("B")?;
    assert_eq!(tip_of_b, main, "The B ref should now point to main");

    let tip_of_c = repo.rev_parse_single("C")?;
    assert_ne!(tip_of_c, c, "The C commit should have been rewritten");
    let rewritten_c = repo.find_commit(tip_of_c)?;
    let parent_ids: Vec<_> = rewritten_c.parent_ids().collect();
    assert_eq!(
        parent_ids,
        vec![main],
        "C should now directly descend from main"
    );

    assert_ne!(b, tip_of_c, "Discarded commit must not remain as C tip");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   ec062b0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 9f14615 (C) C
    * | 09d8e52 (A) A
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");

    Ok(())
}

#[test]
fn cannot_discard_conflicted_commit() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph("with-conflict", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 8450331 (HEAD -> main, tag: conflicted) GitButler WIP Commit
    * a047f81 (tag: normal) init
    ");

    let conflicted = repo.rev_parse_single("conflicted")?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let result = discard_commit(editor, conflicted);

    let err = result.expect_err("Discarding a conflicted commit must fail");
    assert!(
        err.to_string()
            .contains("Cannot discard a conflicted commit"),
        "unexpected error: {err}"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 8450331 (HEAD -> main, tag: conflicted) GitButler WIP Commit
    * a047f81 (tag: normal) init
    ");

    Ok(())
}

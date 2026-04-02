use anyhow::Result;
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use but_testsupport::visualize_commit_graph_all;
use but_workspace::ordering::order_commit_selectors_by_parentage;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments,
    named_writable_scenario_with_description_and_graph as writable_scenario,
};

fn selector_ids_in_order<M: but_core::RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selectors: &[but_rebase::graph_rebase::Selector],
) -> Result<Vec<gix::ObjectId>> {
    selectors
        .iter()
        .map(|selector| editor.lookup_pick(*selector))
        .collect()
}

#[test]
fn linear_chain_is_ordered_parent_to_child() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let one = repo.rev_parse_single("one")?.detach();
    let two = repo.rev_parse_single("two")?.detach();
    let three = repo.rev_parse_single("three")?.detach();

    let ordered = order_commit_selectors_by_parentage(&editor, [three, one, two])?;
    let ordered_ids = selector_ids_in_order(&editor, &ordered)?;
    assert_eq!(ordered_ids, vec![one, two, three]);
    Ok(())
}

#[test]
fn disjoint_commits_use_workspace_traversal_order() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-two-stacks", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let b = repo.rev_parse_single("B")?.detach();

    let first = order_commit_selectors_by_parentage(&editor, [a, b])?;
    let second = order_commit_selectors_by_parentage(&editor, [b, a])?;

    assert_eq!(
        selector_ids_in_order(&editor, &first)?,
        selector_ids_in_order(&editor, &second)?
    );
    Ok(())
}

#[test]
fn mixed_related_and_disjoint_commits_keep_parentage_constraints() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-single-stack-double-stack", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
        })?;

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
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let b = repo.rev_parse_single("B")?.detach();
    let c = repo.rev_parse_single("C")?.detach();

    let ordered = order_commit_selectors_by_parentage(&editor, [c, a, b])?;
    let ids = selector_ids_in_order(&editor, &ordered)?;

    let pos_b = ids.iter().position(|id| *id == b).expect("B selected");
    let pos_c = ids.iter().position(|id| *id == c).expect("C selected");
    assert!(
        pos_b < pos_c,
        "parent commit B must be before child commit C"
    );
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&a));
    Ok(())
}

#[test]
fn duplicate_selectors_are_deduplicated_by_first_occurrence() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let one = repo.rev_parse_single("one")?.detach();
    let two = repo.rev_parse_single("two")?.detach();

    let ordered = order_commit_selectors_by_parentage(&editor, [two, one, two, one])?;
    let ids = selector_ids_in_order(&editor, &ordered)?;
    assert_eq!(ids, vec![one, two]);
    Ok(())
}

#[test]
fn single_selector_is_unchanged() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;

    let two = repo.rev_parse_single("two")?.detach();
    let ordered = order_commit_selectors_by_parentage(&editor, [two])?;
    let ids = selector_ids_in_order(&editor, &ordered)?;
    assert_eq!(ids, vec![two]);
    Ok(())
}

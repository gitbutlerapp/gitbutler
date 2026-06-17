use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, LookupStep, Step, mutate, testing::Testing as _};
use but_testsupport::visualize_commit_graph_all;

use crate::utils::{fixture, fixture_writable, standard_options};

fn short_ids(
    editor: &Editor<'_, '_, impl but_core::RefMetadata>,
    selectors: &[but_rebase::graph_rebase::Selector],
) -> Result<Vec<String>> {
    selectors
        .iter()
        .map(|selector| {
            let id = editor.lookup_pick(*selector)?;
            Ok(id.to_hex_with_len(7).to_string())
        })
        .collect()
}

fn short_id(id: gix::ObjectId) -> String {
    id.to_hex_with_len(7).to_string()
}

fn trim_trailing_whitespace(input: &str) -> String {
    input
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn handles_zero_nodes() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @"
    ◎  refs/heads/main
    ●  120e3a9 c
    ●  a96434e b
    ●  d591dfe a
    ●  35b8235 base
    ");

    let ordered = editor.order_commit_selectors_by_parentage(Vec::<gix::ObjectId>::new())?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    insta::assert_debug_snapshot!(ordered_ids, @"[]");

    Ok(())
}

#[test]
fn handles_one_node() -> Result<()> {
    let (repo, mut meta) = fixture("single-commit")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @"
    ◎  refs/heads/main
    ●  35b8235 base
    ");

    let base = repo.head_id()?.detach();
    let ordered = editor.order_commit_selectors_by_parentage([base])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "35b8235",
    ]
    "#);

    Ok(())
}

#[test]
fn orders_linear_commits_parent_first_for_n_nodes() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let base = repo.rev_parse_single("HEAD~3")?.detach();
    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let b = repo.rev_parse_single("HEAD~1")?.detach();
    let c = repo.rev_parse_single("HEAD")?.detach();

    let ordered = editor.order_commit_selectors_by_parentage([c, a, b, base])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "35b8235",
        "d591dfe",
        "a96434e",
        "120e3a9",
    ]
    "#);

    Ok(())
}

#[test]
fn orders_disjoint_commits_by_editor_graph_traversal_1() -> Result<()> {
    let (repo, mut meta) = fixture("three-branches-merged")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let graph = trim_trailing_whitespace(&visualize_commit_graph_all(&repo)?);
    insta::assert_snapshot!(graph, @r"
    *-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/
    * / add59d2 (A) A: 10 lines on top
    |/
    * 8f0d338 (tag: base) base
    ");

    let a = repo.rev_parse_single("A")?.detach();
    let b = repo.rev_parse_single("B")?.detach();

    let ordered = editor.order_commit_selectors_by_parentage([b, a])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "a748762",
        "add59d2",
    ]
    "#);

    Ok(())
}

#[test]
fn orders_disjoint_commits_by_editor_graph_traversal_2() -> Result<()> {
    let (repo, mut meta) = fixture("three-branches-merged")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let graph = trim_trailing_whitespace(&visualize_commit_graph_all(&repo)?);
    insta::assert_snapshot!(graph, @r"
    *-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/
    * / add59d2 (A) A: 10 lines on top
    |/
    * 8f0d338 (tag: base) base
    ");

    // The tip of A
    let a = repo.rev_parse_single("A")?.detach();
    // The tip of B
    let b = repo.rev_parse_single("B")?.detach();
    // The first parent of the tip of B
    let b1 = repo.rev_parse_single("B~")?.detach();

    let ordered = editor.order_commit_selectors_by_parentage([b, a, b1])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    // The order should be:
    // 1. The first parent of B's tip
    // 2. The tip of B
    // 3. The tip of A
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "62e05ba",
        "a748762",
        "add59d2",
    ]
    "#);

    Ok(())
}

#[test]
fn orders_disjoint_commits_by_editor_graph_traversal_3() -> Result<()> {
    let (repo, mut meta) = fixture("three-branches-merged")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let graph = trim_trailing_whitespace(&visualize_commit_graph_all(&repo)?);
    insta::assert_snapshot!(graph, @r"
    *-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
    |\ \
    | | * 930563a (C) C: add another 10 lines to new file
    | | * 68a2fc3 C: add 10 lines to new file
    | | * 984fd1c C: new file with 10 lines
    | * | a748762 (B) B: another 10 lines at the bottom
    | * | 62e05ba B: 10 lines at the bottom
    | |/
    * / add59d2 (A) A: 10 lines on top
    |/
    * 8f0d338 (tag: base) base
    ");

    // The tip of A
    let a = repo.rev_parse_single("A")?.detach();
    // The tip of B
    let b = repo.rev_parse_single("B")?.detach();
    // The first parent of the tip of C
    let c1 = repo.rev_parse_single("C~")?.detach();
    // The second-level parent of the tip of C
    let c2 = repo.rev_parse_single("C~2")?.detach();

    let ordered = editor.order_commit_selectors_by_parentage([b, c1, a, c2])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    // The order should be:
    // 1. The second-level parent of the tip of C
    // 2. The first-level parent of the tip of C
    // 3. The tip of B
    // 4. The tip of A
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "984fd1c",
        "68a2fc3",
        "a748762",
        "add59d2",
    ]
    "#);

    Ok(())
}

#[test]
fn errors_when_selected_commit_is_absent_from_editor_graph() -> Result<()> {
    let (repo, mut meta) = fixture("disjoint-orphan-branches")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 589e8c3 (HEAD -> main) main: tip
    * 14f3d44 main: base
    * 74debb1 (orphan) orphan: tip
    * c7488cd orphan: base
    ");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    insta::assert_snapshot!(editor.steps_ascii(), @"
    ◎  refs/heads/main
    ●  589e8c3 main: tip
    ●  14f3d44 main: base
    ");

    let orphan = repo.rev_parse_single("orphan")?.detach();

    let error = editor
        .order_commit_selectors_by_parentage([orphan])
        .expect_err("commits absent from editor graph should fail selection");

    let message = error.to_string();
    assert!(
        message.starts_with("Failed to find commit "),
        "unexpected error message format: {message}"
    );
    assert!(
        message.ends_with(" in rebase editor"),
        "unexpected error message format: {message}"
    );
    insta::assert_snapshot!(
        "Failed to find commit <oid> in rebase editor",
        @"Failed to find commit <oid> in rebase editor"
    );

    Ok(())
}

#[test]
fn deduplicates_duplicate_selectors_by_commit_id() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let b = repo.rev_parse_single("HEAD~1")?.detach();
    let c = repo.rev_parse_single("HEAD")?.detach();

    let ordered = editor.order_commit_selectors_by_parentage([b, c, b, a, c])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "d591dfe",
        "a96434e",
        "120e3a9",
    ]
    "#);

    Ok(())
}

#[test]
fn orders_commit_present_in_editor_graph_even_if_workspace_projection_stale() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a = repo.rev_parse_single("HEAD~2")?.detach();
    let a_obj = repo.find_commit(a)?;
    let mut a_commit = a_obj.decode()?;
    a_commit.message = "A rewritten outside workspace traversal".into();
    let rewritten_a = repo.write_object(a_commit)?.detach();

    let a_selector = editor.select_commit(a)?;
    editor.replace(a_selector, Step::new_pick(rewritten_a))?;

    let ordered = editor.order_commit_selectors_by_parentage([rewritten_a])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "1787cd0",
    ]
    "#);

    Ok(())
}

#[test]
fn orders_commit_disconnected_from_checkout_roots_if_still_in_editor_graph() -> Result<()> {
    let (repo, mut meta) = fixture("four-commits")?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let b = repo.rev_parse_single("HEAD~1")?.detach();
    let b_selector = editor.select_commit(b)?;

    editor.disconnect_segment_from(
        mutate::SegmentDelimiter {
            child: b_selector,
            parent: b_selector,
        },
        mutate::SelectorSet::All,
        mutate::SelectorSet::All,
        true,
    )?;

    let ordered = editor.order_commit_selectors_by_parentage([b])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    insta::assert_debug_snapshot!(ordered_ids, @r#"
    [
        "a96434e",
    ]
    "#);

    Ok(())
}

#[test]
fn orders_all_commits_in_y_shaped_two_branch_fixture() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("two-branches-shared-bottom-two")?;

    let graph = trim_trailing_whitespace(&visualize_commit_graph_all(&repo)?);
    insta::assert_snapshot!(graph, @r"
    *   3127e18 (HEAD -> main) merge right into main
    |\
    | * ce0d74d (right) right: head
    * | c3a0d4c (left) left: head
    |/
    * 67a0a68 shared
    * 35b8235 base
    ");
    let right_ref: gix::refs::FullName = "refs/heads/right".try_into()?;

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let merge = repo.rev_parse_single("HEAD")?.detach();
    let left = repo.rev_parse_single("left")?.detach();
    let shared = repo.rev_parse_single("right~")?.detach();
    let base = repo.rev_parse_single("right~2")?.detach();

    let right = repo.rev_parse_single("right")?.detach();
    let right_ref_selector = editor.select_reference(right_ref.as_ref())?;
    let right_selector = editor.select_commit(right)?;

    let ordered = editor.order_commit_selectors_by_parentage([merge, right, left, shared, base])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    assert_eq!(
        ordered_ids,
        vec![
            short_id(base),
            short_id(shared),
            short_id(right),
            short_id(left),
            short_id(merge),
        ]
    );

    // Disconnect the 'right' branch from the merge commit, making it a leaf node, but keeping it in the editor
    // graph.
    editor.disconnect_segment_from(
        mutate::SegmentDelimiter {
            child: right_ref_selector,
            parent: right_selector,
        },
        mutate::SelectorSet::All,
        mutate::SelectorSet::None,
        true,
    )?;

    // The right reference should still exist, but its tip commit should no longer have commit children.
    assert_eq!(editor.lookup_reference(right_ref_selector)?, right_ref);
    let right_children = editor.direct_children(right_ref_selector)?;
    assert!(
        right_children.is_empty(),
        "right should be a leaf commit after disconnecting its children"
    );

    let right = repo.rev_parse_single("right")?.detach();
    let merge = repo.rev_parse_single("HEAD")?.detach();
    let left = repo.rev_parse_single("left")?.detach();
    let shared = repo.rev_parse_single("right~")?.detach();
    let base = repo.rev_parse_single("right~2")?.detach();

    let ordered = editor.order_commit_selectors_by_parentage([merge, right, left, shared, base])?;
    let ordered_ids = short_ids(&editor, &ordered)?;
    assert_eq!(
        ordered_ids,
        vec![
            short_id(base),
            short_id(shared),
            short_id(left),
            short_id(merge),
            short_id(right),
        ]
    );

    let leaf_ordered = editor.order_commit_selectors_by_parentage([merge, right])?;
    let leaf_ordered_ids = short_ids(&editor, &leaf_ordered)?;
    assert_eq!(leaf_ordered_ids, vec![short_id(merge), short_id(right)]);

    Ok(())
}

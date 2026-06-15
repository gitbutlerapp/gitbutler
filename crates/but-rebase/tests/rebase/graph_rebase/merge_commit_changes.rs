use anyhow::Result;
use bstr::ByteSlice as _;
use but_core::RepositoryExt;
use but_graph::Graph;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{Editor, LookupStep as _, Step},
};
use but_testsupport::visualize_commit_graph_all;
use gix::prelude::ObjectIdExt;
use std::mem::ManuallyDrop;

use crate::utils::{fixture, fixture_writable, standard_options};

#[test]
fn matches_clean_octopus_merge() -> Result<()> {
    let (repo, mut meta) = fixture("octopus-merge-with-redundant-input")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   a7dcd9f (HEAD -> main) octopus
    |\ \  
    | | * 2a5954a (right) right
    | |/  
    |/|   
    | * cbaa825 (left) left-2
    | * 777f2d5 left-1
    |/  
    * 66df43d base
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

    let left_1 = repo.rev_parse_single("left~1")?.detach();
    let left_2 = repo.rev_parse_single("left")?.detach();
    let right = repo.rev_parse_single("right")?.detach();
    let expected_tree = repo.rev_parse_single("main^{tree}")?.detach();

    let actual_tree = editor.merge_commit_changes_to_tree(
        left_1,
        vec![left_2, right],
        editor.repo().merge_options_fail_fast()?.0,
    )?;

    assert_eq!(
        actual_tree.tree_id, expected_tree,
        "the target tree should anchor the merged tree while later ranges add their own deltas"
    );
    assert!(actual_tree.conflict.is_none());
    Ok(())
}

#[test]
fn excludes_unselected_parent_changes() -> Result<()> {
    let (repo, mut meta) = fixture("merge-commits-excludes-unselected-parent-visible")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   e8da81c (HEAD -> main) merge
    |\ \  
    | | * fa946b5 (C) C
    | | * 2eb5a0f (B) B
    | |/  
    |/|   
    | * cec649d (A) A
    |/  
    * b301433 M
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

    let a_commit = repo.rev_parse_single("A")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();
    let merged_tree = editor.merge_commit_changes_to_tree(
        a_commit,
        vec![c_commit],
        editor.repo().merge_options_fail_fast()?.0,
    )?;

    let file_a = editor
        .repo()
        .find_tree(merged_tree.tree_id)?
        .lookup_entry_by_path("file-a")?
        .expect("file-a should be present")
        .object()?;
    assert_eq!(file_a.data.as_bstr(), "a\n");

    let file_c = editor
        .repo()
        .find_tree(merged_tree.tree_id)?
        .lookup_entry_by_path("file-c")?
        .expect("file-c should be present")
        .object()?;
    assert_eq!(file_c.data.as_bstr(), "c\n");

    assert!(
        editor
            .repo()
            .find_tree(merged_tree.tree_id)?
            .lookup_entry_by_path("file-b")?
            .is_none(),
        "file-b should not be pulled in from C's unselected parent"
    );
    assert!(merged_tree.conflict.is_none());

    Ok(())
}

#[test]
fn reports_conflicts() -> Result<()> {
    let (repo, mut meta) = fixture("merge-commit-changes-fail-fast-after-conflict-visible")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   9b1de89 (HEAD -> main) merge-C
    |\  
    | * ea9d91a (C) C
    * |   669016a merge-B
    |\ \  
    | * | a1163f7 (B) B
    | |/  
    * |   468ee64 merge-A
    |\ \  
    | |/  
    |/|   
    | * 332e45d (A) A
    |/  
    * 66df43d base
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

    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let merged = editor.merge_commit_changes_to_tree(
        a_commit,
        vec![b_commit],
        editor.repo().merge_options_force_ours()?,
    )?;

    let conflict = merged
        .conflict
        .expect("conflicting merge should report conflict metadata");
    assert_eq!(
        conflict.tree_expression.base_tree_ids,
        vec![repo.rev_parse_single("A~1^{tree}")?.detach()]
    );
    assert_eq!(
        conflict.tree_expression.side_tree_ids.into_vec(),
        vec![
            repo.rev_parse_single("A^{tree}")?.detach(),
            repo.rev_parse_single("B^{tree}")?.detach()
        ]
    );
    assert!(conflict.conflict_entries.has_entries());

    let merged_file = editor
        .repo()
        .find_tree(merged.tree_id)?
        .lookup_entry_by_path("shared.txt")?
        .expect("merged shared file should be present")
        .object()?;
    assert_eq!(merged_file.data.as_bstr(), "A\n");

    Ok(())
}

#[test]
fn stops_folding_after_first_conflict() -> Result<()> {
    let (repo, mut meta) = fixture("merge-commit-changes-fail-fast-after-conflict-visible")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   9b1de89 (HEAD -> main) merge-C
    |\  
    | * ea9d91a (C) C
    * |   669016a merge-B
    |\ \  
    | * | a1163f7 (B) B
    | |/  
    * |   468ee64 merge-A
    |\ \  
    | |/  
    |/|   
    | * 332e45d (A) A
    |/  
    * 66df43d base
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

    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();
    let c_commit = repo.rev_parse_single("C")?.detach();
    let merged = editor.merge_commit_changes_to_tree(
        a_commit,
        vec![b_commit, c_commit],
        editor.repo().merge_options_force_ours()?,
    )?;

    assert!(
        merged.conflict.is_some(),
        "the A/B merge should report a conflict"
    );

    let tree = editor.repo().find_tree(merged.tree_id)?;
    let shared = tree
        .lookup_entry_by_path("shared.txt")?
        .expect("shared.txt should be present")
        .object()?;
    assert_eq!(shared.data.as_bstr(), "A\n");

    assert!(
        tree.lookup_entry_by_path("file-c")?.is_some(),
        "canonical planner ordering should apply C before the conflicting B range"
    );

    Ok(())
}

#[test]
fn preserves_noncontiguous_selected_changes() -> Result<()> {
    let (repo, mut meta) =
        fixture("merge-commits-preserve-noncontiguous-selected-changes-visible")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   1c47eb3 (HEAD -> main) merge
    |\ \  
    | | * bf7c931 (B) D
    | | * fa946b5 C
    | | * 2eb5a0f B
    | |/  
    |/|   
    | * cec649d (A) A
    |/  
    * b301433 M
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

    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B~2")?.detach();
    let d_commit = repo.rev_parse_single("B")?.detach();
    let merged_tree = editor.merge_commit_changes_to_tree(
        a_commit,
        vec![b_commit, d_commit],
        editor.repo().merge_options_fail_fast()?.0,
    )?;

    let tree = editor.repo().find_tree(merged_tree.tree_id)?;

    let file_a = tree
        .lookup_entry_by_path("file-a")?
        .expect("file-a should be present")
        .object()?;
    assert_eq!(file_a.data.as_bstr(), "a\n");

    let file_b = tree
        .lookup_entry_by_path("file-b")?
        .expect("file-b should be present")
        .object()?;
    assert_eq!(file_b.data.as_bstr(), "b\n");

    let file_d = tree
        .lookup_entry_by_path("file-d")?
        .expect("file-d should be present")
        .object()?;
    assert_eq!(file_d.data.as_bstr(), "d\n");

    assert!(
        tree.lookup_entry_by_path("file-c")?.is_none(),
        "file-c should not be pulled in when only B and D are selected"
    );
    assert!(merged_tree.conflict.is_none());

    Ok(())
}

#[test]
fn preserves_first_selected_commit_tree_while_applying_later_selected_ranges() -> Result<()> {
    let (repo, mut meta) = fixture("merge-commits-preserve-anchor-tree-visible")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *-.   e84cecd (HEAD -> main) merge
    |\ \  
    | | * e105958 (E) E
    | | * 739244f (C) C
    | | * 7f69bb3 (B) B
    | |/  
    |/|   
    | * 06e6d84 (D) D
    | * cec649d (A) A
    |/  
    * b301433 M
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

    let d_commit = repo.rev_parse_single("D")?.detach();
    let e_commit = repo.rev_parse_single("E")?.detach();
    let merged_tree = editor.merge_commit_changes_to_tree(
        d_commit,
        vec![e_commit],
        editor.repo().merge_options_fail_fast()?.0,
    )?;

    let tree = editor.repo().find_tree(merged_tree.tree_id)?;

    let file_a = tree
        .lookup_entry_by_path("file-a")?
        .expect("file-a should be present from the anchor commit tree")
        .object()?;
    assert_eq!(file_a.data.as_bstr(), "a\n");

    let file_d = tree
        .lookup_entry_by_path("file-d")?
        .expect("file-d should be present from the anchor commit tree")
        .object()?;
    assert_eq!(file_d.data.as_bstr(), "d\n");

    let file_e = tree
        .lookup_entry_by_path("file-e")?
        .expect("file-e should be present from the later selected range")
        .object()?;
    assert_eq!(file_e.data.as_bstr(), "e\n");

    assert!(
        tree.lookup_entry_by_path("file-b")?.is_none(),
        "file-b should not be pulled in from E's unselected ancestors"
    );
    assert!(
        tree.lookup_entry_by_path("file-c")?.is_none(),
        "file-c should not be pulled in from E's unselected ancestors"
    );
    assert!(merged_tree.conflict.is_none());

    Ok(())
}

#[test]
fn planning_preserves_noncontiguous_selected_changes() -> Result<()> {
    let (repo, mut meta) =
        fixture("merge-commits-preserve-noncontiguous-selected-changes-visible")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a_commit = repo.rev_parse_single("A")?.detach();
    let b_commit = repo.rev_parse_single("B~2")?.detach();
    let d_commit = repo.rev_parse_single("B")?.detach();

    let target = repo.rev_parse_single("A~1")?.detach();
    let plan = editor.plan_commit_changes_for_merge(target, vec![a_commit, b_commit, d_commit])?;

    insta::assert_snapshot!(labeled_plan_entries(&repo, &plan), @"
    B <- M
    D <- C
    A <- M
    ");
    Ok(())
}

#[test]
fn planning_fixture_graph() -> Result<()> {
    let fixture = simplify_fixture()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&fixture.repo)?, @r"
    *-.   d0949aa (HEAD -> main) merged
    |\ \  
    | | * 8259b01 (right) right-3
    | | * 0a63ea6 right-2
    | | * 26b0bd5 right-1
    | * | feaa00d (left) left-3
    | * | 07bba81 left-2
    | * | 4b6a0f2 left-1
    | |/  
    * | f1b6511 main-3
    * | 6bbd9db main-2
    * | 257ee22 main-1
    |/  
    * 6dbc49d base
    ");

    Ok(())
}

#[test]
fn planning_collapses_contiguous_selected_chain() -> Result<()> {
    let mut fixture = simplify_fixture()?;
    let editor = Editor::create(&mut fixture.ws, &mut *fixture.meta, &fixture.repo)?;

    let plan = editor.plan_commit_changes_for_merge(
        fixture.base,
        vec![fixture.left_1, fixture.left_2, fixture.left_3],
    )?;

    insta::assert_snapshot!(labeled_plan_entries(&fixture.repo, &plan), @"left-3 <- base");
    Ok(())
}

#[test]
fn planning_preserves_unrelated_branch_tips() -> Result<()> {
    let mut fixture = simplify_fixture()?;
    let editor = Editor::create(&mut fixture.ws, &mut *fixture.meta, &fixture.repo)?;

    let plan = editor.plan_commit_changes_for_merge(
        fixture.base,
        vec![
            fixture.left_1,
            fixture.left_3,
            fixture.main_2,
            fixture.right_1,
            fixture.right_3,
        ],
    )?;

    insta::assert_snapshot!(labeled_plan_entries(&fixture.repo, &plan), @"
    right-1 <- base
    right-3 <- right-2
    left-1 <- base
    left-3 <- left-2
    main-2 <- main-1
    ");
    Ok(())
}

#[test]
fn planning_deduplicates_and_keeps_order_of_survivors() -> Result<()> {
    let mut fixture = simplify_fixture()?;
    let editor = Editor::create(&mut fixture.ws, &mut *fixture.meta, &fixture.repo)?;

    let plan = editor.plan_commit_changes_for_merge(
        fixture.base,
        vec![
            fixture.main_3,
            fixture.left_2,
            fixture.main_2,
            fixture.left_2,
            fixture.right_3,
            fixture.left_3,
            fixture.right_1,
        ],
    )?;

    insta::assert_snapshot!(labeled_plan_entries(&fixture.repo, &plan), @"
    right-1 <- base
    right-3 <- right-2
    left-3 <- left-1
    main-3 <- main-1
    ");
    Ok(())
}

#[test]
fn uses_editor_visible_commits_not_only_original_workspace_graph() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("four-commits")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let head = repo.rev_parse_single("HEAD")?.detach();
    let mut head_commit = editor.find_commit(head)?;
    head_commit.inner.message = "HEAD rewritten inside editor".into();
    let rewritten_head =
        editor.new_commit_untracked(head_commit, DateMode::CommitterUpdateAuthorKeep)?;

    let head_selector = editor.select_commit(head)?;
    editor.replace(head_selector, Step::new_pick(rewritten_head))?;

    let merged = editor.merge_commit_changes_to_tree(
        rewritten_head,
        Vec::new(),
        editor.repo().merge_options_fail_fast()?.0,
    )?;
    let expected_tree = but_core::Commit::from_id(rewritten_head.attach(editor.repo()))?
        .tree_id_or_auto_resolution()?
        .detach();

    assert_eq!(merged.tree_id, expected_tree);
    assert!(merged.conflict.is_none());
    Ok(())
}

#[test]
fn planning_prunes_subjects_reachable_from_target_first_parent_lineage() -> Result<()> {
    let (repo, mut meta) = fixture("merge-commits-preserve-anchor-tree-visible")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let d_commit = repo.rev_parse_single("D")?.detach();
    let a_commit = repo.rev_parse_single("A")?.detach();

    let plan = editor.plan_commit_changes_for_merge(d_commit, vec![a_commit])?;

    insta::assert_snapshot!(labeled_plan_entries(&repo, &plan), @"");
    Ok(())
}

#[test]
fn planning_prunes_subjects_reachable_from_target_merge_parent_lineage() -> Result<()> {
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

    let merge_commit = repo.rev_parse_single("main")?.detach();
    let b_commit = repo.rev_parse_single("B")?.detach();

    let plan = editor.plan_commit_changes_for_merge(merge_commit, vec![b_commit])?;

    insta::assert_snapshot!(labeled_plan_entries(&repo, &plan), @"");
    Ok(())
}

#[test]
fn planning_prunes_target_ancestors_and_keeps_external_subject_order() -> Result<()> {
    let mut fixture = simplify_fixture()?;
    let editor = Editor::create(&mut fixture.ws, &mut *fixture.meta, &fixture.repo)?;

    let plan = editor.plan_commit_changes_for_merge(
        fixture.main_3,
        vec![
            fixture.left_1,
            fixture.main_2,
            fixture.right_3,
            fixture.left_3,
            fixture.right_1,
        ],
    )?;

    insta::assert_snapshot!(labeled_plan_entries(&fixture.repo, &plan), @"
    right-1 <- base
    right-3 <- right-2
    left-1 <- base
    left-3 <- left-2
    ");
    Ok(())
}

#[test]
fn planning_uses_pruned_selected_first_parent_tree_as_base_boundary() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("two-branches-shared-bottom-two")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let base = repo.rev_parse_single("right~2")?.detach();
    let shared = repo.rev_parse_single("right~1")?.detach();
    let left = repo.rev_parse_single("left")?.detach();
    let right = repo.rev_parse_single("right")?.detach();

    let plan = editor.plan_commit_changes_for_merge(right, vec![base, shared, left])?;

    insta::assert_snapshot!(labeled_plan_entries(&repo, &plan), @"left: head <- shared");
    Ok(())
}

#[test]
fn planning_works_after_normalizing_chained_editor_mutations() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("four-commits")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let head = repo.rev_parse_single("HEAD")?.detach();
    let head_parent = repo.rev_parse_single("HEAD~1")?.detach();
    let mut rewritten_head = editor.find_commit(head)?;
    rewritten_head.inner.message = "HEAD rewritten inside editor".into();
    let rewritten_head =
        editor.new_commit_untracked(rewritten_head, DateMode::CommitterUpdateAuthorKeep)?;

    let head_selector = editor.select_commit(head)?;
    editor.replace(head_selector, Step::new_pick(rewritten_head))?;
    let editor = editor.rebase()?.into_editor();
    let rewritten_head = editor.lookup_pick(head_selector)?;

    let plan = editor.plan_commit_changes_for_merge(rewritten_head, vec![head_parent])?;

    insta::assert_snapshot!(labeled_plan_entries(&repo, &plan), @"");
    Ok(())
}

struct SimplifyFixture {
    repo: gix::Repository,
    meta: ManuallyDrop<but_meta::VirtualBranchesTomlMetadata>,
    ws: but_graph::Workspace,
    base: gix::ObjectId,
    main_2: gix::ObjectId,
    main_3: gix::ObjectId,
    left_1: gix::ObjectId,
    left_2: gix::ObjectId,
    left_3: gix::ObjectId,
    right_1: gix::ObjectId,
    right_3: gix::ObjectId,
}

fn simplify_fixture() -> Result<SimplifyFixture> {
    let (repo, meta) = fixture("three-branches-three-commits-visible")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let ws = graph.into_workspace()?;

    let base = repo.rev_parse_single("main~4")?.detach();
    let main_2 = repo.rev_parse_single("main~2")?.detach();
    let main_3 = repo.rev_parse_single("main~1")?.detach();
    let left_1 = repo.rev_parse_single("left~2")?.detach();
    let left_2 = repo.rev_parse_single("left~1")?.detach();
    let left_3 = repo.rev_parse_single("left")?.detach();
    let right_1 = repo.rev_parse_single("right~2")?.detach();
    let right_3 = repo.rev_parse_single("right")?.detach();

    Ok(SimplifyFixture {
        repo,
        meta,
        ws,
        base,
        main_2,
        main_3,
        left_1,
        left_2,
        left_3,
        right_1,
        right_3,
    })
}

fn labeled_plan_entries(
    repo: &gix::Repository,
    plan: &[but_rebase::graph_rebase::merge_commit_changes::PlannedCommitChange],
) -> String {
    plan.iter()
        .map(|entry| {
            let commit =
                label_commit_by_subject(repo, entry.commit_id).unwrap_or_else(|| "unknown".into());
            let base =
                label_tree_by_subject(repo, entry.base_tree_id).unwrap_or_else(|| "unknown".into());
            format!("{commit} <- {base}")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn label_commit_by_subject(repo: &gix::Repository, commit_id: gix::ObjectId) -> Option<String> {
    let commit = repo.find_commit(commit_id).ok()?;
    commit_subject(commit.message_raw().ok()?)
}

fn label_tree_by_subject(repo: &gix::Repository, tree_id: gix::ObjectId) -> Option<String> {
    if tree_id == gix::ObjectId::empty_tree(repo.object_hash()) {
        return Some("empty".into());
    }

    [
        "main", "main~1", "main~2", "main~3", "main~4", "left", "left~1", "left~2", "left~3",
        "right", "right~1", "right~2", "right~3", "A", "A~1", "B", "B~1", "B~2", "C", "D", "E",
    ]
    .into_iter()
    .find_map(|spec| {
        let commit_id = repo.rev_parse_single(spec).ok()?.detach();
        let commit = repo.find_commit(commit_id).ok()?;
        let commit_tree = commit.tree_id().ok()?.detach();
        if commit_tree == tree_id {
            commit_subject(commit.message_raw().ok()?)
        } else {
            None
        }
    })
}

fn commit_subject(message: &[u8]) -> Option<String> {
    let subject = std::str::from_utf8(message).ok()?.lines().next()?.trim();
    Some(subject.to_string())
}

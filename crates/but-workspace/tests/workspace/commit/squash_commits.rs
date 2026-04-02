use anyhow::Result;
use bstr::ByteSlice as _;
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use but_testsupport::{graph_workspace, visualize_commit_graph_all};
use but_workspace::commit::squash_commits;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments,
    named_writable_scenario_with_description_and_graph as writable_scenario,
};

#[test]
fn squash_top_commit_into_parent() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let subject_id = repo.rev_parse_single("three")?.detach();
    let target_id = repo.rev_parse_single("two")?.detach();
    let subject_tree = repo.find_commit(subject_id)?.tree_id()?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(editor, subject_id, target_id)?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit three\n\ncommit two\n",
        "combined message should be subject followed by target with one blank line"
    );
    assert_eq!(
        squashed_commit.tree_id()?.detach(),
        subject_tree,
        "squashed commit should take the top-most (subject) tree"
    );

    let two_tip = repo.find_reference("two")?.peel_to_id()?.detach();
    let three_tip = repo.find_reference("three")?.peel_to_id()?.detach();
    assert_eq!(
        two_tip, squashed_id,
        "target reference should point to squashed commit"
    );
    assert_eq!(
        three_tip, squashed_id,
        "subject reference should be reconnected to squashed commit"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 6426178 (HEAD -> three, two) commit three
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    Ok(())
}

#[test]
fn squash_reorders_when_subject_is_not_on_top() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    // Subject starts below target in history and needs internal reorder first.
    let subject_id = repo.rev_parse_single("two")?.detach();
    let target_id = repo.rev_parse_single("three")?.detach();
    let target_tree = repo.find_commit(target_id)?.tree_id()?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(editor, subject_id, target_id)?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit two\n\ncommit three\n",
        "combined message should respect subject-then-target order"
    );
    assert_eq!(
        squashed_commit.tree_id()?.detach(),
        target_tree,
        "when subject is above target in ancestry, the target tree is top-most and must be kept"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 655b033 (HEAD -> three, two) commit two
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    Ok(())
}

#[test]
fn squash_same_commit_is_rejected() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let commit_id = repo.rev_parse_single("two")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;

    let err = squash_commits(editor, commit_id, commit_id).expect_err("must fail");
    assert!(
        err.to_string()
            .contains("Cannot squash a commit into itself"),
        "error should make same-commit squash invalid"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    Ok(())
}

#[test]
fn squash_down_keeps_topmost_tree_for_shared_file_lineage() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("squash-shared-file-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a209f1b (HEAD -> three) commit three
    * c0570de (two) commit two
    * 8df0fa3 (one) commit one
    ");

    let subject_id = repo.rev_parse_single("three")?.detach();
    let target_id = repo.rev_parse_single("two")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(editor, subject_id, target_id)?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let spec = format!("{squashed_id}:shared.txt");
    let object = repo.rev_parse_single(spec.as_str())?.object()?;
    assert_eq!(object.data.as_bstr(), "v3\n");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 4fbac4b (HEAD -> three, two) commit three
    * 8df0fa3 (one) commit one
    ");

    Ok(())
}

#[test]
fn squash_up_reorders_subject_below_target_for_shared_file_lineage() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("squash-shared-file-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a209f1b (HEAD -> three) commit three
    * c0570de (two) commit two
    * 8df0fa3 (one) commit one
    ");

    let subject_id = repo.rev_parse_single("two")?.detach();
    let target_id = repo.rev_parse_single("three")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(editor, subject_id, target_id)?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let spec = format!("{squashed_id}:shared.txt");
    let object = repo.rev_parse_single(spec.as_str())?.object()?;
    assert_eq!(object.data.as_bstr(), "v3\n");

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit two\n\ncommit three\n"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 69e6e54 (HEAD -> three, two) commit two
    * 8df0fa3 (one) commit one
    ");

    Ok(())
}

#[test]
fn squash_down_out_of_order_reorders_subject_below_target_for_shared_file_lineage() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("squash-shared-file-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a209f1b (HEAD -> three) commit three
    * c0570de (two) commit two
    * 8df0fa3 (one) commit one
    ");

    let subject_id = repo.rev_parse_single("three")?.detach();
    let target_id = repo.rev_parse_single("one")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(editor, subject_id, target_id)?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let spec = format!("{squashed_id}:shared.txt");
    let object = repo.rev_parse_single(spec.as_str())?.object()?;
    assert_eq!(object.data.as_bstr(), "v3\n");

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit three\n\ncommit one\n"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 09ef005 (HEAD -> three, two) commit two
    * f297a08 (one) commit three
    ");

    Ok(())
}

#[test]
fn squash_across_stacks_subject_into_target() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-two-stacks", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {2}
    │   └── 📙:4:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let subject_id = repo.rev_parse_single("A")?.detach();
    let target_id = repo.rev_parse_single("B")?.detach();
    let subject_tree = repo.find_commit(subject_id)?.tree_id()?.detach();

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = squash_commits(editor, subject_id, target_id)?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(squashed_commit.message_raw()?, "A\n\nB\n");
    assert_eq!(squashed_commit.tree_id()?.detach(), subject_tree);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f0f9abb (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 17e27b0 (B) A
    |/  
    * 85efbe4 (origin/main, main, A) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:B on 85efbe4 {2}
    │   └── 📙:3:B
    │       └── ·17e27b0 (🏘️)
    └── ≡📙:4:A on 85efbe4 {1}
        └── 📙:4:A
    ");

    Ok(())
}

#[test]
fn squash_across_stacks_target_into_subject() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("ws-ref-ws-commit-two-stacks", |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
            add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        })?;

    let mut ws = graph.into_workspace()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/  
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {2}
    │   └── 📙:4:B
    │       └── ·c813d8d (🏘️)
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·09d8e52 (🏘️)
    ");

    let subject_id = repo.rev_parse_single("B")?.detach();
    let target_id = repo.rev_parse_single("A")?.detach();
    let subject_tree = repo.find_commit(subject_id)?.tree_id()?.detach();

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = squash_commits(editor, subject_id, target_id)?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(squashed_commit.message_raw()?, "B\n\nA\n");
    assert_eq!(squashed_commit.tree_id()?.detach(), subject_tree);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   3d10054 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | 82d6f41 (A) B
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:B on 85efbe4 {2}
    │   └── 📙:4:B
    └── ≡📙:3:A on 85efbe4 {1}
        └── 📙:3:A
            └── ·82d6f41 (🏘️)
    ");

    Ok(())
}

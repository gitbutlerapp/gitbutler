use anyhow::Result;
use bstr::ByteSlice as _;
use but_rebase::graph_rebase::{Editor, LookupStep};
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
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit two\n\ncommit three\n",
        "combined message should be target followed by subject with one blank line"
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
    * 655b033 (HEAD -> three, two) commit two
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    Ok(())
}

#[test]
fn squash_top_commit_into_parent_keeping_target_message() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let subject_id = repo.rev_parse_single("three")?.detach();
    let target_id = repo.rev_parse_single("two")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepTarget,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit two\n",
        "combined message should keep only the target message"
    );

    Ok(())
}

#[test]
fn squash_top_commit_into_parent_keeping_subject_message() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let subject_id = repo.rev_parse_single("three")?.detach();
    let target_id = repo.rev_parse_single("two")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepSubject,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit three\n",
        "combined message should keep only the subject message"
    );

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

    // Explicitly place the subject below the target before squashing.
    let subject_id = repo.rev_parse_single("two")?.detach();
    let target_id = repo.rev_parse_single("three")?.detach();
    let target_tree = repo.find_commit(target_id)?.tree_id()?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit three\n\ncommit two\n",
        "combined message should respect target-then-subject order"
    );
    assert_eq!(
        squashed_commit.tree_id()?.detach(),
        target_tree,
        "when subject is above target in ancestry, the target tree is top-most and must be kept"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 6426178 (HEAD -> three) commit three
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (two, one) commit one
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

    let err = squash_commits(
        editor,
        vec![commit_id],
        commit_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )
    .expect_err("must fail");
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
fn squash_rejects_target_in_subject_commit_ids() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let subject_id = repo.rev_parse_single("three")?.detach();
    let target_id = repo.rev_parse_single("two")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;

    let err = squash_commits(
        editor,
        vec![subject_id, target_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )
    .expect_err("must fail");
    assert!(
        err.to_string()
            .contains("Cannot squash a commit into itself"),
        "error should explain that target cannot be one of the source commits"
    );

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
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let spec = format!("{squashed_id}:shared.txt");
    let object = repo.rev_parse_single(spec.as_str())?.object()?;
    assert_eq!(object.data.as_bstr(), "v3\n");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 69e6e54 (HEAD -> three, two) commit two
    * 8df0fa3 (one) commit one
    ");

    Ok(())
}

#[test]
fn squash_move_subject_below_target_for_shared_file_lineage() -> Result<()> {
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
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let spec = format!("{squashed_id}:shared.txt");
    let object = repo.rev_parse_single(spec.as_str())?.object()?;
    assert_eq!(object.data.as_bstr(), "v3\n");

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(
        squashed_commit.message_raw()?,
        "commit three\n\ncommit two\n"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 4fbac4b (HEAD -> three) commit three
    * 8df0fa3 (two, one) commit one
    ");

    Ok(())
}

#[test]
fn squash_move_subject_above_target_out_of_order_for_shared_file_lineage_fails() -> Result<()> {
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
    let err = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )
    .expect_err("must fail when reordering produces conflicts");

    assert!(
        err.to_string()
            .contains("became conflicted after reordering"),
        "error should explain that conflicted commits cannot be squashed"
    );

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * a209f1b (HEAD -> three) commit three
    * c0570de (two) commit two
    * 8df0fa3 (one) commit one
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
    let normalized = visualize_commit_graph_all(&repo)?.replace("  \n", "\n");
    insta::assert_snapshot!(normalized, @r"
    *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/
    * 85efbe4 (origin/main, main) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:4:B on 85efbe4 {2}
        └── 📙:4:B
            └── ·c813d8d (🏘️)
    ");

    let subject_id = repo.rev_parse_single("A")?.detach();
    let target_id = repo.rev_parse_single("B")?.detach();
    let subject_tree = repo.find_commit(subject_id)?.tree_id()?.detach();

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(squashed_commit.message_raw()?, "B\n\nA\n");
    assert_eq!(squashed_commit.tree_id()?.detach(), subject_tree);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   e9ad17f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | 82d6f41 (B) B
    |/  
    * 85efbe4 (origin/main, main, A) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:4:A on 85efbe4 {1}
    │   └── 📙:4:A
    └── ≡📙:3:B on 85efbe4 {2}
        └── 📙:3:B
            └── ·82d6f41 (🏘️)
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

    let normalized = visualize_commit_graph_all(&repo)?.replace("  \n", "\n");
    insta::assert_snapshot!(normalized, @r"
    *   c49e4d8 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\
    | * 09d8e52 (A) A
    * | c813d8d (B) B
    |/
    * 85efbe4 (origin/main, main) M
    ");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·09d8e52 (🏘️)
    └── ≡📙:4:B on 85efbe4 {2}
        └── 📙:4:B
            └── ·c813d8d (🏘️)
    ");

    let subject_id = repo.rev_parse_single("B")?.detach();
    let target_id = repo.rev_parse_single("A")?.detach();
    let subject_tree = repo.find_commit(subject_id)?.tree_id()?.detach();

    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = squash_commits(
        editor,
        vec![subject_id],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let squashed_commit = repo.find_commit(squashed_id)?;
    assert_eq!(squashed_commit.message_raw()?, "A\n\nB\n");
    assert_eq!(squashed_commit.tree_id()?.detach(), subject_tree);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   b7ec700 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 17e27b0 (A) A
    |/  
    * 85efbe4 (origin/main, main, B) M
    ");
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 85efbe4
    ├── ≡📙:3:A on 85efbe4 {1}
    │   └── 📙:3:A
    │       └── ·17e27b0 (🏘️)
    └── ≡📙:4:B on 85efbe4 {2}
        └── 📙:4:B
    ");

    Ok(())
}

#[test]
fn squash_all_c_commits_into_second_commit_of_b_keeps_new_file_content() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) = writable_scenario("three-stacks", |meta| {
        add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
        add_stack_with_segments(meta, 3, "C", StackState::InWorkspace, &[]);
    })?;

    let c_top = repo.rev_parse_single("C")?.detach();
    let c_second = repo.rev_parse_single("C~1")?.detach();
    let c_third = repo.rev_parse_single("C~2")?.detach();
    let target_id = repo.rev_parse_single("B~1")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = squash_commits(
        editor,
        vec![c_top, c_second, c_third],
        target_id,
        squash_commits::MessageCombinationStrategy::KeepBoth,
    )?;

    let materialized = outcome.rebase.materialize()?;
    let squashed_id = materialized.lookup_pick(outcome.commit_selector)?;

    let new_file_blob = repo
        .rev_parse_single(format!("{squashed_id}:new-file").as_str())?
        .object()?;
    insta::assert_snapshot!(new_file_blob.data.as_bstr(), @"
    1
    2
    3
    4
    5
    6
    7
    8
    9
    10
    11
    12
    13
    14
    15
    16
    17
    18
    19
    20
    21
    22
    23
    24
    25
    26
    27
    28
    29
    30
    ");

    Ok(())
}

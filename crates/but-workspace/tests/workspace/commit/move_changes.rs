use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{Editor, Selector, ToCommitSelector as _, mutate::InsertSide};
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::{
    discard_commits, insert_blank_commit, move_changes_between_commits, squash_commits,
};
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

fn diff_spec_for_file(path: &str) -> DiffSpec {
    DiffSpec {
        previous_path: None,
        path: path.into(),
        hunk_headers: vec![],
    }
}

fn visualize_tree(id: gix::Id<'_>) -> String {
    but_testsupport::visualize_tree(id).to_string()
}

fn split_above_itself<'ws, 'meta, M: but_core::RefMetadata>(
    editor: Editor<'ws, 'meta, M>,
    source: Selector,
    file: &str,
) -> Result<(Editor<'ws, 'meta, M>, Selector)> {
    let (rebase, blank) = insert_blank_commit(editor, InsertSide::Above, source)?;
    let mut editor = rebase.into_editor();
    editor.split_from(source, blank)?;
    let moved = move_changes_between_commits(
        editor.rebase()?.into_editor(),
        source,
        blank,
        [diff_spec_for_file(file)],
        0,
    )?;
    Ok((moved.rebase.into_editor(), moved.destination_selector))
}

fn split_two_above_itself<'ws, 'meta, M: but_core::RefMetadata>(
    editor: Editor<'ws, 'meta, M>,
    repo: &gix::Repository,
) -> Result<(Editor<'ws, 'meta, M>, Selector)> {
    let two = repo.rev_parse_single("two")?.detach();
    let two = two.to_commit_selector(&editor)?;
    split_above_itself(editor, two, "two.txt")
}

#[test]
fn splitting_records_both_commits_parent_first() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let (editor, _) = split_two_above_itself(editor, &repo)?;

    // `two` became what's left of it and the new commit, and `three` moved on top of those.
    snapbox::assert_data_eq!(
        editor.rebase()?.rewrites()?.to_debug(),
        snapbox::str![[r#"
[
    Split {
        prec: Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d),
        succs: [
            Sha1(ebc7217ab9e33830c4f09601dfda826eb2e9fed0),
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
        ],
    },
    Replace {
        prec: Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae),
        succ: Sha1(54a799f3c5fd9b43b05c72da1344ab9a40d18252),
    },
]

"#]]
    );
    Ok(())
}

#[test]
fn folding_one_half_of_each_of_two_splits_is_a_fold_between_them() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two = repo.rev_parse_single("two")?.detach();
    let three = repo.rev_parse_single("three")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let two = two.to_commit_selector(&editor)?;
    let three = three.to_commit_selector(&editor)?;
    let (editor, new_two) = split_above_itself(editor, two, "two.txt")?;
    let (editor, new_three) =
        split_above_itself(editor.rebase()?.into_editor(), three, "three.txt")?;
    let outcome = squash_commits(
        editor.rebase()?.into_editor(),
        vec![new_three],
        new_two,
        squash_commits::MessageCombinationStrategy::KeepTarget,
    )?;

    // The new halves never left the session, but they're what the fold folded.
    snapbox::assert_data_eq!(
        outcome.rebase.rewrites()?.to_debug(),
        snapbox::str![[r#"
[
    Fold {
        precs: [
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
            Sha1(288a385368b0ee35aa726bcb02cc7c940381d966),
        ],
        succ: Sha1(5dcea5492787303ae262152859fa924e7017a398),
    },
    Split {
        prec: Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d),
        succs: [
            Sha1(ebc7217ab9e33830c4f09601dfda826eb2e9fed0),
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
        ],
    },
    Split {
        prec: Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae),
        succs: [
            Sha1(830f8f5e69dc6becd18e9db7733a5ec7dfcd2868),
            Sha1(288a385368b0ee35aa726bcb02cc7c940381d966),
        ],
    },
]

"#]]
    );
    Ok(())
}

#[test]
fn splitting_into_a_commit_that_has_changes_is_refused() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two = repo.rev_parse_single("two")?.detach();
    let three = repo.rev_parse_single("three")?.detach();
    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;

    assert!(
        editor.split_from(two, three).is_err(),
        "a split recorded after its changes moved would name the wrong commits"
    );
    Ok(())
}

#[test]
fn folding_half_of_a_split_into_another_commit_keeps_both_hyperedges() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let one = repo.rev_parse_single("one")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let (editor, new_commit) = split_two_above_itself(editor, &repo)?;
    let outcome = squash_commits(
        editor.rebase()?.into_editor(),
        vec![new_commit],
        one,
        squash_commits::MessageCombinationStrategy::KeepTarget,
    )?;

    // `two` still has two successors, so the squashed commit isn't a fold of `one` and `two`.
    snapbox::assert_data_eq!(
        outcome.rebase.rewrites()?.to_debug(),
        snapbox::str![[r#"
[
    Fold {
        precs: [
            Sha1(8b426d09509e2a1e924d939055e1e3eb4b6e7fb4),
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
        ],
        succ: Sha1(01a75ca6ca2bcc260bd5a581149c070c8199b8ff),
    },
    Split {
        prec: Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d),
        succs: [
            Sha1(571f747348d6dd2ae70eebfd94270038769bea9b),
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
        ],
    },
    Replace {
        prec: Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae),
        succ: Sha1(d1f692179f9179ecfbac5701b4875c86d2b2c5f6),
    },
]

"#]]
    );
    Ok(())
}

#[test]
fn folding_a_split_back_into_its_source_keeps_both_hyperedges() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let (editor, new_commit) = split_two_above_itself(editor, &repo)?;
    let rebase = editor.rebase()?;
    let source = repo.rev_parse_single("two")?.detach();
    let source = rebase.history.continuations()[&source];
    let outcome = squash_commits(
        rebase.into_editor(),
        vec![new_commit],
        source,
        squash_commits::MessageCombinationStrategy::KeepTarget,
    )?;

    // The halves were real commits, so the fold is from them rather than from `two` again.
    snapbox::assert_data_eq!(
        outcome.rebase.rewrites()?.to_debug(),
        snapbox::str![[r#"
[
    Fold {
        precs: [
            Sha1(ebc7217ab9e33830c4f09601dfda826eb2e9fed0),
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
        ],
        succ: Sha1(987b5db2b72bf49d065a1231f6ea764a88a91aae),
    },
    Split {
        prec: Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d),
        succs: [
            Sha1(ebc7217ab9e33830c4f09601dfda826eb2e9fed0),
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
        ],
    },
    Replace {
        prec: Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae),
        succ: Sha1(fdf0a120bba687f17ff67d7926572d39694e0a01),
    },
]

"#]]
    );
    Ok(())
}

#[test]
fn discarding_half_of_a_split_prunes_that_half() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo, &mut db)?;
    let (editor, new_commit) = split_two_above_itself(editor, &repo)?;
    let rebase = editor.rebase()?;
    let new_commit = rebase.lookup_commit(new_commit)?.id;
    let rebase = discard_commits(rebase.into_editor(), [new_commit])?;

    // `two` lives on in what's left of it, so only the new commit is pruned.
    snapbox::assert_data_eq!(
        rebase.rewrites()?.to_debug(),
        snapbox::str![[r#"
[
    Prune {
        prec: Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
    },
    Split {
        prec: Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d),
        succs: [
            Sha1(ebc7217ab9e33830c4f09601dfda826eb2e9fed0),
            Sha1(0cf46ba9cd6f1285a930cfdba8f4795c96ce5c20),
        ],
    },
    Replace {
        prec: Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae),
        succ: Sha1(b5e5f88a9d51e96f468dc8ca95e069d69cc1b262),
    },
]

"#]]
    );
    Ok(())
}

#[test]
fn move_changes_same_commit_is_noop() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let commit_id = repo.rev_parse_single("three")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;

    // Moving changes from a commit to itself should be a no-op
    let outcome =
        move_changes_between_commits(editor, commit_id, commit_id, Vec::<DiffSpec>::new(), 0)?;

    // Materialize should succeed
    outcome.rebase.materialize(Default::default())?;

    // Graph should be unchanged
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    Ok(())
}

#[test]
fn move_file_from_head_to_parent() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    // Verify initial tree contents
    let three_id = repo.rev_parse_single("three")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();

    snapbox::assert_data_eq!(
        visualize_tree(three_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
e0495e9
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
├── three.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    snapbox::assert_data_eq!(
        visualize_tree(two_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
aac5238
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // Move three.txt from commit three to commit two
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let outcome = move_changes_between_commits(
        editor,
        three_id,
        two_id,
        vec![diff_spec_for_file("three.txt")],
        0,
    )?;

    let materialized = outcome.rebase.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        materialized.history.continuations().to_debug(),
        snapbox::str![[r#"
{
    Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d): Sha1(198bf421399ca45dfc033777ed30ea2241c55ff4),
    Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae): Sha1(7cb368cce7a1e76a623c8d7dcd03bcff3145f06a),
}

"#]]
    );

    // Graph structure should be maintained (commit hashes will change)
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 7cb368c (HEAD -> three) commit three
* 198bf42 (two) commit two
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );

    // Verify new tree contents - three.txt should be in commit two's tree, not three's
    let new_three_id = repo.rev_parse_single("three")?.detach();
    let new_two_id = repo.rev_parse_single("two")?.detach();

    // commit three should no longer introduce three.txt
    snapbox::assert_data_eq!(
        visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
e0495e9
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
├── three.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // commit two should now have three.txt
    snapbox::assert_data_eq!(
        visualize_tree(new_two_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
e0495e9
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
├── three.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn move_file_from_parent_to_head() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let three_id = repo.rev_parse_single("three")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();

    // Move two.txt from commit two up to commit three
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let outcome = move_changes_between_commits(
        editor,
        two_id,
        three_id,
        vec![diff_spec_for_file("two.txt")],
        0,
    )?;

    let materialized = outcome.rebase.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        materialized.history.continuations().to_debug(),
        snapbox::str![[r#"
{
    Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d): Sha1(ebc7217ab9e33830c4f09601dfda826eb2e9fed0),
    Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae): Sha1(f56f108a3b4bf2a67e554d22deb7a22bd7e93955),
}

"#]]
    );

    // Graph structure should be maintained
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f56f108 (HEAD -> three) commit three
* ebc7217 (two) commit two
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );

    // Verify tree contents
    let new_three_id = repo.rev_parse_single("three")?.detach();
    let new_two_id = repo.rev_parse_single("two")?.detach();

    // commit two should no longer have two.txt
    snapbox::assert_data_eq!(
        visualize_tree(new_two_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
6820889
├── .gitignore:100644:f4ec724 "/remote/\n"
└── one.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // commit three should still have three.txt AND now have two.txt
    snapbox::assert_data_eq!(
        visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
e0495e9
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
├── three.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn move_file_between_non_adjacent_commits() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let three_id = repo.rev_parse_single("three")?.detach();
    let one_id = repo.rev_parse_single("one")?.detach();

    // Move three.txt from commit three to commit one (skipping two)
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let outcome = move_changes_between_commits(
        editor,
        three_id,
        one_id,
        vec![diff_spec_for_file("three.txt")],
        0,
    )?;

    let materialized = outcome.rebase.materialize(Default::default())?;
    snapbox::assert_data_eq!(
        materialized.history.continuations().to_debug(),
        snapbox::str![[r#"
{
    Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d): Sha1(d5ac8ac48b1eb997e273a131a88d57e4b8ae970d),
    Sha1(8b426d09509e2a1e924d939055e1e3eb4b6e7fb4): Sha1(4ae70a4fccb46431274b7c89b04600d0fe124b0b),
    Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae): Sha1(114c8f7e7527abaa7f35faaeb0c38c2d7db2c93e),
}

"#]]
    );

    // Graph structure should be maintained
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 114c8f7 (HEAD -> three) commit three
* d5ac8ac (two) commit two
* 4ae70a4 (one) commit one
* 16fd221 (origin/two) commit two
* 8b426d0 commit one

"#]]
    );

    // Verify tree contents
    let new_three_id = repo.rev_parse_single("three")?.detach();
    let new_two_id = repo.rev_parse_single("two")?.detach();
    let new_one_id = repo.rev_parse_single("one")?.detach();

    // commit one should now have three.txt
    snapbox::assert_data_eq!(
        visualize_tree(new_one_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
c97666c
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
└── three.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // commit two should be unchanged (still has two.txt, now also has three.txt from one)
    snapbox::assert_data_eq!(
        visualize_tree(new_two_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
e0495e9
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
├── three.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // commit three should no longer have three.txt as its own change
    snapbox::assert_data_eq!(
        visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()),
        snapbox::str![[r#"
e0495e9
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
├── three.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn error_when_changes_not_found_in_source() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description, mut db) =
        writable_scenario("reword-three-commits", |_| {})?;

    let three_id = repo.rev_parse_single("three")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();

    // Try to move a file that doesn't exist in source commit
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo, &mut db)?;
    let result = move_changes_between_commits(
        editor,
        three_id,
        two_id,
        vec![diff_spec_for_file("nonexistent.txt")],
        0,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("Failed to extract described changes"),
        "Expected error about failed extraction, got: {err}"
    );

    Ok(())
}

use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::Editor;
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::move_changes_between_commits;
use gix::prelude::ObjectIdExt;

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

#[test]
fn move_changes_same_commit_is_noop() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let commit_id = repo.rev_parse_single("three")?.detach();
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;

    // Moving changes from a commit to itself should be a no-op
    let outcome =
        move_changes_between_commits(editor, commit_id, commit_id, Vec::<DiffSpec>::new(), 0)?;

    // Materialize should succeed
    outcome.rebase.materialize()?;

    // Graph should be unchanged
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    Ok(())
}

#[test]
fn move_file_from_head_to_parent() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    // Verify initial tree contents
    let three_id = repo.rev_parse_single("three")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();

    insta::assert_snapshot!(visualize_tree(three_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    e0495e9
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    ├── three.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    insta::assert_snapshot!(visualize_tree(two_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    aac5238
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    // Move three.txt from commit three to commit two
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = move_changes_between_commits(
        editor,
        three_id,
        two_id,
        vec![diff_spec_for_file("three.txt")],
        0,
    )?;

    let materialized = outcome.rebase.materialize()?;
    insta::assert_debug_snapshot!(materialized.history.commit_mappings(), @"
    {
        Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d): Sha1(88ba151b2f14a87051cdbeb3bdf850a6175eb8fe),
        Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae): Sha1(5f79d8bc78f0fc157cf33456412cfa443c0ae4fc),
    }
    ");

    // Graph structure should be maintained (commit hashes will change)
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 5f79d8b (HEAD -> three) commit three
    * 88ba151 (two) commit two
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    // Verify new tree contents - three.txt should be in commit two's tree, not three's
    let new_three_id = repo.rev_parse_single("three")?.detach();
    let new_two_id = repo.rev_parse_single("two")?.detach();

    // commit three should no longer introduce three.txt
    insta::assert_snapshot!(visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    e0495e9
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    ├── three.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    // commit two should now have three.txt
    insta::assert_snapshot!(visualize_tree(new_two_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    e0495e9
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    ├── three.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    Ok(())
}

#[test]
fn move_file_from_parent_to_head() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let three_id = repo.rev_parse_single("three")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();

    // Move two.txt from commit two up to commit three
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = move_changes_between_commits(
        editor,
        two_id,
        three_id,
        vec![diff_spec_for_file("two.txt")],
        0,
    )?;

    let materialized = outcome.rebase.materialize()?;
    insta::assert_debug_snapshot!(materialized.history.commit_mappings(), @"
    {
        Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d): Sha1(0f198e0be723143e843ec2e2f9538f4ba815cd62),
        Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae): Sha1(cffb8daaedf92594a000665b3990714ad04115c7),
    }
    ");

    // Graph structure should be maintained
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * cffb8da (HEAD -> three) commit three
    * 0f198e0 (two) commit two
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    // Verify tree contents
    let new_three_id = repo.rev_parse_single("three")?.detach();
    let new_two_id = repo.rev_parse_single("two")?.detach();

    // commit two should no longer have two.txt
    insta::assert_snapshot!(visualize_tree(new_two_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    6820889
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    └── one.txt:100644:257cc56 "foo\n"
    "#);

    // commit three should still have three.txt AND now have two.txt
    insta::assert_snapshot!(visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    e0495e9
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    ├── three.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    Ok(())
}

#[test]
fn move_file_between_non_adjacent_commits() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let three_id = repo.rev_parse_single("three")?.detach();
    let one_id = repo.rev_parse_single("one")?.detach();

    // Move three.txt from commit three to commit one (skipping two)
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = move_changes_between_commits(
        editor,
        three_id,
        one_id,
        vec![diff_spec_for_file("three.txt")],
        0,
    )?;

    let materialized = outcome.rebase.materialize()?;
    insta::assert_debug_snapshot!(materialized.history.commit_mappings(), @"
    {
        Sha1(16fd22163adbb1118551777970db5fb4b59f6b9d): Sha1(d9e603c74228d57e22e16431db09647bc281b65a),
        Sha1(8b426d09509e2a1e924d939055e1e3eb4b6e7fb4): Sha1(9bc8248dd9ea42b08f66103cd43352fcd2f45f3d),
        Sha1(c9f444cbd4d94f5b90aaa3e6e2e388c876cdbdae): Sha1(cda98e06158943a26336b4584f016938ae72864b),
    }
    ");

    // Graph structure should be maintained
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * cda98e0 (HEAD -> three) commit three
    * d9e603c (two) commit two
    * 9bc8248 (one) commit one
    * 16fd221 (origin/two) commit two
    * 8b426d0 commit one
    ");

    // Verify tree contents
    let new_three_id = repo.rev_parse_single("three")?.detach();
    let new_two_id = repo.rev_parse_single("two")?.detach();
    let new_one_id = repo.rev_parse_single("one")?.detach();

    // commit one should now have three.txt
    insta::assert_snapshot!(visualize_tree(new_one_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    c97666c
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    └── three.txt:100644:257cc56 "foo\n"
    "#);

    // commit two should be unchanged (still has two.txt, now also has three.txt from one)
    insta::assert_snapshot!(visualize_tree(new_two_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    e0495e9
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    ├── three.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    // commit three should no longer have three.txt as its own change
    insta::assert_snapshot!(visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    e0495e9
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    ├── three.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    Ok(())
}

#[test]
fn error_when_changes_not_found_in_source() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let three_id = repo.rev_parse_single("three")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();

    // Try to move a file that doesn't exist in source commit
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
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

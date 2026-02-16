use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{GraphExt, LookupStep};
use but_testsupport::{visualize_commit_graph_all, visualize_tree};
use but_workspace::commit::uncommit_changes;
use gix::prelude::ObjectIdExt;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

fn diff_spec_for_file(path: &str) -> DiffSpec {
    DiffSpec {
        previous_path: None,
        path: path.into(),
        hunk_headers: vec![],
    }
}

#[test]
fn uncommit_file_from_head() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    // Verify initial tree contents
    let three_id = repo.rev_parse_single("three")?.detach();

    insta::assert_snapshot!(visualize_tree(three_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    e0495e9
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    ├── three.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    // Uncommit three.txt from commit three
    let editor = graph.to_editor(&repo)?;
    let outcome = uncommit_changes(editor, three_id, vec![diff_spec_for_file("three.txt")], 0)?;

    let materialized = outcome.rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    // Graph structure should be maintained (commit hash will change)
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 832a93c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    // Verify new tree contents - three.txt should no longer be in commit three's tree
    insta::assert_snapshot!(visualize_tree(new_commit_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    aac5238
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    Ok(())
}

#[test]
fn uncommit_file_from_parent() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let two_id = repo.rev_parse_single("two")?.detach();

    // Verify initial tree of commit two
    insta::assert_snapshot!(visualize_tree(two_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    aac5238
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    └── two.txt:100644:257cc56 "foo\n"
    "#);

    // Uncommit two.txt from commit two
    let editor = graph.to_editor(&repo)?;
    let outcome = uncommit_changes(editor, two_id, vec![diff_spec_for_file("two.txt")], 0)?;

    let materialized = outcome.rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    // Graph structure should be maintained
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 2c4471e (HEAD -> three) commit three
    * 0f198e0 (two) commit two
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    // Verify commit two no longer has two.txt
    insta::assert_snapshot!(visualize_tree(new_commit_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    6820889
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    └── one.txt:100644:257cc56 "foo\n"
    "#);

    // Verify commit three still has all files (two.txt reappears from three's perspective)
    let new_three_id = repo.rev_parse_single("three")?.detach();
    insta::assert_snapshot!(visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    c97666c
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    ├── one.txt:100644:257cc56 "foo\n"
    └── three.txt:100644:257cc56 "foo\n"
    "#);

    Ok(())
}

#[test]
fn uncommit_file_from_root_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let one_id = repo.rev_parse_single("one")?.detach();

    // Verify initial tree of commit one
    insta::assert_snapshot!(visualize_tree(one_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    6820889
    ├── .gitignore:100644:f4ec724 "/remote/\n"
    └── one.txt:100644:257cc56 "foo\n"
    "#);

    // Uncommit one.txt from commit one (the root commit)
    let editor = graph.to_editor(&repo)?;
    let outcome = uncommit_changes(editor, one_id, vec![diff_spec_for_file("one.txt")], 0)?;

    let materialized = outcome.rebase.materialize()?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    // Graph structure should be maintained
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 72f5d24 (HEAD -> three) commit three
    * 0a49f31 (two) commit two
    * 7fcda42 (one) commit one
    * 16fd221 (origin/two) commit two
    * 8b426d0 commit one
    ");

    // Verify commit one no longer has one.txt
    insta::assert_snapshot!(visualize_tree(new_commit_id.attach(&repo).object()?.peel_to_tree()?.id()), @r#"
    f2ff419
    └── .gitignore:100644:f4ec724 "/remote/\n"
    "#);

    Ok(())
}

#[test]
fn error_when_changes_not_found() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;

    let three_id = repo.rev_parse_single("three")?.detach();

    // Try to uncommit a file that doesn't exist in source commit
    let editor = graph.to_editor(&repo)?;
    let result = uncommit_changes(editor, three_id, vec![diff_spec_for_file("nonexistent.txt")], 0);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Failed to remove specified changes"),
        "Expected error about failed removal, got: {err}"
    );

    Ok(())
}

#[test]
fn uncommit_empty_changes_is_noop() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let three_id = repo.rev_parse_single("three")?.detach();

    // Uncommit with empty changes should effectively be a no-op rebase
    let editor = graph.to_editor(&repo)?;
    let outcome = uncommit_changes(editor, three_id, Vec::<DiffSpec>::new(), 0)?;

    outcome.rebase.materialize()?;

    // Graph should be unchanged
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * fbb2bd1 (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    Ok(())
}

use anyhow::Result;
use but_core::{DiffSpec, HunkHeader};
use but_rebase::graph_rebase::{Editor, LookupStep};
use but_testsupport::{hunk_header, visualize_commit_graph_all, visualize_tree};
use but_workspace::commit::{
    UncommitChangesSource, uncommit_changes, uncommit_changes_from_commits,
};
use gix::prelude::ObjectIdExt;
use snapbox::IntoData;
use std::collections::HashMap;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

fn diff_spec_for_file(path: &str) -> DiffSpec {
    DiffSpec {
        previous_path: None,
        path: path.into(),
        hunk_headers: vec![],
    }
}

fn source(commit_id: gix::ObjectId, path: &str) -> UncommitChangesSource {
    UncommitChangesSource {
        commit_id,
        changes: vec![diff_spec_for_file(path)],
    }
}

fn source_with_changes(commit_id: gix::ObjectId, paths: &[&str]) -> UncommitChangesSource {
    UncommitChangesSource {
        commit_id,
        changes: paths.iter().map(|path| diff_spec_for_file(path)).collect(),
    }
}

fn source_with_hunks(
    commit_id: gix::ObjectId,
    path: &str,
    hunk_headers: Vec<HunkHeader>,
) -> UncommitChangesSource {
    UncommitChangesSource {
        commit_id,
        changes: vec![DiffSpec {
            previous_path: None,
            path: path.into(),
            hunk_headers,
        }],
    }
}

fn graph_diff(before: &str, after: &str) -> String {
    let mut labels = HashMap::<String, String>::new();
    let before = normalize_graph(before, &mut labels);
    let after = normalize_graph(after, &mut labels);

    let mut out = String::from("--- before\n");
    for line in before.lines() {
        out.push('-');
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.push_str("+++ after\n");
    for line in after.lines() {
        out.push('+');
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn normalize_graph(graph: &str, labels: &mut HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut token = String::new();
    for ch in graph.chars() {
        if ch.is_ascii_hexdigit() {
            token.push(ch);
        } else {
            push_normalized_token(&mut out, &mut token, labels);
            out.push(ch);
        }
    }
    push_normalized_token(&mut out, &mut token, labels);
    out
}

fn push_normalized_token(
    out: &mut String,
    token: &mut String,
    labels: &mut HashMap<String, String>,
) {
    if (7..=40).contains(&token.len()) && token.chars().all(|ch| ch.is_ascii_hexdigit()) {
        let next = labels.len() + 1;
        let label = labels
            .entry(std::mem::take(token))
            .or_insert_with(|| format!("[C{next}]"));
        out.push_str(label);
    } else {
        out.push_str(token);
        token.clear();
    }
}

fn assert_worktree_file(repo: &gix::Repository, path: &str, expected: &str) {
    let actual = std::fs::read_to_string(repo.workdir().unwrap().join(path))
        .unwrap_or_else(|err| panic!("failed to read worktree file {path}: {err}"));
    assert_eq!(actual, expected, "worktree file {path} should match");
}

#[test]
fn uncommit_file_from_head() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
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

    snapbox::assert_data_eq!(
        visualize_tree(three_id.attach(&repo).object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
e0495e9
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
├── three.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // Uncommit three.txt from commit three
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = uncommit_changes(editor, three_id, vec![diff_spec_for_file("three.txt")], 0)?;

    let materialized = outcome.rebase.materialize(Default::default())?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    // Graph structure should be maintained (commit hash will change)
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 298ab48 (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    // Verify new tree contents - three.txt should no longer be in commit three's tree
    snapbox::assert_data_eq!(
        visualize_tree(new_commit_id.attach(&repo).object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
aac5238
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn uncommit_file_from_parent() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let two_id = repo.rev_parse_single("two")?.detach();

    // Verify initial tree of commit two
    snapbox::assert_data_eq!(
        visualize_tree(two_id.attach(&repo).object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
aac5238
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // Uncommit two.txt from commit two
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = uncommit_changes(editor, two_id, vec![diff_spec_for_file("two.txt")], 0)?;

    let materialized = outcome.rebase.materialize(Default::default())?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    // Graph structure should be maintained
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* b5e5f88 (HEAD -> three) commit three
* ebc7217 (two) commit two
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );

    // Verify commit two no longer has two.txt
    snapbox::assert_data_eq!(
        visualize_tree(new_commit_id.attach(&repo).object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
6820889
├── .gitignore:100644:f4ec724 "/remote/\n"
└── one.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // Verify commit three still has all files (two.txt reappears from three's perspective)
    let new_three_id = repo.rev_parse_single("three")?.detach();
    snapbox::assert_data_eq!(
        visualize_tree(new_three_id.attach(&repo).object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
c97666c
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
└── three.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn uncommit_file_from_root_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c9f444c (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    let one_id = repo.rev_parse_single("one")?.detach();

    // Verify initial tree of commit one
    snapbox::assert_data_eq!(
        visualize_tree(one_id.attach(&repo).object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
6820889
├── .gitignore:100644:f4ec724 "/remote/\n"
└── one.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );

    // Uncommit one.txt from commit one (the root commit)
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = uncommit_changes(editor, one_id, vec![diff_spec_for_file("one.txt")], 0)?;

    let materialized = outcome.rebase.materialize(Default::default())?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    // Graph structure should be maintained
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 5dbf8bd (HEAD -> three) commit three
* 9d299c1 (two) commit two
* c124928 (one) commit one
* 16fd221 (origin/two) commit two
* 8b426d0 commit one

"#]]
    );

    // Verify commit one no longer has one.txt
    snapbox::assert_data_eq!(
        visualize_tree(new_commit_id.attach(&repo).object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
f2ff419
└── .gitignore:100644:f4ec724 "/remote/\n"

"#]]
        .raw()
    );

    Ok(())
}

#[test]
fn error_when_changes_not_found() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let three_id = repo.rev_parse_single("three")?.detach();

    // Try to uncommit a file that doesn't exist in source commit
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let result = uncommit_changes(
        editor,
        three_id,
        vec![diff_spec_for_file("nonexistent.txt")],
        0,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("Failed to remove specified changes"),
        "Expected error about failed removal, got: {err}"
    );

    Ok(())
}

#[test]
fn uncommit_empty_changes_is_noop() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
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

    // Uncommit with empty changes should effectively be a no-op rebase
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = uncommit_changes(editor, three_id, Vec::<DiffSpec>::new(), 0)?;

    outcome.rebase.materialize(Default::default())?;

    // Graph should be unchanged
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* c389dc7 (HEAD -> three) commit three
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_groups_and_orders_sources() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let one_id = repo.rev_parse_single("one")?.detach();
    let two_id = repo.rev_parse_single("two")?.detach();
    let three_id = repo.rev_parse_single("three")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = uncommit_changes_from_commits(
        editor,
        [
            source(one_id, "one.txt"),
            source(three_id, "three.txt"),
            source(two_id, "two.txt"),
        ],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "all valid grouped sources should uncommit: {:?}",
        outcome.failures
    );

    let rebase = outcome
        .rebase
        .expect("valid sources should produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-* [C1] (HEAD -> three) commit three
-* [C2] (origin/two, two) commit two
-* [C3] (one) commit one
+++ after
+* [C4] (HEAD -> three) commit three
+* [C5] (two) commit two
+* [C6] (one) commit one
+* [C2] (origin/two) commit two
+* [C3] commit one

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_tree(
            repo.rev_parse_single("three")?
                .object()?
                .peel_to_tree()?
                .id()
        )
        .to_string(),
        snapbox::str![[r#"
f2ff419
└── .gitignore:100644:f4ec724 "/remote/\n"

"#]]
        .raw()
    );
    assert_worktree_file(&repo, "one.txt", "foo\n");
    assert_worktree_file(&repo, "two.txt", "foo\n");
    assert_worktree_file(&repo, "three.txt", "foo\n");

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_removes_multiple_changes_from_one_source() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let one_id = repo.rev_parse_single("one")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // A single source carries several changes for the same commit.
    let outcome = uncommit_changes_from_commits(
        editor,
        [source_with_changes(one_id, &[".gitignore", "one.txt"])],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "all changes in the source should be uncommitted: {:?}",
        outcome.failures
    );

    let rebase = outcome
        .rebase
        .expect("valid sources should produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-* [C1] (HEAD -> three) commit three
-* [C2] (origin/two, two) commit two
-* [C3] (one) commit one
+++ after
+* [C4] (HEAD -> three) commit three
+* [C5] (two) commit two
+* [C6] (one) commit one
+* [C2] (origin/two) commit two
+* [C3] commit one

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_tree(repo.rev_parse_single("one")?.object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
4b825dc

"#]]
    );
    assert_worktree_file(&repo, ".gitignore", "/remote/\n");
    assert_worktree_file(&repo, "one.txt", "foo\n");

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_groups_duplicate_commit_ids() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let one_id = repo.rev_parse_single("one")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Two separate sources point at the same commit and must be grouped so that
    // both changes are removed in a single tree replacement.
    let outcome = uncommit_changes_from_commits(
        editor,
        [source(one_id, ".gitignore"), source(one_id, "one.txt")],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "duplicate commit ids should be grouped and uncommitted: {:?}",
        outcome.failures
    );

    let rebase = outcome
        .rebase
        .expect("valid sources should produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-* [C1] (HEAD -> three) commit three
-* [C2] (origin/two, two) commit two
-* [C3] (one) commit one
+++ after
+* [C4] (HEAD -> three) commit three
+* [C5] (two) commit two
+* [C6] (one) commit one
+* [C2] (origin/two) commit two
+* [C3] commit one

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_tree(repo.rev_parse_single("one")?.object()?.peel_to_tree()?.id()).to_string(),
        snapbox::str![[r#"
4b825dc

"#]]
    );
    assert_worktree_file(&repo, ".gitignore", "/remote/\n");
    assert_worktree_file(&repo, "one.txt", "foo\n");

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_reports_failures_and_materializes_successes() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let one_id = repo.rev_parse_single("one")?.detach();
    let three_id = repo.rev_parse_single("three")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = uncommit_changes_from_commits(
        editor,
        [
            source(one_id, "does-not-exist.txt"),
            source(three_id, "three.txt"),
        ],
        0,
    )?;

    assert_eq!(outcome.failures.len(), 1);
    assert_eq!(outcome.failures[0].commit_id, one_id);
    assert!(
        outcome.failures[0]
            .error
            .contains("Failed to remove specified changes"),
        "failure should explain why the grouped source was skipped: {:?}",
        outcome.failures[0]
    );

    let rebase = outcome
        .rebase
        .expect("one successful source should still produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-* [C1] (HEAD -> three) commit three
-* [C2] (origin/two, two) commit two
-* [C3] (one) commit one
+++ after
+* [C4] (HEAD -> three) commit three
+* [C2] (origin/two, two) commit two
+* [C3] (one) commit one

"#]]
    );

    snapbox::assert_data_eq!(
        visualize_tree(
            repo.rev_parse_single("three")?
                .object()?
                .peel_to_tree()?
                .id()
        )
        .to_string(),
        snapbox::str![[r#"
aac5238
├── .gitignore:100644:f4ec724 "/remote/\n"
├── one.txt:100644:257cc56 "foo\n"
└── two.txt:100644:257cc56 "foo\n"

"#]]
        .raw()
    );
    assert_worktree_file(&repo, "three.txt", "foo\n");

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_all_failures_does_not_rebase() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;

    let one_id = repo.rev_parse_single("one")?.detach();
    let three_before = repo.rev_parse_single("three")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = uncommit_changes_from_commits(
        editor,
        [
            source(one_id, "does-not-exist.txt"),
            source(three_before, "also-missing.txt"),
        ],
        0,
    )?;

    assert_eq!(outcome.failures.len(), 2);
    assert!(
        outcome.rebase.is_none(),
        "all failed sources should avoid producing a rebase"
    );
    assert_eq!(
        repo.rev_parse_single("three")?.detach(),
        three_before,
        "refs should be unchanged when there is no rebase to materialize"
    );

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_can_uncommit_selected_lines() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("uncommit-lines-from-file", |_| {})?;

    let branch_id = repo.rev_parse_single("branch")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = uncommit_changes_from_commits(
        editor,
        [source_with_hunks(
            branch_id,
            "story.txt",
            vec![hunk_header("-3,0", "+4,2")],
        )],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "selected lines should uncommit cleanly: {:?}",
        outcome.failures
    );

    let rebase = outcome
        .rebase
        .expect("valid selected lines should produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-* [C1] (HEAD -> branch) edit story
-* [C2] (main) base story
+++ after
+* [C3] (HEAD -> branch) edit story
+* [C2] (main) base story

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_tree(
            repo.rev_parse_single("branch")?
                .object()?
                .peel_to_tree()?
                .id()
        )
        .to_string(),
        snapbox::str![[r#"
12a9d5c
└── story.txt:100644:35f45fd "base-1\nbase-2\nkeep-1\nkeep-2\n"

"#]]
        .raw()
    );
    assert_worktree_file(
        &repo,
        "story.txt",
        "base-1\nbase-2\nkeep-1\ndrop-1\ndrop-2\nkeep-2\n",
    );

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_merges_multiple_specs_for_same_file() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("uncommit-two-hunks-from-file", |_| {})?;

    let branch_id = repo.rev_parse_single("branch")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // Two separate sources target the same file in the same commit, one per hunk.
    // They must be merged into a single spec so both hunks are removed; without
    // merging, the tree rebuild would keep only the last spec's change.
    let outcome = uncommit_changes_from_commits(
        editor,
        [
            source_with_hunks(branch_id, "story.txt", vec![hunk_header("-2,1", "+2,1")]),
            source_with_hunks(branch_id, "story.txt", vec![hunk_header("-8,1", "+8,1")]),
        ],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "both hunks for the same file should uncommit cleanly: {:?}",
        outcome.failures
    );

    let rebase = outcome
        .rebase
        .expect("valid sources should produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-* [C1] (HEAD -> branch) edit story
-* [C2] (main) base story
+++ after
+* [C3] (HEAD -> branch) edit story
+* [C2] (main) base story

"#]]
    );

    // Both edits were removed from the commit, so its tree matches the base story.
    let committed = repo
        .rev_parse_single("branch:story.txt")?
        .object()?
        .data
        .clone();
    assert_eq!(
        String::from_utf8(committed)?,
        "line-1\nline-2\nline-3\nline-4\nline-5\nline-6\nline-7\nline-8\nline-9\n"
    );
    // The worktree keeps both edits as uncommitted changes.
    assert_worktree_file(
        &repo,
        "story.txt",
        "line-1\nEDIT-2\nline-3\nline-4\nline-5\nline-6\nline-7\nEDIT-8\nline-9\n",
    );

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_whole_file_spec_supersedes_hunk_specs() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("uncommit-two-hunks-from-file", |_| {})?;

    let branch_id = repo.rev_parse_single("branch")?.detach();

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    // A single source mixes a whole-file spec (empty hunks) with a hunk spec for
    // the same file. The whole-file spec must win, removing every change rather
    // than just the named hunk.
    let outcome = uncommit_changes_from_commits(
        editor,
        [UncommitChangesSource {
            commit_id: branch_id,
            changes: vec![
                diff_spec_for_file("story.txt"),
                DiffSpec {
                    previous_path: None,
                    path: "story.txt".into(),
                    hunk_headers: vec![hunk_header("-2,1", "+2,1")],
                },
            ],
        }],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "whole-file supersede should uncommit cleanly: {:?}",
        outcome.failures
    );

    outcome
        .rebase
        .expect("valid sources should produce a rebase")
        .materialize_without_checkout()?;

    // The whole file was reverted, including the hunk not named by any spec.
    let committed = repo
        .rev_parse_single("branch:story.txt")?
        .object()?
        .data
        .clone();
    assert_eq!(
        String::from_utf8(committed)?,
        "line-1\nline-2\nline-3\nline-4\nline-5\nline-6\nline-7\nline-8\nline-9\n"
    );

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_handles_parallel_stacks() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("uncommit-from-parallel-stacks", |_| {})?;

    let stack_a_id = repo.rev_parse_single("stack-a")?.detach();
    let stack_b_id = repo.rev_parse_single("stack-b")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = uncommit_changes_from_commits(
        editor,
        [source(stack_b_id, "b.txt"), source(stack_a_id, "a.txt")],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "parallel stack sources should uncommit cleanly: {:?}",
        outcome.failures
    );

    let rebase = outcome
        .rebase
        .expect("valid parallel stack sources should produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-*-.   [C1] (HEAD -> gitbutler/workspace) GitButler Workspace Commit
-|\ \
-| | * [C2] (stack-b) stack B adds file
-| |/
-|/|
-| * [C3] (stack-a) stack A adds file
-|/
-* [C4] (main) base
+++ after
+*-.   [C5] (HEAD -> gitbutler/workspace) GitButler Workspace Commit
+|\ \
+| | * [C6] (stack-b) stack B adds file
+| |/
+|/|
+| * [C7] (stack-a) stack A adds file
+|/
+* [C4] (main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        visualize_tree(
            repo.rev_parse_single("gitbutler/workspace")?
                .object()?
                .peel_to_tree()?
                .id()
        )
        .to_string(),
        snapbox::str![[r#"
4b36dfd
└── base.txt:100644:df967b9 "base\n"

"#]]
        .raw()
    );
    assert_worktree_file(&repo, "a.txt", "a\n");
    assert_worktree_file(&repo, "b.txt", "b\n");

    Ok(())
}

#[test]
fn uncommit_changes_from_commits_handles_unordered_overwrites_of_same_file() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("uncommit-overwritten-file-three-commits", |_| {})?;

    let one_id = repo.rev_parse_single("branch~2")?.detach();
    let two_id = repo.rev_parse_single("branch~1")?.detach();
    let three_id = repo.rev_parse_single("branch")?.detach();
    let graph_before = visualize_commit_graph_all(&repo)?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let outcome = uncommit_changes_from_commits(
        editor,
        [
            source(one_id, "file.txt"),
            source(three_id, "file.txt"),
            source(two_id, "file.txt"),
        ],
        0,
    )?;

    assert!(
        outcome.failures.is_empty(),
        "unordered overwrites should be sorted child-to-parent: {:?}",
        outcome.failures
    );

    let rebase = outcome
        .rebase
        .expect("valid overwrite sources should produce a rebase");
    rebase.materialize_without_checkout()?;
    let graph_after = visualize_commit_graph_all(&repo)?;

    snapbox::assert_data_eq!(
        graph_diff(&graph_before, &graph_after),
        snapbox::str![[r#"
--- before
-* [C1] (HEAD -> branch) write three
-* [C2] write two
-* [C3] write one
-* [C4] (main) base
+++ after
+* [C5] (HEAD -> branch) write three
+* [C6] write two
+* [C7] write one
+* [C4] (main) base

"#]]
    );
    snapbox::assert_data_eq!(
        visualize_tree(
            repo.rev_parse_single("branch")?
                .object()?
                .peel_to_tree()?
                .id()
        )
        .to_string(),
        snapbox::str![[r#"
7bee507
└── file.txt:100644:df967b9 "base\n"

"#]]
        .raw()
    );
    assert_worktree_file(&repo, "file.txt", "three\n");

    Ok(())
}

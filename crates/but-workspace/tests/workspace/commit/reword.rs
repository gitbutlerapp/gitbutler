use anyhow::Result;
use but_rebase::graph_rebase::Editor;
use but_testsupport::{cat_commit, visualize_commit_graph_all};
use but_workspace::commit::reword;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn reword_head_commit() -> Result<()> {
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

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("three")?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bee1f03 (HEAD -> three) New name
* 16fd221 (origin/two, two) commit two
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_middle_commit() -> Result<()> {
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

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("two")?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize(Default::default())?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 555bf78 (HEAD -> three) commit three
* 5608218 (two) New name
| * 16fd221 (origin/two) commit two
|/  
* 8b426d0 (one) commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_conflicted_commit_keeps_conflict_markers() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        writable_scenario("with-conflict-marked-message", |_| {})?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 7c77c78 (HEAD -> main, tag: conflicted) [conflict] GitButler WIP Commit
* a047f81 (tag: normal) init

"#]]
    );

    let id = repo.rev_parse_single("conflicted")?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize(Default::default())?;

    // Prefix and trailer are re-applied, so the commit still reads as conflicted.
    snapbox::assert_data_eq!(
        cat_commit(&repo, "main")?,
        snapbox::str![[r#"
tree c986d61715fc89d762de9e07087f6afc621fa4af
parent a047f8183ba2bb7eb00ef89e60050c5fde740483
author GitButler <gitbutler@gitbutler.com> 1730625617 +0100
committer Committer (Memory Override) <committer@example.com> 946771200 +0000
gitbutler-headers-version 2
change-id 0f74c342-1cd3-4408-b965-6c2dfac89857

[conflict] New name

GitButler-Conflict: This is a GitButler-managed conflicted commit. Files are auto-resolved
   using the "ours" side. The commit tree contains additional directories:
     .conflict-side-0  — our tree
     .conflict-side-1  — their tree
     .conflict-base-0  — the merge base tree
     .auto-resolution  — the auto-resolved tree
     .conflict-files   — metadata about conflicted files
   To manually resolve, check out this commit, remove the directories
   listed above, resolve the conflicts, and amend the commit.

"#]]
    );

    let commit = but_core::Commit::from_id(repo.rev_parse_single("main")?)?;
    assert!(
        commit.is_conflicted(),
        "rewording must not drop the conflicted state"
    );

    Ok(())
}

#[test]
fn reword_base_commit() -> Result<()> {
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

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("one")?;
    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize(Default::default())?;

    // We end up with two divergent histories here. This is to be expected if we
    // rewrite the very bottom commit in a repository.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 93151df (HEAD -> three) commit three
* fc0e8de (two) commit two
* f1db5b0 (one) New name
* 16fd221 (origin/two) commit two
* 8b426d0 commit one

"#]]
    );

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

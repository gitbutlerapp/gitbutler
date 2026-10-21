//! Exercises the step option for whether a step should be allowed to enter a conflicted state.

use anyhow::Result;
use but_graph::Graph;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{Editor, Step, mutate::InsertSide},
};
use but_testsupport::{cat_commit, git_status, graph_tree, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn by_default_conflicts_are_allowed() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits-one-file")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f37690f (HEAD -> main, c) c
* 3b3bd41 (b) b
* 5e0ba46 (a) a
* 6155f21 (base) base

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Replacing b with none will cause c to conflict
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace_with_none(b_sel)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    ├── ·04d1892 (⌂) ►c
    └── ·5e0ba46 (⌂) ►a, ►b
        └── ►:1[1]:base
            └── 🏁·6155f21 (⌂)

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    // Unlike the overlay, the refreshed graph sees the padding commit that was checked out on top of c.
    snapbox::assert_data_eq!(
        graph_tree(&outcome.workspace.graph).to_string(),
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    └── ·45942da (⌂)
        └── ►:1[1]:c
            ├── ·04d1892 (⌂)
            └── ·5e0ba46 (⌂) ►a, ►b
                └── ►:2[2]:base
                    └── 🏁·6155f21 (⌂)

"#]]
    );

    // The conflicted c can't be checked out, so main moves to a padding commit on top of it.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 45942da (HEAD -> main) GitButler Padding Commit
* 04d1892 (c) [conflict] c
* 5e0ba46 (b, a) a
* 6155f21 (base) base

"#]]
    );
    assert_eq!(
        repo.head_name()?,
        Some("refs/heads/main".try_into()?),
        "HEAD stays attached to the branch that now points at the padding commit"
    );
    // The padding commit carries c's auto-resolution and is recognisable by its title.
    snapbox::assert_data_eq!(
        cat_commit(&repo, "main")?,
        snapbox::str![[r#"
tree aaff74984cccd156a469afa7d9ab10e4777beb24
parent 04d1892b53c16cf49385c95206d8105dbe88daf5
author author <author@example.com> 946684800 +0000
committer Committer (Memory Override) <committer@example.com> 946771200 +0000

GitButler Padding Commit

Hi! I'm a stand-in for the conflicted commit below me, so your editor shows its
auto-resolved changes instead of conflict markers.

There's nothing to do with me. GitButler takes me out again once you resolve the
conflict, and anything you commit on top of me moves onto the conflicted commit.

"#]]
    );
    // The worktree is exactly the padding commit.
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    // We expect to see conflicted headers
    snapbox::assert_data_eq!(
        cat_commit(&repo, "c")?,
        snapbox::str![[r#"
tree d935c009623a61ebde94512baef22c76c5ccbaef
parent 5e0ba4636be91de6216903697b269915d3db6c53
author author <author@example.com> 946684800 +0000
committer Committer (Memory Override) <committer@example.com> 946771200 +0000
gitbutler-headers-version 2
change-id ulsktwtuywxwowluzmmxlqrqnyxktwuz

[conflict] c

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

    Ok(())
}

#[test]
fn if_a_commit_has_been_configured_not_to_conflict_but_ends_up_conflicted_an_error_is_raised()
-> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits-one-file")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f37690f (HEAD -> main, c) c
* 3b3bd41 (b) b
* 5e0ba46 (a) a
* 6155f21 (base) base

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Replacing b with none will cause c to conflict
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace_with_none(b_sel)?;

    // Set c to disallow conflicts
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.update_pick(c_sel, |_editor, mut c_pick| {
        c_pick.conflictable = false;
        Ok(c_pick)
    })?;

    // We should see an error given saying C ended up being conflicted
    snapbox::assert_data_eq!(
        editor.rebase().to_debug(),
        snapbox::str![[r#"
Err(
    "Commit f37690fa0ac6f48391974bb0a7cdc4c8a6c6fe7a was marked as not conflictable, but resulted in a conflicted state",
)

"#]]
    );

    Ok(())
}

#[test]
fn if_a_commit_has_been_configured_not_to_conflict_and_doesnt_end_up_conflicted_result_is_ok()
-> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits-one-file")?;

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* f37690f (HEAD -> main, c) c
* 3b3bd41 (b) b
* 5e0ba46 (a) a
* 6155f21 (base) base

"#]]
    );

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;

    // Insert an empty commit above b to cause c to get cherry picked with out a conflict
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    let mut empty = editor.empty_commit()?;
    empty.message = b"I'm a new commit! Hello there".into();
    let empty_id = editor.new_commit(empty, DateMode::CommitterKeepAuthorKeep)?;
    editor.insert(b_sel, Step::new_pick(empty_id), InsertSide::Above)?;

    // Set c to disallow conflicts
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    editor.update_pick(c_sel, |_editor, mut c_pick| {
        c_pick.conflictable = false;
        Ok(c_pick)
    })?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    snapbox::assert_data_eq!(
        &overlayed,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    └── ·df438f2 (⌂) ►c
        └── ►:1[1]:b
            ├── ·0c6475b (⌂)
            └── ·3b3bd41 (⌂)
                └── ►:2[2]:a
                    └── ·5e0ba46 (⌂)
                        └── ►:3[3]:base
                            └── 🏁·6155f21 (⌂)

"#]]
    );
    let outcome = outcome.materialize(Default::default())?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    // The rebase is successful because `c` remained unconflicted
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* df438f2 (HEAD -> main, c) c
* 0c6475b (b) I'm a new commit! Hello there
* 3b3bd41 b
* 5e0ba46 (a) a
* 6155f21 (base) base

"#]]
    );

    Ok(())
}

#[test]
fn a_padded_tip_stays_until_its_conflict_is_gone() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits-one-file")?;
    let mut ws = workspace(&repo, &*meta, &mut db)?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let b = editor.select_commit(repo.rev_parse_single("b")?.detach())?;
    editor.replace_with_none(b)?;
    editor.rebase()?.materialize(Default::default())?;
    let padded = repo.rev_parse_single("main")?.detach();

    let mut ws = workspace(&repo, &*meta, &mut db)?;
    let outcome = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?.rebase()?;
    assert!(
        !outcome.references_updated()?,
        "main already points at the padding commit a checkout of the conflicted c lands on"
    );
    outcome.materialize(Default::default())?;
    assert_eq!(
        repo.rev_parse_single("main")?,
        padded,
        "an edit that changes nothing keeps the padding commit"
    );

    let mut ws = workspace(&repo, &*meta, &mut db)?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let c = editor.select_commit(repo.rev_parse_single("c")?.detach())?;
    editor.replace_with_none(c)?;
    editor.rebase()?.materialize(Default::default())?;
    // Without the conflicted c, main lands on the clean a and the padding commit is gone.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 5e0ba46 (HEAD -> main, c, b, a) a
* 6155f21 (base) base

"#]]
    );
    // The worktree is exactly a.
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn without_checkout_a_conflicted_tip_is_padded_too() -> Result<()> {
    let (repo, _tmpdir, mut meta, mut db) = fixture_writable("four-commits-one-file")?;
    let mut ws = workspace(&repo, &*meta, &mut db)?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo, &mut db)?;
    let b = editor.select_commit(repo.rev_parse_single("b")?.detach())?;
    editor.replace_with_none(b)?;
    editor.rebase()?.materialize_without_checkout()?;

    // Ref edits are padded regardless of the checkout option, so main moves to the padding commit all the same.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 45942da (HEAD -> main) GitButler Padding Commit
* 04d1892 (c) [conflict] c
* 5e0ba46 (b, a) a
* 6155f21 (base) base

"#]]
    );

    Ok(())
}

fn workspace(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    db: &mut but_db::DbHandle,
) -> Result<but_graph::Workspace> {
    Graph::from_head(
        repo,
        meta,
        but_core::ref_metadata::ProjectMeta::default(),
        db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()
}

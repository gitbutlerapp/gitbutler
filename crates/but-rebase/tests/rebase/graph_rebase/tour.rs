//! # The guided tour, continued: mutation
//!
//! The pipeline tour (`but-graph`'s `tests/graph/tour.rs`) ends where the editor
//! begins. Two chapters, the two halves of the model's one separation: **inserting a
//! new branch between a commit and an existing branch pointing at it** (a pure ref
//! motion — rewrites nothing), and **inserting a new commit between a commit and the
//! branch pointing at it** (a graph motion — the branch follows without anyone
//! updating it). The second is the narrative exhibit of `refs-as-positions.md`,
//! executable.
//!
//! Under refs-as-nodes, both need edge surgery and the ref's movement is a side
//! effect nobody states. Here a reference is a POSITION — refs on one commit form an
//! ordered group — so each operation is one named call, and the ancestry is
//! untouched by construction: commit surgery *cannot* move a ref, and ref surgery
//! *cannot* rewrite a commit. The measured argument: `refs-as-positions.md`.

use anyhow::Result;
use but_core::Commit;
use but_graph::Workspace;
use but_rebase::graph_rebase::{CommitSpec, Editor, mutate::InsertSide, testing::Testing as _};
use but_testsupport::{graph_dag, visualize_commit_graph_all};
use gix::prelude::ObjectIdExt;
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn insert_a_branch_between_a_commit_and_its_branch() -> Result<()> {
    // Before: `A` points at `add59d2`. We want a new branch `A-base` BETWEEN that
    // commit and `A` — same commit, one rank below in the group — the way a user
    // splits work off an existing branch without touching history.
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e8ee978 (HEAD -> with-inner-merge) on top of inner merge
*   2fc288c Merge branch 'B' into with-inner-merge
|\  
| * 984fd1c (B) C: new file with 10 lines
* | add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;

    // The whole mutation. `InsertSide` is the one decision the model demands of the
    // caller — which side of the group they mean: BELOW a reference takes its
    // position, shifting it and everything above one rank up. (The other three arms
    // of the (side × is-the-target-a-reference) table are one-sentence rules too.)
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    let a = editor.select_reference("refs/heads/A".try_into()?)?;
    editor.insert_reference(a, "refs/heads/A-base".try_into()?, InsertSide::Below)?;

    // The rewrite is computed, then previewed WITHOUT touching the repository: the
    // outcome's overlay re-runs the same derivation the tour walked, over unwritten
    // state. `A-base` stands on `add59d2`, and every commit id is unchanged.
    let outcome = editor.rebase()?;
    let preview_ws = ws.rederive_with(outcome.repo(), outcome.meta(), outcome.overlay()?)?;
    let preview = graph_dag(&preview_ws);
    snapbox::assert_data_eq!(
        preview.as_str(),
        snapbox::str![[r#"
*  👉·e8ee978 (⌂) ►with-inner-merge[🌳]
*    ·2fc288c (⌂)
├─╮
* │  ·add59d2 (⌂) ►A, ►A-base
│ *  ·984fd1c (⌂) ►B
├─╯
*  🏁·8f0d338 (⌂) ►main, ►tags/base
"#]]
    );

    // "Between" is a GROUP-ORDER fact: refs on one commit form an ORDERED group, and
    // the stored layout answers it directly — the ref directly below `A` is now the
    // new branch. (The arena's per-commit ref list is name-sorted display data; order
    // is the layout's fact.)
    let layout = preview_ws
        .commit_graph()
        .layout()
        .expect("built graphs carry a layout");
    assert_eq!(
        layout
            .below_of("refs/heads/A".try_into()?)
            .map(|n| n.as_bstr()),
        Some("refs/heads/A-base".into()),
        "the new branch stands between the commit and `A`"
    );

    // The punchline, asserted: a pure ref motion REWRITES NOTHING. No commit was
    // replayed, no object written — a reference is a name plus a place, exactly what
    // git stores.
    assert_eq!(
        outcome.commit_mappings().len(),
        0,
        "inserting a reference rewrites no commit at all"
    );

    // Materializing writes the ref and nothing else; git agrees with the preview.
    let (graph, _meta) = outcome.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &*meta, &mut but_testsupport::in_memory_db())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* e8ee978 (HEAD -> with-inner-merge) on top of inner merge
*   2fc288c Merge branch 'B' into with-inner-merge
|\  
| * 984fd1c (B) C: new file with 10 lines
* | add59d2 (A-base, A) A: 10 lines on top
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn insert_a_commit_between_a_commit_and_its_branch() -> Result<()> {
    // The narrative exhibit of `refs-as-positions.md`, executable. The merge `2fc288c`
    // has parent `add59d2`, and branch `A` points at that parent. Insert a new empty
    // commit N between the commit and the branch tip — spelled: target the COMMIT,
    // `Above`. The question the exhibit asks: who moves `A`?
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    let c = repo.rev_parse_single("A")?.detach();
    let c_handle = editor.select_commit(c)?;

    // The new empty commit, authored with the fixture commit's dates so every id
    // below is deterministic.
    let mut commit = Commit::from_id(c.attach(&repo))?;
    commit.message = "N (interposed)".into();
    commit.parents = vec![].into();
    let n = repo.write_object(commit.inner)?.detach();

    // The whole mutation. The (Above × commit) arm's one-sentence rule: children
    // rewire with parent numbers preserved, and every ref sitting on the commit moves
    // up — `A`'s lift onto N is a stated call (`reposition_refs`), not a side effect
    // of edge surgery.
    editor.insert_commit(c_handle, CommitSpec::new(n), InsertSide::Above)?;

    // `A` already renders on N, before any object is rewritten.
    snapbox::assert_data_eq!(
        editor
            .steps_ascii()
            .replace(&n.to_hex_with_len(7).to_string(), "[ n ]"),
        snapbox::str![[r#"
◎  refs/heads/with-inner-merge
●  e8ee978 on top of inner merge
●    2fc288c Merge branch 'B' into with-inner-merge
├─╮
◎ │  refs/heads/A
● │  [ n ] N (interposed)
● │  add59d2 A: 10 lines on top
│ ◎  refs/heads/B
│ ●  984fd1c C: new file with 10 lines
├─╯
◎  refs/heads/main
◎  refs/tags/base (immutable)
●  8f0d338 base
"#]]
    );

    // Replay. Ids are rewritten IN PLACE under the positions — N itself and the two
    // commits above the insertion — so a position keeps naming the right entry while
    // the id underneath it changes.
    let outcome = editor.rebase()?;
    assert_eq!(
        outcome.commit_mappings().len(),
        3,
        "N plus the merge and the tip above it are rewritten; nothing below moves"
    );

    // Materializing writes commits AND the derived ref transaction. Nobody wrote
    // "update A": its target is derived from its position — the answer to the
    // exhibit's question is that no one moves `A`, and no one can forget to.
    let (graph, _meta) = outcome.materialize()?;
    drop(graph);
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a469f8a (HEAD -> with-inner-merge) on top of inner merge
*   40168e4 Merge branch 'B' into with-inner-merge
|\  
| * 984fd1c (B) C: new file with 10 lines
* | 1283fae (A) N (interposed)
* | add59d2 A: 10 lines on top
|/  
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );
    Ok(())
}

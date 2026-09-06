//! The rider rules of `move_range`: an ordinary reference on a moved commit stays
//! behind in the lineage, while a WORKTREE's checked-out branch follows the commit its
//! worktree stands on — regression coverage for the worktree-move fix.
use anyhow::Result;
use but_graph::Workspace;
use but_rebase::graph_rebase::{
    Editor,
    anchor::{Anchor, Connect, Cut, Range},
    mutate::{InsertSide, Reconnect},
};
use but_testsupport::visualize_commit_graph_all;
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

#[test]
fn ref_on_moved_commit_rides_it() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;
    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;
    // `B` stands on 984fd1c. Move that commit above `main`; `B` must ride.
    let b_commit = editor.select_commit(repo.rev_parse_single("refs/heads/B")?.detach())?;
    editor.move_range(
        Range::single(b_commit),
        Cut::All,
        Anchor::Reference("refs/heads/main".try_into()?),
        InsertSide::Above,
        Connect::Splice,
        Reconnect::Heal,
    )?;
    let outcome = editor.rebase()?;
    let (graph, _meta) = outcome.materialize()?;
    ws.refresh_from_commit_graph(graph, &repo, &*meta, &mut but_testsupport::in_memory_db())?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 71d231b (HEAD -> with-inner-merge) on top of inner merge
*   fec54ae Merge branch 'B' into with-inner-merge
|\  
* | 131f4a5 (A) A: 10 lines on top
|/  
* 984fd1c (B) C: new file with 10 lines
* 8f0d338 (tag: base, main) base

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn worktree_ref_on_moved_commit_rides_it() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("worktree-move-mixed")?;
    let mut options = standard_options();
    for (name, branch) in [("wt", "feat"), ("other", "other")] {
        options.worktree_tips.push(but_graph::walk::WorktreeTip {
            name: name.into(),
            ref_name: Some(format!("refs/heads/{branch}").try_into()?),
            id: repo.find_reference(branch)?.peel_to_id()?.detach(),
        });
    }
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        Default::default(),
        &mut but_testsupport::in_memory_db(),
        options,
    )?
    .validated()?;
    let mut editor = but_rebase::graph_rebase::Editor::create_with_opts(
        ws.commit_graph(),
        ws.project_meta(),
        &mut *meta,
        &repo,
        &but_rebase::graph_rebase::EditorStoreOptions {
            worktree_tips: ws.options().worktree_tips.clone(),
            ..Default::default()
        },
    )?;
    let feat = editor.select_commit(repo.rev_parse_single("feat")?.detach())?;
    editor.move_range(
        Range::single(feat),
        Cut::All,
        Anchor::Reference("refs/heads/other".try_into()?),
        InsertSide::Below,
        Connect::Splice,
        Reconnect::Heal,
    )?;
    let outcome = editor.rebase()?;
    outcome.materialize()?;
    // `other` is checked out in a linked worktree, so it forks directly onto its commit:
    // the moved commit lands under `other` alone, and the lane that shared the base with
    // it is not rebased.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 57d0038 (other, feat) worktree source
| * baa5a4c (HEAD -> main) workspace source
| * 4119f49 (stack-base) stack base
|/  
* 35b8235 (target, stable) base

"#]]
        .raw()
    );
    Ok(())
}

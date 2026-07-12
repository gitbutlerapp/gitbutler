//! These tests exercise [`Editor::insert_range`] directly. They live in-crate because
//! it is editor plumbing: `move_range` absorbed every public caller, so only unit
//! tests may still address the bare insert.
use anyhow::{Context, Result};
use bstr::ByteSlice;
use but_graph::Workspace;
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::{git_status, graph_dag, visualize_commit_graph, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::graph_rebase::{Editor, anchor, mutate};

/// A fixture that may be written to (the tests-tree `utils::fixture_writable`).
fn fixture_writable(
    fixture_name: &str,
) -> Result<(
    gix::Repository,
    tempfile::TempDir,
    std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
)> {
    let (repo, tmp) = but_testsupport::writable_scenario(fixture_name);
    let meta = VirtualBranchesTomlMetadata::from_path(
        repo.path()
            .join(".git")
            .join("should-never-be-written.toml"),
    )?;
    Ok((repo, tmp, std::mem::ManuallyDrop::new(meta)))
}

fn standard_options() -> but_graph::walk::Options {
    but_graph::walk::Options {
        collect_tags: true,
        commits_limit_hint: None,
        commits_limit_recharge_location: vec![],
        hard_limit: None,
        extra_target_commit_id: None,
        worktree_tips: vec![],
    }
}

fn parent_subjects(repo: &gix::Repository, rev: &str) -> Result<Vec<String>> {
    let commit = repo.rev_parse_single(rev)?.object()?.peel_to_commit()?;
    commit
        .parent_ids()
        .map(|parent_id| {
            let parent = parent_id.object()?.peel_to_commit()?;
            let subject = parent
                .message_raw()?
                .as_bstr()
                .lines()
                .next()
                .unwrap_or_default()
                .to_str_lossy()
                .into_owned();
            Ok(subject)
        })
        .collect()
}
#[test]
fn insert_single_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_handle = editor
        .select_commit(a)
        .context("Failed to find commit a in the editor store")?;
    let b = "refs/heads/B".try_into()?;
    let b_handle = editor
        .select_reference(b)
        .context("Failed to find reference b in the editor store")?;

    let range = anchor::Range {
        child: a_handle.into(),
        parent: a_handle.into(),
    };

    editor.insert_range(
        b_handle,
        range,
        mutate::InsertSide::Above,
        anchor::Connect::Splice,
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.rederive_with(outcome.repo(), outcome.meta(), outcome.overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*    👉·ee7f107 (⌂) ►main[🌳]
├─╮
* │    ·69221b4 (⌂) ►A
├───╮
* │ │  ·a748762 (⌂) ►B
* │ │  ·62e05ba (⌂)
├───╯
│ *  ·930563a (⌂) ►C
│ *  ·68a2fc3 (⌂)
│ *  ·984fd1c (⌂)
├─╯
*  🏁·8f0d338 (⌂) ►tags/base
"#]]
    );
    let (outcome, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome, &repo, &*meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   ee7f107 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\  
| * 930563a (C) C: add another 10 lines to new file
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
* | 69221b4 (A) A: 10 lines on top
|\| 
* | a748762 (B) B: another 10 lines at the bottom
* | 62e05ba B: 10 lines at the bottom
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}
#[test]
fn insert_single_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_handle = editor
        .select_commit(a)
        .context("Failed to find commit a in the editor store")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_handle = editor
        .select_commit(b)
        .context("Failed to find commit b in the editor store")?;

    let range = anchor::Range {
        child: a_handle.into(),
        parent: a_handle.into(),
    };

    editor.insert_range(
        b_handle,
        range,
        mutate::InsertSide::Below,
        anchor::Connect::Splice,
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.rederive_with(outcome.repo(), outcome.meta(), outcome.overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*      👉·b005f3c (⌂) ►main[🌳]
├─┬─╮
│ * │  ·a3301fe (⌂) ►B
├─╯ │
*   │  ·7f0cc55 (⌂) ►A
├─╮ │
* │ │  ·62e05ba (⌂)
├─╯ │
│   *  ·930563a (⌂) ►C
│   *  ·68a2fc3 (⌂)
│   *  ·984fd1c (⌂)
├───╯
*  🏁·8f0d338 (⌂) ►tags/base
"#]]
    );
    let (outcome, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome, &repo, &*meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   b005f3c (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a3301fe (B) B: another 10 lines at the bottom
|/ /  
* | 7f0cc55 (A) A: 10 lines on top
|\| 
* | 62e05ba B: 10 lines at the bottom
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}
#[test]
fn insert_multi_node_segment_above() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_handle = editor
        .select_commit(a)
        .context("Failed to find commit a in the editor store")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_handle = editor
        .select_commit(b)
        .context("Failed to find commit b in the editor store")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_handle = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in the editor store")?;

    let range = anchor::Range {
        child: b_handle.into(),
        parent: b_parent_handle.into(),
    };

    editor.insert_range(
        a_handle,
        range,
        mutate::InsertSide::Above,
        anchor::Connect::Splice,
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.rederive_with(outcome.repo(), outcome.meta(), outcome.overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*    👉·61b2679 (⌂) ►main[🌳]
├─╮
* │  ·758c8a3 (⌂) ►A, ►B
* │    ·db40ffc (⌂)
├───╮
* │ │  ·add59d2 (⌂)
├───╯
│ *  ·930563a (⌂) ►C
│ *  ·68a2fc3 (⌂)
│ *  ·984fd1c (⌂)
├─╯
*  🏁·8f0d338 (⌂) ►tags/base
"#]]
    );
    let (outcome, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome, &repo, &*meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   61b2679 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\  
| * 930563a (C) C: add another 10 lines to new file
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
* | 758c8a3 (B, A) B: another 10 lines at the bottom
* | db40ffc B: 10 lines at the bottom
|\| 
* | add59d2 A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn insert_multi_node_segment_below() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_handle = editor
        .select_commit(a)
        .context("Failed to find commit a in the editor store")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_handle = editor
        .select_commit(b)
        .context("Failed to find commit b in the editor store")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_handle = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in the editor store")?;

    let range = anchor::Range {
        child: b_handle.into(),
        parent: b_parent_handle.into(),
    };

    editor.insert_range(
        a_handle,
        range,
        mutate::InsertSide::Below,
        anchor::Connect::Splice,
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.rederive_with(outcome.repo(), outcome.meta(), outcome.overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*      👉·4db28a9 (⌂) ►main[🌳]
├─┬─╮
* │ │  ·71dfc8f (⌂) ►A
├─╯ │
*   │  ·a748762 (⌂) ►B
*   │  ·62e05ba (⌂)
│   *  ·930563a (⌂) ►C
│   *  ·68a2fc3 (⌂)
│   *  ·984fd1c (⌂)
├───╯
*  🏁·8f0d338 (⌂) ►tags/base
"#]]
    );
    let (outcome, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome, &repo, &*meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   4db28a9 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
* | | 71dfc8f (A) A: 10 lines on top
|/ /  
* | a748762 (B) B: another 10 lines at the bottom
* | 62e05ba B: 10 lines at the bottom
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn insert_single_node_segment_above_with_explicit_children() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_handle = editor
        .select_commit(a)
        .context("Failed to find commit a in the editor store")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_handle = editor
        .select_commit(b)
        .context("Failed to find commit b in the editor store")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_handle = editor
        .select_commit(c)
        .context("Failed to find commit c in the editor store")?;

    let range = anchor::Range {
        child: a_handle.into(),
        parent: a_handle.into(),
    };

    editor.insert_range(
        b_handle,
        range,
        mutate::InsertSide::Above,
        anchor::Connect::only([c_handle]),
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.rederive_with(outcome.repo(), outcome.meta(), outcome.overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*      👉·cca953f (⌂) ►main[🌳]
├─┬─╮
│ │ *  ·76e2160 (⌂) ►C
╭───┤
│ │ *  ·68a2fc3 (⌂)
│ │ *  ·984fd1c (⌂)
* │ │  ·69221b4 (⌂) ►A
╰─┬─╮
  * │  ·a748762 (⌂) ►B
  * │  ·62e05ba (⌂)
  ├─╯
  *  🏁·8f0d338 (⌂) ►tags/base
"#]]
    );
    let (outcome, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome, &repo, &*meta)?;
    assert_eq!(overlayed, graph_dag(&ws));

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   cca953f (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | *   76e2160 (C) C: add another 10 lines to new file
| | |\  
| |_|/  
|/| |   
* | |   69221b4 (A) A: 10 lines on top
|\ \ \  
| |/ /  
|/| |   
* | | a748762 (B) B: another 10 lines at the bottom
* | | 62e05ba B: 10 lines at the bottom
|/ /  
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

#[test]
fn insert_single_node_segment_below_with_explicit_parents() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(&repo, "@")?,
        snapbox::str![[r#"
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/  
* / add59d2 (A) A: 10 lines on top
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    let mut ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_handle = editor
        .select_commit(a)
        .context("Failed to find commit a in the editor store")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_handle = editor
        .select_commit(b)
        .context("Failed to find commit b in the editor store")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_handle = editor
        .select_commit(c)
        .context("Failed to find commit c in the editor store")?;

    let range = anchor::Range {
        child: b_handle.into(),
        parent: b_handle.into(),
    };

    editor.insert_range(
        a_handle,
        range,
        mutate::InsertSide::Below,
        anchor::Connect::only([c_handle]),
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.rederive_with(outcome.repo(), outcome.meta(), outcome.overlay()?)?);
    snapbox::assert_data_eq!(
        overlayed.as_str(),
        snapbox::str![[r#"
*      👉·54f9cab (⌂) ►main[🌳]
├─┬─╮
* │ │  ·9501727 (⌂) ►A
├─╮ │
│ * │  ·347772f (⌂) ►B
│ ├─╮
│ │ *  ·930563a (⌂) ►C
│ │ *  ·68a2fc3 (⌂)
│ │ *  ·984fd1c (⌂)
├───╯
│ *  ·62e05ba (⌂)
├─╯
*  🏁·8f0d338 (⌂) ►tags/base
"#]]
    );
    let (outcome, _) = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome, &repo, &*meta)?;
    assert_eq!(overlayed, graph_dag(&ws));
    assert_eq!(
        parent_subjects(&repo, "B")?,
        [
            "C: add another 10 lines to new file",
            "B: 10 lines at the bottom"
        ]
    );

    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   54f9cab (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \  
* | | 9501727 (A) A: 10 lines on top
|\| | 
| * |   347772f (B) B: another 10 lines at the bottom
| |\ \  
| | |/  
| |/|   
| | * 62e05ba B: 10 lines at the bottom
| |/  
|/|   
| * 930563a (C) C: add another 10 lines to new file
| * 68a2fc3 C: add 10 lines to new file
| * 984fd1c C: new file with 10 lines
|/  
* 8f0d338 (tag: base) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

/// An empty `Connect::Only` would leave the inserted range dangling; the operation refuses
/// it with a message that points at `Connect::Splice`.
#[test]
fn insert_rejects_an_empty_connect() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("four-commits")?;

    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let subject = editor.select_commit(repo.rev_parse_single("@~1")?.detach())?;
    let target = editor.select_commit(repo.rev_parse_single("@")?.detach())?;
    let range = anchor::Range {
        child: subject.into(),
        parent: subject.into(),
    };
    let err = editor
        .insert_range(
            target,
            range,
            mutate::InsertSide::Above,
            anchor::Connect::Only(Vec::new()),
        )
        .expect_err("an empty Only wires nothing");
    assert!(
        err.to_string()
            .contains("use `Connect::Splice` to adopt the target's neighbors"),
        "the error teaches the honest spelling: {err:#}"
    );
    Ok(())
}

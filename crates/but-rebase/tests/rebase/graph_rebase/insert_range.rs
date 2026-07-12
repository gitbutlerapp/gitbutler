//! These tests exercise the insert segment operation.
use anyhow::{Context, Result};
use bstr::ByteSlice;
use but_graph::Workspace;
use but_rebase::graph_rebase::{Editor, mutate, selector};
use but_testsupport::{git_status, graph_dag, visualize_commit_graph, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::utils::{fixture_writable, standard_options};

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
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = "refs/heads/B".try_into()?;
    let b_selector = editor
        .select_reference(b)
        .context("Failed to find reference b in editor graph")?;

    let range = selector::StepRange {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_range(b_selector, range, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
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
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
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
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;

    let range = selector::StepRange {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_range(b_selector, range, mutate::InsertSide::Below)?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
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
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
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
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;

    let range = selector::StepRange {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_range(a_selector, range, mutate::InsertSide::Above)?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
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
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
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
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let b_parent = repo.rev_parse_single("B~")?.detach();
    let b_parent_selector = editor
        .select_commit(b_parent)
        .context("Failed to find parent of commit b in editor graph")?;

    let range = selector::StepRange {
        child: b_selector,
        parent: b_parent_selector,
    };

    editor.insert_range(a_selector, range, mutate::InsertSide::Below)?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
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
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
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
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let range = selector::StepRange {
        child: a_selector,
        parent: a_selector,
    };

    editor.insert_range_into(
        b_selector,
        range,
        mutate::InsertSide::Above,
        Some(selector::SomeSelectors::new(vec![c_selector])?),
        mutate::ParentReparentingOrder::Prepend,
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
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
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
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
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let range = selector::StepRange {
        child: b_selector,
        parent: b_selector,
    };

    editor.insert_range_into(
        a_selector,
        range,
        mutate::InsertSide::Below,
        Some(selector::SomeSelectors::new(vec![c_selector])?),
        mutate::ParentReparentingOrder::Prepend,
    )?;

    let outcome = editor.rebase()?;
    let overlayed =
        graph_dag(&ws.redo(outcome.repo(), outcome.meta(), outcome.rebase_overlay()?)?);
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
    let outcome = outcome.materialize()?;
    ws.refresh_from_commit_graph(outcome.arena().clone(), &repo, outcome.meta)?;
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

#[test]
fn insert_single_node_segment_below_can_append_reparented_parent() -> Result<()> {
    let (repo, _tmp, mut meta) = fixture_writable("three-branches-merged")?;
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;
    let mut editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut *meta, &repo)?;

    let a = repo.rev_parse_single("A")?.detach();
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    let b = repo.rev_parse_single("B")?.detach();
    let b_selector = editor
        .select_commit(b)
        .context("Failed to find commit b in editor graph")?;
    let c = repo.rev_parse_single("C")?.detach();
    let c_selector = editor
        .select_commit(c)
        .context("Failed to find commit c in editor graph")?;

    let range = selector::StepRange {
        child: b_selector,
        parent: b_selector,
    };

    editor.insert_range_into(
        a_selector,
        range,
        mutate::InsertSide::Below,
        Some(selector::SomeSelectors::new(vec![c_selector])?),
        mutate::ParentReparentingOrder::Append,
    )?;

    editor.rebase()?.materialize()?;
    assert_eq!(
        parent_subjects(&repo, "B")?,
        [
            "B: 10 lines at the bottom",
            "C: add another 10 lines to new file"
        ]
    );

    snapbox::assert_data_eq!(git_status(&repo)?, snapbox::str![""]);

    Ok(())
}

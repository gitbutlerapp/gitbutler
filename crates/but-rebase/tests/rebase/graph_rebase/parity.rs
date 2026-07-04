//! The C2 parity corpus: mutate-then-project must equal rewalk-then-project.
//!
//! Every test drives one representative mutation through `rebase()`, then compares the
//! projection of the mutated editor graph against the projection of a fresh editor created
//! from the materialized, re-walked repository (via
//! [`but_rebase::graph_rebase::testing::rewalk_parity_report`]). Divergence here is exactly
//! the gap that stops editor sessions from living directly on the walked graph — the corpus
//! is the collapse's worklist, so new mutation kinds should gain a scenario when they land.

use anyhow::Result;
use but_core::ref_metadata::ProjectMeta;
use but_graph::Workspace;
use but_rebase::graph_rebase::{
    Editor, Step, mutate, mutate::InsertSide, testing::rewalk_parity_report,
};

use crate::utils::{fixture_writable, standard_options};

/// Assert both projections agree, with a readable dump on divergence.
///
/// Both sides are canonicalized first (see [`canonicalize_sibling_order`]) so the harness
/// measures SEMANTIC parity — a reference's presence and which commit it sits on — and not the
/// relative order of co-located siblings, which is a genuine no-clean-spec arbitration (two
/// branches at one commit have no topological order; a fresh walk name-sorts them, a mutation
/// may preserve prior stacking). Canonicalization only reorders within a co-located run, so a
/// reference landing on the WRONG commit — or dropping entirely — still fails loudly.
fn assert_parity(mutated: &str, rewalked: &str) {
    let (cm, cr) = (
        canonicalize_sibling_order(mutated),
        canonicalize_sibling_order(rewalked),
    );
    assert!(
        cm == cr,
        "mutate-then-project != rewalk-then-project (co-located sibling order normalized)\n\n--- mutated editor graph ---\n{mutated}\n\n--- rewalked repository ---\n{rewalked}\n"
    );
}

/// A rendered reference row: `<lane>◎  <name>`, where the lane before the node glyph is only
/// vertical-bar/space fill. Returns `(lane, name)`; `None` for commit rows and lane-only rows.
fn split_ref_row(line: &str) -> Option<(&str, &str)> {
    let idx = line.find('◎')?;
    let (lane, rest) = line.split_at(idx);
    if !lane.chars().all(|c| c == ' ' || c == '│') {
        return None;
    }
    let name = rest.strip_prefix('◎')?.trim_start();
    Some((lane, name))
}

/// Sort each maximal run of consecutive reference rows sharing one lane by refname, leaving all
/// other lines untouched. Co-located siblings render as such a run; ordering it makes the
/// comparison blind to their arbitrary order while preserving everything structural.
fn canonicalize_sibling_order(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((lane, _)) = split_ref_row(lines[i]) else {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        };
        let mut run: Vec<&str> = vec![lines[i]];
        let mut j = i + 1;
        while j < lines.len() {
            match split_ref_row(lines[j]) {
                Some((l2, _)) if l2 == lane => {
                    run.push(lines[j]);
                    j += 1;
                }
                _ => break,
            }
        }
        run.sort_by_key(|line| split_ref_row(line).map(|(_, n)| n).unwrap_or(line));
        out.extend(run.into_iter().map(str::to_string));
        i = j;
    }
    out.join("\n")
}

/// Build a workspace editor for `fixture` with no target.
macro_rules! editor {
    ($fixture:literal, $repo:ident, $tmp:ident, $meta:ident, $ws:ident) => {
        let ($repo, $tmp, mut $meta) = fixture_writable($fixture)?;
        let mut $ws =
            Workspace::from_head(&$repo, &*$meta, ProjectMeta::default(), standard_options())?
                .validated()?;
    };
}

/// The identity mutation: a plain rebase must round-trip through materialize + rewalk.
#[test]
fn noop_rebase() -> Result<()> {
    editor!("workspace-signed", repo, _tmp, meta, ws);
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A new commit inserted above a mid-stack commit.
#[test]
fn insert_pick_above_commit() -> Result<()> {
    editor!("workspace-signed", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let b = repo.rev_parse_single("b")?.detach();
    let mut new_commit = but_core::Commit::from_id(repo.rev_parse_single("b")?)?;
    new_commit.message = "inserted above b".into();
    new_commit.parents = vec![].into();
    let new_id = repo.write_object(new_commit.inner)?.detach();

    let selector = editor.select_commit(b)?;
    editor.insert(selector, Step::new_pick(new_id), InsertSide::Above)?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A new reference created above a mid-stack commit (branch creation).
#[test]
fn insert_reference_above_commit() -> Result<()> {
    editor!("workspace-signed", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let b = repo.rev_parse_single("b")?.detach();
    let selector = editor.select_commit(b)?;
    editor.insert(
        selector,
        Step::new_reference("refs/heads/created-here".try_into()?),
        InsertSide::Above,
    )?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A new commit inserted below a mid-stack commit (exercises the Below-pick parent rewire).
#[test]
fn insert_pick_below_commit() -> Result<()> {
    editor!("workspace-signed", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let b = repo.rev_parse_single("b")?.detach();
    let mut new_commit = but_core::Commit::from_id(repo.rev_parse_single("base")?)?;
    new_commit.message = "inserted below b".into();
    new_commit.parents = vec![].into();
    let new_id = repo.write_object(new_commit.inner)?.detach();

    let selector = editor.select_commit(b)?;
    editor.insert(selector, Step::new_pick(new_id), InsertSide::Below)?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A mid-stack commit disconnected and tombstoned (commit deletion).
///
/// When a commit is deleted, its co-located branch ref and the ref on the surviving parent
/// re-anchor onto that parent, fed by the leg the reconnect bridges in. The disconnect empties
/// their vias as it rewires; the full-child re-anchor restores the chain top's via to the
/// bridge (`legs_into_pick`, correct in the merge case too) before the moved refs inherit it.
/// The residual difference — the two collapsed branches' relative order — is normalized away by
/// [`assert_parity`], since co-located sibling order is a no-clean-spec arbitration.
#[test]
fn disconnect_and_remove_commit() -> Result<()> {
    editor!("workspace-signed", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let b = repo.rev_parse_single("b")?.detach();
    let selector = editor.select_commit(b)?;
    editor.disconnect_segment_from(
        mutate::SegmentDelimiter {
            child: selector,
            parent: selector,
        },
        mutate::SelectorSet::All,
        mutate::SelectorSet::All,
        false,
    )?;
    editor.replace(selector, Step::None)?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// An empty stack round-trips unchanged.
#[test]
fn noop_rebase_with_empty_stack() -> Result<()> {
    editor!("workspace-with-empty-stack", repo, _tmp, meta, ws);
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A dup-parent workspace commit (two divergent stacks merged) round-trips unchanged.
///
/// This is the per-ref-`via` shape: the two co-located `stack-a` / `stack-b` refs map to
/// DISTINCT legs of the merge, so a uniform "restore every ref to `legs_into_pick`" would be
/// wrong. A noop must still project identically after materialize + rewalk.
#[test]
fn noop_rebase_two_stacks() -> Result<()> {
    editor!("workspace-two-stacks", repo, _tmp, meta, ws);
    let editor = Editor::create(&mut ws, &mut *meta, &repo)?;
    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A new commit inserted above one lane's tip in the dup-parent merge fixture — the insert
/// must maintain the merge leg's `via` without disturbing the other lane.
#[test]
fn insert_pick_into_merge_lane() -> Result<()> {
    editor!("workspace-two-stacks", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a2 = repo.rev_parse_single("stack-a")?.detach();
    let mut new_commit = but_core::Commit::from_id(repo.rev_parse_single("stack-a")?)?;
    new_commit.message = "inserted above A2".into();
    new_commit.parents = vec![].into();
    let new_id = repo.write_object(new_commit.inner)?.detach();

    let selector = editor.select_commit(a2)?;
    editor.insert(selector, Step::new_pick(new_id), InsertSide::Above)?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A branch created mid-lane in the dup-parent merge fixture: the new ref must ADOPT the
/// lane's leg (`legs_into_pick`), not `via=[]`, in the per-ref-`via` merge context.
#[test]
fn insert_reference_into_merge_lane() -> Result<()> {
    editor!("workspace-two-stacks", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a1 = repo.rev_parse_single("stack-a~1")?.detach();
    let selector = editor.select_commit(a1)?;
    editor.insert(
        selector,
        Step::new_reference("refs/heads/mid-stack-a".try_into()?),
        InsertSide::Above,
    )?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// Two branches created above the SAME commit — co-located siblings, exercising the rank /
/// chain-member machinery. Their relative order is a no-clean-spec arbitration, normalized by
/// `assert_parity`; what must hold is that BOTH land on the right commit.
#[test]
fn insert_colocated_sibling_references() -> Result<()> {
    editor!("workspace-signed", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let b = repo.rev_parse_single("b")?.detach();
    let first = editor.select_commit(b)?;
    editor.insert(
        first,
        Step::new_reference("refs/heads/sibling-one".try_into()?),
        InsertSide::Above,
    )?;
    let second = editor.select_commit(b)?;
    editor.insert(
        second,
        Step::new_reference("refs/heads/sibling-two".try_into()?),
        InsertSide::Above,
    )?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A whole lane of the dup-parent merge deleted down to empty (both A1 and A2 removed): the
/// `stack-a` ref collapses onto the shared `base`, co-located with `main` — the "two empties on
/// one base" corner, over the merge's per-ref legs.
#[test]
fn delete_whole_lane_in_merge() -> Result<()> {
    editor!("workspace-two-stacks", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    for rev in ["stack-a", "stack-a~1"] {
        let commit = repo.rev_parse_single(rev)?.detach();
        let selector = editor.select_commit(commit)?;
        editor.disconnect_segment_from(
            mutate::SegmentDelimiter {
                child: selector,
                parent: selector,
            },
            mutate::SelectorSet::All,
            mutate::SelectorSet::All,
            false,
        )?;
        editor.replace(selector, Step::None)?;
    }

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A lane-tip commit (carrying the co-located `stack-a` ref) deleted in the dup-parent merge
/// fixture — the co-located ref re-anchors onto the lane's surviving parent, and the merge's
/// per-ref legs must be preserved (a blanket `legs_into_pick` restore would collide here).
#[test]
fn delete_lane_tip_in_merge() -> Result<()> {
    editor!("workspace-two-stacks", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let a2 = repo.rev_parse_single("stack-a")?.detach();
    let selector = editor.select_commit(a2)?;
    editor.disconnect_segment_from(
        mutate::SegmentDelimiter {
            child: selector,
            parent: selector,
        },
        mutate::SelectorSet::All,
        mutate::SelectorSet::All,
        false,
    )?;
    editor.replace(selector, Step::None)?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

/// A new reference created BELOW a mid-stack commit — the uncovered arm of the
/// insert(reference, side) matrix (branch creation anchored under the selected commit).
#[test]
fn insert_reference_below_commit() -> Result<()> {
    editor!("workspace-signed", repo, _tmp, meta, ws);
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let b = repo.rev_parse_single("b")?.detach();
    let selector = editor.select_commit(b)?;
    editor.insert(
        selector,
        Step::new_reference("refs/heads/created-below".try_into()?),
        InsertSide::Below,
    )?;

    let rebase = editor.rebase()?;
    let (mutated, rewalked) = rewalk_parity_report(rebase, &repo)?;
    assert_parity(&mutated, &rewalked);
    Ok(())
}

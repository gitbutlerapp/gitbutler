//! Snapshot tests for [`detailed_graph_workspace`], mirroring the permutations
//! of the `but-rebase` `graph_workspace` suite, but exercising the extra
//! information this projection computes on top of the raw graph: the ordered
//! rows per stack, the box-drawing geometry, plus the `linear_segments` and
//! `reference_segments`.
//!
//! Each stack is rendered as a box-drawing DAG (in the same style as the
//! `but-rebase` `graph_workspace` snapshots) by replaying the stored
//! `node_line`/`link_line` geometry through a small port of renderdag's
//! formatter, with each row index-prefixed so the segments below it stay
//! readable.

use anyhow::Result;
use but_core::ref_metadata::ProjectMeta;
use but_graph::{Graph, init::Options};
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::{gix_testtools::tempfile::TempDir, visualize_commit_graph_all};
use but_workspace::workspace::{
    DetailedGraphWorkspace, GraphRowData, Stack, detailed_graph_workspace,
};
use gix::bstr::ByteSlice;
use renderdag::{LinkLine, NodeLine};
use snapbox::IntoData;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, add_stack_with_segments, named_writable_scenario_with_description,
};

/// Build the detailed workspace for `fixture`, optionally bounded by a target
/// ref (e.g. `"refs/heads/main"`). Returns the repo too so callers can also
/// snapshot the underlying commit graph.
fn detailed(
    fixture: &str,
    target: Option<&str>,
) -> Result<(gix::Repository, DetailedGraphWorkspace)> {
    let repo = crate::utils::read_only_in_memory_scenario(fixture)?;
    let mut meta = VirtualBranchesTomlMetadata::from_path(
        repo.path()
            .join(".git")
            .join("should-never-be-written.toml"),
    )?;
    let project_meta = ProjectMeta {
        target_ref: target.map(gix::refs::FullName::try_from).transpose()?,
        // Bound the graph at the target commit too, so the projection is
        // actually trimmed (matching how a real workspace target behaves).
        target_commit_id: target
            .map(|t| repo.rev_parse_single(t).map(|id| id.detach()))
            .transpose()?,
        ..Default::default()
    };
    let graph = Graph::from_head(&repo, &meta, project_meta, Options::limited())?;
    let mut ws = graph.into_workspace()?;
    let detailed = detailed_graph_workspace(&mut ws, &mut meta, &repo)?;
    Ok((repo, detailed))
}

/// Build a detailed workspace from one of the writable upstream-integration
/// fixtures after applying stack metadata.
fn detailed_writable(
    fixture: &str,
    target_remote: &str,
    target_branch: &str,
    target_rev: &str,
    mut configure_stacks: impl FnMut(&mut VirtualBranchesTomlMetadata),
) -> Result<(TempDir, DetailedGraphWorkspace)> {
    let (tmp, repo, mut meta, _desc) = named_writable_scenario_with_description(fixture)?;
    let target_sha = repo.rev_parse_single(target_rev)?.detach();
    configure_stacks(&mut meta);

    let project_meta = ProjectMeta {
        target_ref: Some(format!("refs/remotes/{target_remote}/{target_branch}").try_into()?),
        target_commit_id: Some(target_sha),
        push_remote: None,
    };
    let graph = Graph::from_head(
        &repo,
        &meta,
        project_meta,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut ws = graph.into_workspace()?;
    let detailed = detailed_graph_workspace(&mut ws, &mut meta, &repo)?;
    Ok((tmp, detailed))
}

/// Curved box-drawing glyphs, indexed by the `glyph::*` constants. Transcribed
/// from renderdag's `BoxDrawingRenderer` (its `CURVED_GLYPHS` table and the
/// formatter are private), so these snapshots match the `but-rebase`
/// `graph_workspace` box-drawing style.
const GLYPHS: [&str; 13] = [
    "  ", "──", "│ ", "╷ ", "╯ ", "╰─", "┴─", "╮ ", "╭─", "┬─", "┤ ", "├─", "┼─",
];

#[allow(dead_code, clippy::missing_docs_in_private_items)]
mod glyph {
    pub const SPACE: usize = 0;
    pub const HORIZONTAL: usize = 1;
    pub const PARENT: usize = 2;
    pub const ANCESTOR: usize = 3;
    pub const MERGE_LEFT: usize = 4;
    pub const MERGE_RIGHT: usize = 5;
    pub const MERGE_BOTH: usize = 6;
    pub const FORK_LEFT: usize = 7;
    pub const FORK_RIGHT: usize = 8;
    pub const FORK_BOTH: usize = 9;
    pub const JOIN_LEFT: usize = 10;
    pub const JOIN_RIGHT: usize = 11;
    pub const JOIN_BOTH: usize = 12;
}

/// Render each stack as a box-drawing DAG (rows index-prefixed so the segments
/// stay readable), then its linear and reference segments by row index.
fn render(detailed: &DetailedGraphWorkspace) -> String {
    use std::fmt::Write as _;
    if detailed.stacks.is_empty() {
        return "(no stacks)".into();
    }
    let mut out = String::new();
    for (i, stack) in detailed.stacks.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "# Stack {i}");
        let _ = writeln!(out, "{}", render_box(stack));
        for seg in &stack.linear_segments {
            let r = seg.reference_idx.map_or("-".to_string(), |r| r.to_string());
            let _ = writeln!(out, "  linear    ref={r:<2} rows={:?}", seg.row_idxs);
        }
        for seg in &stack.reference_segments {
            let _ = writeln!(
                out,
                "  reference ref={:<2} rows={:?}",
                seg.reference_idx, seg.row_idxs
            );
        }
    }
    out.trim_end().to_string()
}

/// Replay a stack's stored renderdag geometry (`node_line`/`link_line`) back
/// into box-drawing. The production rows carry only the geometry (the glyph and
/// message are applied by the consumer), so we supply the symbol + label here.
fn render_box(stack: &Stack) -> String {
    let mut out = String::new();
    for (idx, row) in stack.rows.iter().enumerate() {
        let (sym, label) = row_glyph_label(&row.data);
        let mut line = String::new();
        for entry in &row.node_line {
            match entry {
                NodeLine::Node => {
                    line.push_str(sym);
                    line.push(' ');
                }
                NodeLine::Parent => line.push_str(GLYPHS[glyph::PARENT]),
                NodeLine::Ancestor => line.push_str(GLYPHS[glyph::ANCESTOR]),
                NodeLine::Blank => line.push_str(GLYPHS[glyph::SPACE]),
            }
        }
        line.push_str(&format!(" {idx} {label}"));
        out.push_str(line.trim_end());
        out.push('\n');

        // A merge row (renderdag's `merge` flag) changes a couple of link-line
        // glyphs. We approximate it by the commit's parent count, which matches
        // renderdag's `parents.len() > 1` whenever both parents are visible (as
        // they are in these fixtures).
        let merge =
            matches!(&row.data, GraphRowData::Commit { commit, .. } if commit.parent_ids.len() > 1);
        if let Some(link_row) = &row.link_line {
            let mut link = String::new();
            for cur in link_row {
                link.push_str(GLYPHS[link_glyph(*cur, merge)]);
            }
            out.push_str(link.trim_end());
            out.push('\n');
        }

        assert!(
            row.term_line.is_none(),
            "terminator rows not supported by this test renderer"
        );
    }
    out.trim_end().to_string()
}

/// The glyph for one `link_line` cell — a faithful transcription of renderdag's
/// box-drawing match (kept structurally 1:1 for easy diffing; renderdag
/// `#[allow]`s the same lint on the same code).
#[allow(clippy::if_same_then_else)]
fn link_glyph(cur: LinkLine, merge: bool) -> usize {
    if cur.intersects(LinkLine::HORIZONTAL) {
        if cur.intersects(LinkLine::CHILD) {
            glyph::JOIN_BOTH
        } else if cur.intersects(LinkLine::ANY_FORK) && cur.intersects(LinkLine::ANY_MERGE) {
            glyph::JOIN_BOTH
        } else if cur.intersects(LinkLine::ANY_FORK)
            && cur.intersects(LinkLine::VERT_PARENT)
            && !merge
        {
            glyph::JOIN_BOTH
        } else if cur.intersects(LinkLine::ANY_FORK) {
            glyph::FORK_BOTH
        } else if cur.intersects(LinkLine::ANY_MERGE) {
            glyph::MERGE_BOTH
        } else {
            glyph::HORIZONTAL
        }
    } else if cur.intersects(LinkLine::VERT_PARENT) && !merge {
        let left = cur.intersects(LinkLine::LEFT_MERGE | LinkLine::LEFT_FORK);
        let right = cur.intersects(LinkLine::RIGHT_MERGE | LinkLine::RIGHT_FORK);
        match (left, right) {
            (true, true) => glyph::JOIN_BOTH,
            (true, false) => glyph::JOIN_LEFT,
            (false, true) => glyph::JOIN_RIGHT,
            (false, false) => glyph::PARENT,
        }
    } else if cur.intersects(LinkLine::VERT_PARENT | LinkLine::VERT_ANCESTOR)
        && !cur.intersects(LinkLine::LEFT_FORK | LinkLine::RIGHT_FORK)
    {
        let left = cur.intersects(LinkLine::LEFT_MERGE);
        let right = cur.intersects(LinkLine::RIGHT_MERGE);
        match (left, right) {
            (true, true) => glyph::JOIN_BOTH,
            (true, false) => glyph::JOIN_LEFT,
            (false, true) => glyph::JOIN_RIGHT,
            (false, false) => {
                if cur.intersects(LinkLine::VERT_ANCESTOR) {
                    glyph::ANCESTOR
                } else {
                    glyph::PARENT
                }
            }
        }
    } else if cur.intersects(LinkLine::LEFT_FORK)
        && cur.intersects(LinkLine::LEFT_MERGE | LinkLine::CHILD)
    {
        glyph::JOIN_LEFT
    } else if cur.intersects(LinkLine::RIGHT_FORK)
        && cur.intersects(LinkLine::RIGHT_MERGE | LinkLine::CHILD)
    {
        glyph::JOIN_RIGHT
    } else if cur.intersects(LinkLine::LEFT_MERGE) && cur.intersects(LinkLine::RIGHT_MERGE) {
        glyph::MERGE_BOTH
    } else if cur.intersects(LinkLine::LEFT_FORK) && cur.intersects(LinkLine::RIGHT_FORK) {
        glyph::FORK_BOTH
    } else if cur.intersects(LinkLine::LEFT_FORK) {
        glyph::FORK_LEFT
    } else if cur.intersects(LinkLine::LEFT_MERGE) {
        glyph::MERGE_LEFT
    } else if cur.intersects(LinkLine::RIGHT_FORK) {
        glyph::FORK_RIGHT
    } else if cur.intersects(LinkLine::RIGHT_MERGE) {
        glyph::MERGE_RIGHT
    } else {
        glyph::SPACE
    }
}

/// List every reference row with the push status this projection computed for
/// it: its own status, the combined status (folding in parent references), and
/// the remote-tracking branch it was compared against.
fn render_push_status(detailed: &DetailedGraphWorkspace) -> String {
    use std::fmt::Write as _;
    if detailed.stacks.is_empty() {
        return "(no stacks)".into();
    }
    let mut out = String::new();
    for (i, stack) in detailed.stacks.iter().enumerate() {
        let _ = writeln!(out, "# Stack {i}");
        for row in &stack.rows {
            let GraphRowData::Reference {
                ref_name,
                additional_ref_info,
            } = &row.data
            else {
                continue;
            };
            let info = additional_ref_info.as_ref();
            let remote = info
                .and_then(|i| i.remote_ref.as_ref())
                .map(|r| r.as_bstr().to_string())
                .unwrap_or_else(|| "-".into());
            let push = info.map_or_else(|| "-".into(), |i| format!("{:?}", i.push_status));
            let combined =
                info.map_or_else(|| "-".into(), |i| format!("{:?}", i.combined_push_status));
            let _ = writeln!(
                out,
                "{:<14} push={push:<30} combined={combined:<30} remote={remote}",
                ref_name.as_bstr().to_string()
            );
        }
    }
    out.trim_end().to_string()
}

/// List every commit row with the per-commit [`CommitState`] the projection
/// computed for it.
fn render_commit_state(detailed: &DetailedGraphWorkspace) -> String {
    use std::fmt::Write as _;
    if detailed.stacks.is_empty() {
        return "(no stacks)".into();
    }
    let mut out = String::new();
    for (i, stack) in detailed.stacks.iter().enumerate() {
        let _ = writeln!(out, "# Stack {i}");
        for row in &stack.rows {
            let GraphRowData::Commit { commit, state } = &row.data else {
                continue;
            };
            let subject = commit
                .message
                .lines()
                .next()
                .map(|l| l.to_str_lossy().trim().to_string())
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "{} {subject:<8} state={}",
                commit.id.to_hex_with_len(7),
                state.display(commit.id)
            );
        }
    }
    out.trim_end().to_string()
}

fn render_statuses(detailed: &DetailedGraphWorkspace) -> String {
    format!(
        "{}\n\n{}",
        render_push_status(detailed),
        render_commit_state(detailed)
    )
}

fn row_glyph_label(data: &GraphRowData) -> (&'static str, String) {
    match data {
        GraphRowData::Commit { commit, .. } => {
            let subject = commit
                .message
                .lines()
                .next()
                .map(|l| l.to_str_lossy().trim().to_string())
                .unwrap_or_default();
            ("●", format!("{} {subject}", commit.id.to_hex_with_len(7)))
        }
        GraphRowData::Reference { ref_name, .. } => ("◎", ref_name.as_bstr().to_string()),
    }
}

/// A linear workspace (base→a→b→c under the workspace commit) with no target:
/// the single stack reaches all the way down to `base`.
#[test]
fn single_stack_no_target() -> Result<()> {
    let (repo, detailed) = detailed("workspace-linear", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6db951d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 120e3a9 (main, c) c
* a96434e (b) b
* d591dfe (a) a
* 35b8235 (base) base

"#]]
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/c
◎  1 refs/heads/main
●  2 120e3a9 c
◎  3 refs/heads/b
●  4 a96434e b
◎  5 refs/heads/a
●  6 d591dfe a
◎  7 refs/heads/base
●  8 35b8235 base
  linear    ref=0  rows=[0]
  linear    ref=1  rows=[1, 2]
  linear    ref=3  rows=[3, 4]
  linear    ref=5  rows=[5, 6]
  linear    ref=7  rows=[7, 8]
  reference ref=0  rows=[0]
  reference ref=1  rows=[1, 2]
  reference ref=3  rows=[3, 4]
  reference ref=5  rows=[5, 6]
  reference ref=7  rows=[7, 8]
"#]]
    );
    Ok(())
}

/// The same linear workspace bounded by a target at `base`.
#[test]
fn single_stack_with_target() -> Result<()> {
    let (_repo, detailed) = detailed("workspace-linear", Some("refs/heads/base"))?;
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/c
◎  1 refs/heads/main
●  2 120e3a9 c
◎  3 refs/heads/b
●  4 a96434e b
◎  5 refs/heads/a
●  6 d591dfe a
◎  7 refs/heads/base
  linear    ref=0  rows=[0]
  linear    ref=1  rows=[1, 2]
  linear    ref=3  rows=[3, 4]
  linear    ref=5  rows=[5, 6]
  linear    ref=7  rows=[7]
  reference ref=0  rows=[0]
  reference ref=1  rows=[1, 2]
  reference ref=3  rows=[3, 4]
  reference ref=5  rows=[5, 6]
  reference ref=7  rows=[7]
"#]]
    );
    Ok(())
}

/// Two parents of the workspace commit whose histories overlap; with no target
/// they share ancestry, so de-duplication merges them into a single stack.
#[test]
fn overlapping_stacks_merge_into_one() -> Result<()> {
    let (repo, detailed) = detailed("workspace-with-empty-stack", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   74bcc92 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
* | 2169646 (stack-1) Commit D
* | 46ef828 Commit C
|/  
| * a0f2ac5 (origin/main, main) Commit X
|/  
* f555940 (stack-2) Commit A
* d664be0 Commit B
* fafd9d0 init

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/stack-1
●  1 2169646 Commit D
●  2 46ef828 Commit C
◎  3 refs/heads/stack-2
●  4 f555940 Commit A
●  5 d664be0 Commit B
●  6 fafd9d0 init
  linear    ref=0  rows=[0, 1, 2]
  linear    ref=3  rows=[3, 4, 5, 6]
  reference ref=0  rows=[0, 1, 2]
  reference ref=3  rows=[3, 4, 5, 6]
"#]]
    );
    Ok(())
}

/// Three stacks that all point at the same base commit collapse into one stack.
#[test]
fn three_stacks_same_base_collapse() -> Result<()> {
    let (repo, detailed) = detailed("workspace-with-three-empty-stacks", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* a26ae77 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
| * 1cf9cf4 (origin/main, main) Commit X
|/  
* fafd9d0 (stack-3, stack-2, stack-1) init

"#]]
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/stack-1
◎  1 refs/heads/stack-2
◎  2 refs/heads/stack-3
●  3 fafd9d0 init
  linear    ref=0  rows=[0]
  linear    ref=1  rows=[1]
  linear    ref=2  rows=[2, 3]
  reference ref=0  rows=[0]
  reference ref=1  rows=[1]
  reference ref=2  rows=[2, 3]
"#]]
    );
    Ok(())
}

/// Two divergent branches sharing `base` merge into a single stack: the
/// fork/merge is where `linear_segments` splits and `reference_segments`
/// computes exclusive reachability.
#[test]
fn divergent_stacks_sharing_base_merge() -> Result<()> {
    let (repo, detailed) = detailed("workspace-two-stacks", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   1162583 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * afc3f8f (stack-b) B2
| * b3ee99c B1
* | 49c06ff (stack-a) A2
* | ff76d2f A1
|/  
* 965998b (origin/main, main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/stack-a
●  1 49c06ff A2
●  2 ff76d2f A1
│ ◎  3 refs/heads/stack-b
│ ●  4 afc3f8f B2
│ ●  5 b3ee99c B1
├─╯
◎  6 refs/heads/main
●  7 965998b base
  linear    ref=0  rows=[0, 1, 2]
  linear    ref=3  rows=[3, 4, 5]
  linear    ref=6  rows=[6, 7]
  reference ref=0  rows=[0, 1, 2]
  reference ref=3  rows=[3, 4, 5]
  reference ref=6  rows=[6, 7]
"#]]
    );
    Ok(())
}

/// The same divergent fixture bounded by a target at `main` (which sits at
/// `base`).
#[test]
fn divergent_stacks_sharing_base_merge_with_target() -> Result<()> {
    let (_repo, detailed) = detailed("workspace-two-stacks", Some("refs/heads/main"))?;
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/stack-a
●  1 49c06ff A2
●  2 ff76d2f A1
│ ◎  3 refs/heads/stack-b
│ ●  4 afc3f8f B2
│ ●  5 b3ee99c B1
├─╯
◎  6 refs/heads/main
  linear    ref=0  rows=[0, 1, 2]
  linear    ref=3  rows=[3, 4, 5]
  linear    ref=6  rows=[6]
  reference ref=0  rows=[0, 1, 2]
  reference ref=3  rows=[3, 4, 5]
  reference ref=6  rows=[6]
"#]]
    );
    Ok(())
}

/// Pegged onto `main` (no workspace commit): everything reachable from HEAD
/// lands in one stack.
#[test]
fn pegged_no_target() -> Result<()> {
    let (repo, detailed) = detailed("four-commits", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 120e3a9 (HEAD -> main) c
* a96434e b
* d591dfe a
* 35b8235 base

"#]]
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/main
●  1 120e3a9 c
●  2 a96434e b
●  3 d591dfe a
●  4 35b8235 base
  linear    ref=0  rows=[0, 1, 2, 3, 4]
  reference ref=0  rows=[0, 1, 2, 3, 4]
"#]]
    );
    Ok(())
}

/// Two stacks with no shared history stay genuinely separate.
#[test]
fn disjoint_stacks_stay_separate() -> Result<()> {
    let (repo, detailed) = detailed("workspace-disjoint-stacks", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   f97c026 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * cb7021b (stack-b) B2
| * ce3278a B1
* 49c06ff (stack-a) A2
* ff76d2f A1
* 965998b (origin/main, main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/stack-b
●  1 cb7021b B2
●  2 ce3278a B1
  linear    ref=0  rows=[0, 1, 2]
  reference ref=0  rows=[0, 1, 2]

# Stack 1
◎  0 refs/heads/stack-a
●  1 49c06ff A2
●  2 ff76d2f A1
◎  3 refs/heads/main
●  4 965998b base
  linear    ref=0  rows=[0, 1, 2]
  linear    ref=3  rows=[3, 4]
  reference ref=0  rows=[0, 1, 2]
  reference ref=3  rows=[3, 4]
"#]]
    );
    Ok(())
}

/// The same disjoint stacks, bounded by a target at `main`.
#[test]
fn disjoint_stacks_stay_separate_with_target() -> Result<()> {
    let (_repo, detailed) = detailed("workspace-disjoint-stacks", Some("refs/heads/main"))?;
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/stack-b
●  1 cb7021b B2
●  2 ce3278a B1
  linear    ref=0  rows=[0, 1, 2]
  reference ref=0  rows=[0, 1, 2]

# Stack 1
◎  0 refs/heads/stack-a
●  1 49c06ff A2
●  2 ff76d2f A1
◎  3 refs/heads/main
  linear    ref=0  rows=[0, 1, 2]
  linear    ref=3  rows=[3]
  reference ref=0  rows=[0, 1, 2]
  reference ref=3  rows=[3]
"#]]
    );
    Ok(())
}

/// A single stack of two dependent branches stacked on a shared base. Each
/// reference owns the run of commits exclusively below it: `branch-top` owns its
/// two commits, `branch-bottom` owns its two, and `main` owns `base` — so each
/// `reference_segment` spans multiple commits within one linear stack.
#[test]
fn stacked_dependent_branches_partition_per_reference() -> Result<()> {
    let (repo, detailed) = detailed("workspace-stacked-branches", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* bec4789 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 2fe462c (branch-top) top 2
* 2c08207 top 1
* d27b21f (branch-bottom) bottom 2
* a3a5a44 bottom 1
* 965998b (origin/main, main) base

"#]]
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/branch-top
●  1 2fe462c top 2
●  2 2c08207 top 1
◎  3 refs/heads/branch-bottom
●  4 d27b21f bottom 2
●  5 a3a5a44 bottom 1
◎  6 refs/heads/main
●  7 965998b base
  linear    ref=0  rows=[0, 1, 2]
  linear    ref=3  rows=[3, 4, 5]
  linear    ref=6  rows=[6, 7]
  reference ref=0  rows=[0, 1, 2]
  reference ref=3  rows=[3, 4, 5]
  reference ref=6  rows=[6, 7]
"#]]
    );
    Ok(())
}

/// The same stack bounded by a target at `main`: `base` drops out, so `main`'s
/// reference segment becomes header-only while the branch segments are intact.
#[test]
fn stacked_dependent_branches_with_target() -> Result<()> {
    let (_repo, detailed) = detailed("workspace-stacked-branches", Some("refs/heads/main"))?;
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/branch-top
●  1 2fe462c top 2
●  2 2c08207 top 1
◎  3 refs/heads/branch-bottom
●  4 d27b21f bottom 2
●  5 a3a5a44 bottom 1
◎  6 refs/heads/main
  linear    ref=0  rows=[0, 1, 2]
  linear    ref=3  rows=[3, 4, 5]
  linear    ref=6  rows=[6]
  reference ref=0  rows=[0, 1, 2]
  reference ref=3  rows=[3, 4, 5]
  reference ref=6  rows=[6]
"#]]
    );
    Ok(())
}

/// A stack whose branch sits above an internal merge: `feature` reaches the
/// whole diamond `M -> {P, Q}` before the walk stops at `main`, so its
/// `reference_segment` is non-linear (`[feature, M, P, Q]`), while `base` below
/// `main` belongs to `main`.
#[test]
fn non_linear_reference_segment_with_internal_merge() -> Result<()> {
    let (repo, detailed) = detailed("workspace-merge-in-stack", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 30345c3 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
*   d585e46 (feature) M
|\  
| * ed2a973 Q
* | 6339d5b P
|/  
* 965998b (origin/main, main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/feature
●    1 d585e46 M
├─╮
● │  2 6339d5b P
│ ●  3 ed2a973 Q
├─╯
◎  4 refs/heads/main
●  5 965998b base
  linear    ref=0  rows=[0]
  linear    ref=-  rows=[1]
  linear    ref=-  rows=[2, 3]
  linear    ref=4  rows=[4, 5]
  reference ref=0  rows=[0, 1, 2, 3]
  reference ref=4  rows=[4, 5]
"#]]
    );
    Ok(())
}

/// Two stacks forking from a shared, unnamed commit `S` (no reference sits
/// between `S` and the fork). `S` is reachable from both `stack-x` and
/// `stack-y`, so it is included in *both* of their reference segments.
#[test]
fn shared_commit_belongs_to_both_reference_segments() -> Result<()> {
    let (repo, detailed) = detailed("workspace-shared-commit", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   a3bbad0 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * e6d5410 (stack-y) Y1
* | 9f0269c (stack-x) X1
|/  
* d2bff94 S
* 965998b (origin/main, main) base

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(
        render(&detailed),
        snapbox::str![[r#"
# Stack 0
◎  0 refs/heads/stack-x
●  1 9f0269c X1
│ ◎  2 refs/heads/stack-y
│ ●  3 e6d5410 Y1
├─╯
●  4 d2bff94 S
◎  5 refs/heads/main
●  6 965998b base
  linear    ref=0  rows=[0, 1]
  linear    ref=2  rows=[2, 3]
  linear    ref=-  rows=[4]
  linear    ref=5  rows=[5, 6]
  reference ref=0  rows=[0, 1, 4]
  reference ref=2  rows=[2, 3, 4]
  reference ref=5  rows=[5, 6]
"#]]
    );
    Ok(())
}

/// Push status: `main` matches its remote `origin/main` (both at `base`), so it
/// has nothing to push; `stack-a`/`stack-b` have no remote-tracking branch, so
/// they are completely unpushed.
///
/// The divergent (force/unpushed) statuses are covered by the
/// `combined_status_escalates_from_force_parent` and `push_status_mapping` unit
/// tests in `but-rebase` (`graph_rebase::workspace::test`); reaching them
/// end-to-end needs a workspace graph that traverses per-branch remotes, which
/// this metadata-free projection harness intentionally does not set up.
#[test]
fn push_status_nothing_to_push_and_unpushed() -> Result<()> {
    let (_repo, detailed) = detailed("workspace-two-stacks", Some("refs/heads/main"))?;
    snapbox::assert_data_eq!(
        render_push_status(&detailed),
        snapbox::str![[r#"
# Stack 0
refs/heads/stack-a push=CompletelyUnpushed             combined=CompletelyUnpushed             remote=-
refs/heads/stack-b push=CompletelyUnpushed             combined=CompletelyUnpushed             remote=-
refs/heads/main push=NothingToPush                  combined=NothingToPush                  remote=refs/remotes/origin/main
"#]]
    );
    Ok(())
}

/// Integration status: branch `A` sits at `origin/main` so every commit it owns
/// has landed upstream — it reports `Integrated`. Branch `B` has an un-merged
/// commit, so it has no remote tracking branch (`CompletelyUnpushed`). Mirrors
/// the `fully-integrated-branch` upstream-integration fixture, which needs real
/// project metadata for the projection to know what it
/// integrates into.
///
/// Note the force statuses in the snapshot: `origin/main` was advanced past
/// local `main` (it now contains `A1`), so `main` is *behind* its remote and
/// reports `UnpushedCommitsRequiringForce`. `B` sits above `main` in the stack,
/// so its `combined` status escalates to force even though its own push status
/// is `CompletelyUnpushed`.
#[test]
fn integration_status_marks_fully_integrated_branch() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "fully-integrated-branch",
        "origin",
        "main",
        "main",
        |meta| {
            add_stack(meta, 1, "A", StackState::InWorkspace);
            add_stack(meta, 2, "B", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_push_status(&detailed),
        snapbox::str![[r#"
# Stack 0
refs/heads/A   push=Integrated                     combined=Integrated                     remote=-
refs/heads/B   push=CompletelyUnpushed             combined=UnpushedCommitsRequiringForce  remote=-
refs/heads/main push=UnpushedCommitsRequiringForce  combined=UnpushedCommitsRequiringForce  remote=refs/remotes/origin/main
"#]]
    );
    // `add A1` (== origin/main) is integrated; `add B1` is local-only.
    snapbox::assert_data_eq!(
        render_commit_state(&detailed),
        snapbox::str![[r#"
# Stack 0
905d6e5 add A1   state=integrated
b38b04b add B1   state=local
"#]]
    );
    Ok(())
}

/// Commit state via CONTENT integration: stack commits `A` and `B` were
/// cherry-picked onto the target (`origin/master`) with *different* commit IDs,
/// so they are not reachable from the target ref and are caught only by the
/// changeset-similarity engine — exercising the content branch of
/// `is_commit_integrated`, which no other projection test reaches. `D` and `E`
/// are local-only, and the merge commit `C` carries no changes of its own.
#[test]
fn commit_state_marks_content_integrated_commits() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "diamond-partially-content-integrated",
        "origin",
        "master",
        "o1",
        |meta| {
            add_stack(meta, 1, "E", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_commit_state(&detailed),
        snapbox::str![[r#"
# Stack 0
a6588cf E        state=local
4827d2f C        state=local
3d3bfa7 B        state=integrated
d8d0970 D        state=local
f5b02d3 A        state=integrated
"#]]
    );
    Ok(())
}

/// Commit state via HISTORICAL integration: `A` and `B` are reachable from the
/// target ref through the merge commit on `origin/master`.
#[test]
fn commit_state_marks_historically_integrated_commits() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "diamond-partially-historically-integrated",
        "origin",
        "master",
        "o1",
        |meta| {
            add_stack(meta, 1, "E", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_commit_state(&detailed),
        snapbox::str![[r#"
# Stack 0
972cf74 E        state=local
9e74c75 C        state=local
ffb801b B        state=integrated
d6a7004 D        state=local
448b195 A        state=integrated
"#]]
    );
    Ok(())
}

/// Reference integration in a multi-branch stack: segment `C` is already
/// upstream, while its child segment `A` and sibling stack `B` still have local
/// work.
#[test]
fn integration_status_marks_partially_integrated_multi_branch_stack() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "partially-integrated-multi-branch-stack",
        "origin",
        "main",
        "main",
        |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &["C"]);
            add_stack(meta, 2, "B", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_statuses(&detailed),
        snapbox::str![[r#"
# Stack 0
refs/heads/A   push=CompletelyUnpushed             combined=UnpushedCommitsRequiringForce  remote=-
refs/heads/C   push=Integrated                     combined=Integrated                     remote=-
refs/heads/B   push=CompletelyUnpushed             combined=UnpushedCommitsRequiringForce  remote=-
refs/heads/main push=UnpushedCommitsRequiringForce  combined=UnpushedCommitsRequiringForce  remote=refs/remotes/origin/main

# Stack 0
44c9428 add A1   state=local
f1e7451 add C1   state=integrated
b38b04b add B1   state=local
"#]]
    );
    Ok(())
}

/// Same multi-branch shape, but both segments in stack `A` are integrated; `B`
/// remains local.
#[test]
fn integration_status_marks_fully_integrated_multi_branch_stack() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "fully-integrated-multi-branch-stack",
        "origin",
        "main",
        "main",
        |meta| {
            add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &["C"]);
            add_stack(meta, 2, "B", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_statuses(&detailed),
        snapbox::str![[r#"
# Stack 0
refs/heads/A   push=Integrated                     combined=Integrated                     remote=-
refs/heads/C   push=Integrated                     combined=Integrated                     remote=-
refs/heads/B   push=CompletelyUnpushed             combined=UnpushedCommitsRequiringForce  remote=-
refs/heads/main push=UnpushedCommitsRequiringForce  combined=UnpushedCommitsRequiringForce  remote=refs/remotes/origin/main

# Stack 0
44c9428 add A1   state=integrated
f1e7451 add C1   state=integrated
b38b04b add B1   state=local
"#]]
    );
    Ok(())
}

/// Two separate stacks whose commits have both landed upstream through merge
/// commits on the target.
#[test]
fn integration_status_marks_fully_integrated_two_stacks() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "fully-integrated-two-stacks",
        "origin",
        "main",
        "main~2",
        |meta| {
            add_stack(meta, 1, "A", StackState::InWorkspace);
            add_stack(meta, 2, "B", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_statuses(&detailed),
        snapbox::str![[r#"
# Stack 0
refs/heads/B   push=Integrated                     combined=Integrated                     remote=-
# Stack 1
refs/heads/A   push=Integrated                     combined=Integrated                     remote=-

# Stack 0
b38b04b add B1   state=integrated
# Stack 1
905d6e5 add A1   state=integrated
"#]]
    );
    Ok(())
}

/// Empty-branch remote-tip integration: the projection sees the shared topic
/// tip commit and marks both the commit and reference integrated.
#[test]
fn empty_branch_remote_tip_marks_reference_integrated() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "empty-branch-remote-tip-integrated",
        "origin",
        "main",
        "main^",
        |meta| {
            add_stack(meta, 1, "topic", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_statuses(&detailed),
        snapbox::str![[r#"
# Stack 0
refs/heads/topic push=Integrated                     combined=Integrated                     remote=refs/remotes/origin/topic

# Stack 0
6ba217e add topic state=integrated
"#]]
    );
    Ok(())
}

/// When a branch has local work above an integrated remote tip, only the shared
/// remote-tip commit is integrated; the local commit stays local.
#[test]
fn non_empty_branch_remote_tip_keeps_local_work_unintegrated() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "non-empty-branch-remote-tip-integrated",
        "origin",
        "main",
        "main^",
        |meta| {
            add_stack(meta, 1, "topic", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_statuses(&detailed),
        snapbox::str![[r#"
# Stack 0
refs/heads/topic push=UnpushedCommits                combined=UnpushedCommits                remote=refs/remotes/origin/topic

# Stack 0
f1a3cba add local state=local
6ba217e add topic state=integrated
"#]]
    );
    Ok(())
}

/// Commit state: `feature-foo`'s commit is present on `origin/feature-foo`
/// (which is ahead of it) but not on the target, so it reports `LocalAndRemote`
/// by identity — covering the third `CommitState` variant.
#[test]
fn commit_state_marks_pushed_unintegrated_commit_local_and_remote() -> Result<()> {
    let (_tmp, detailed) = detailed_writable(
        "remote-advanced-with-empty-top-branch",
        "origin",
        "main",
        "main",
        |meta| {
            add_stack(meta, 1, "feature-foo", StackState::InWorkspace);
        },
    )?;
    snapbox::assert_data_eq!(
        render_commit_state(&detailed),
        snapbox::str![[r#"
# Stack 0
f0c6d1c add foo.txt state=local/remote(identity)
"#]]
    );
    Ok(())
}

/// Commit state: `A` has `shared local/remote` present on `origin/A` by identity,
/// and `shared by name` which is a *different* commit on `origin/A` with the same
/// changes — caught only by the changeset-similarity match. The two unique local
/// commits stay local-only.
#[test]
fn commit_state_uses_similarity_for_local_and_remote() -> Result<()> {
    use crate::ref_info::with_workspace_commit::utils::{
        StackState, add_stack, project_meta, read_only_in_memory_scenario,
    };
    use anyhow::Context as _;

    let (repo, mut meta) = read_only_in_memory_scenario("target-ahead-remote-rewritten")?;
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    let project_meta = project_meta(&repo)?;
    let target_sha = project_meta
        .target_commit_id
        .context("scenario should configure a target")?;
    let graph = Graph::from_head(
        &repo,
        &*meta,
        project_meta,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?;
    let mut ws = graph.into_workspace()?;
    let detailed = detailed_graph_workspace(&mut ws, &mut *meta, &repo)?;
    snapbox::assert_data_eq!(
        render_commit_state(&detailed),
        snapbox::str![[r#"
# Stack 0
d5d3a92 unique local tip state=local
6ffd040 shared by name state=local/remote(similarity)
4cd56ab unique local state=local
872c22f shared local/remote state=local/remote(identity)
"#]]
    );
    Ok(())
}

/// On the workspace branch but the tip is a plain commit, not a managed
/// workspace commit: no stacks are produced.
#[test]
fn workspace_branch_without_managed_commit() -> Result<()> {
    let (repo, detailed) = detailed("workspace-without-managed-commit", None)?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 1b78c63 (HEAD -> gitbutler/workspace) just a normal commit
* 4d41a5c (origin/main, main) one
* 965998b base

"#]]
    );
    snapbox::assert_data_eq!(render(&detailed), snapbox::str!["(no stacks)"]);
    Ok(())
}

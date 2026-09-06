//! # The guided tour: one repository through the whole pipeline
//!
//! Every stage of the pipeline documented in isolation is documented here **together**,
//! on one small fixture, as the data each stage actually produces. Read this file top to
//! bottom before reading any module: the snapshots are the pipeline, and because they are
//! asserted, they cannot rot — if a stage's output changes, this tour fails and is
//! regenerated with `SNAPSHOTS=overwrite`.
//!
//! The pipeline under tour (see `but-graph`'s crate docs and `crates/WORKSPACE_MODEL.md`):
//!
//! ```text
//! repository ──walk──▶ CommitGraph (+ ref layout) ──frame + partition──▶ Workspace
//!                                                                          │
//!                                              display_stacks() ◀──────────┘  (per call)
//! ```
//!
//! To poke at a real repository the same way, `but-debug` prints these stages for any
//! checkout: `but-debug -t -t -t -C <repo> graph --stats` (spans time every phase).
//!
//! The tour continues where the pipeline ends — MUTATION through the graph editor —
//! in `but-rebase`'s `tests/rebase/graph_rebase/tour.rs`.

use but_graph::Workspace;
use but_testsupport::{graph_dag, graph_workspace, visualize_commit_graph_all};
use snapbox::prelude::*;

use crate::walk::utils::{
    add_workspace, default_project_meta, read_only_in_memory_scenario, standard_options,
};

/// The one test. Each stage is one snapshot with the story of what just happened.
#[test]
fn the_pipeline_stage_by_stage() -> anyhow::Result<()> {
    // ── Stage 0: the repository, as git sees it. ──────────────────────────────────────
    //
    // A GitButler workspace with three lanes merged by the workspace commit: `B` (two
    // commits), the dependent pair `D` on `C`, and `A` — which upstream has ALREADY
    // MERGED (`origin/main` contains it). `A` rests on the run S1..S3 (`shared`), and
    // `main`, the target's LOCAL, sits at the very bottom, behind everything. One small
    // repo exercising a managed merge, a dependent chain, an integrated branch, a
    // shared tail, and a target with a stale local.
    let (repo, mut meta) =
        read_only_in_memory_scenario("ws/multi-lane-with-shared-segment-one-integrated")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*-.   1cf594d (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\ \  
| | * 9895054 (D) D1
| | * de625cc (C) C3
| | * 23419f8 C2
| | * 5dc4389 C1
| * | acdc49a (B) B2
| * | f0117e0 B1
| |/  
| | *   c08dc6b (origin/main) Merge branch 'A' into soon-remote-main
| | |\  
| |_|/  
|/| |   
* | | 0bad3af (A) A1
|/ /  
* | d4f537e (shared) S3
* | b448757 S2
* | e9a378d S1
|/  
* 3183e43 (main) M1

"#]]
        .raw()
    );

    add_workspace(&mut meta);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        default_project_meta(),
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;

    // ── Stage 1: the seeds — what the walk is told to start from. ─────────────────────
    //
    // Seeds are resolved from HEAD, the workspace ref, declared stack branches (this
    // fixture's metadata declares none — the lanes below emerge from the merge's
    // parents alone), and the target pair; each role decides initial flags and pacing.
    // They ride along on the finished graph as data. Note the target's LOCAL: proven a
    // strict ancestor of the target (one generation-cutoff ancestry check), it is
    // recorded as a fact and never queued — walking to a stale local would drag the
    // traversal as far below the base as the local is behind. Its commit can still
    // surface in the arena the ordinary way, as stage 2 shows.
    snapbox::assert_data_eq!(
        seeds_table(&ws),
        snapbox::str![[r#"
👉1cf594d workspace      ►refs/heads/gitbutler/workspace
  c08dc6b target         ►refs/remotes/origin/main
  3183e43 target-local  refs/heads/main  (proven behind: a fact, not a walk)

"#]]
    );

    // ── Stage 2: the arena — what the walk observed. ──────────────────────────────────
    //
    // The walk stores commits with ORDERED PARENT ARRAYS and refs attached as data; no
    // structure is decided here. Flags propagate to ancestors: `⌂` reachable from a
    // non-remote seed, `🏘` inside the workspace merge's cone, `✓` reachable from the
    // target — so `A`, the shared run and `main` all carry it, while `origin/main`'s
    // own merge is `🟣`, owned by a remote alone. `👉` is the entrypoint, `🏁` is
    // history's first commit, and `<>` links a local to its remote counterpart. The
    // `layout:` trailer is the build's ref-placement table riding on the arena. (Full
    // glyph legend: `but-testsupport`'s `graph` module docs.)
    snapbox::assert_data_eq!(
        graph_dag(&ws),
        snapbox::str![[r#"
*      👉·1cf594d (⌂|🏘)
├─┬─╮
│ * │  ·acdc49a (⌂|🏘) ►B
│ * │  ·f0117e0 (⌂|🏘)
│ │ *  ·9895054 (⌂|🏘) ►D
│ │ *  ·de625cc (⌂|🏘) ►C
│ │ *  ·23419f8 (⌂|🏘)
│ │ *  ·5dc4389 (⌂|🏘)
│ ├─╯
│ │ *  🟣c08dc6b (✓) ►origin/main
╭───┤
* │ │  ·0bad3af (⌂|🏘|✓) ►A
├─╯ │
*   │  ·d4f537e (⌂|🏘|✓) ►shared
*   │  ·b448757 (⌂|🏘|✓)
*   │  ·e9a378d (⌂|🏘|✓)
├───╯
*  🏁·3183e43 (⌂|🏘|✓) ►main <> origin/main
layout:
  materialized parents: 1cf594d: 0bad3af acdc49a 9895054
"#]]
    );

    // ── Stage 3: the frame — the view's verdict. ──────────────────────────────────────
    //
    // Before any stack exists, the frame settles what KIND of view this is and its
    // boundaries: the entrypoint (where HEAD stood), the target, and the lower bound —
    // the workspace↔target merge-base, the floor every lane stops at.
    snapbox::assert_data_eq!(
        frame_report(&ws),
        snapbox::str![[r#"
kind:        managed (a workspace merge governs the view)
entrypoint:  1cf594d
target:      refs/remotes/origin/main, 1 commit(s) not yet integrated
lower bound: d4f537e

"#]]
    );

    // ── Stage 4: the partition — the stored stacks, the operations' authority. ────────
    //
    // ONE derivation (`projection/partition.rs`) colours the graph: tips are unioned
    // into classes by what they share above the base, and each class becomes one stack
    // at its full decided extent. All three stacks rest on the SAME floor — the bound —
    // and the dependent pair is one stack of two segments. This is the substrate
    // operations resolve against, and it is TOTAL: `A` is here even though upstream
    // already merged it. Hiding is not this layer's job.
    snapbox::assert_data_eq!(
        stored_partition(&ws),
        snapbox::str![[r#"
A(1) on d4f537e
B(2) on d4f537e
D(1) → C(3) on d4f537e

"#]]
    );

    // ── Stage 5: the display — derived per call, never stored. ────────────────────────
    //
    // `display_stacks()` re-materializes the stored shape for the UI: hide (archived,
    // out-of-cone, integrated), enrich (remote reachability, commits on remote), then
    // the view rule. Compare with stage 4: the integrated `A` lane is GONE — the
    // display prunes what upstream already has — while the substrate keeps it, which is
    // exactly why operations resolve on the substrate and never on this view. The `⇣1`
    // in the header is the target's un-integrated future. Nothing here is ever written
    // back; the next build re-derives everything from git.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on d4f537e
├── ≡:B on d4f537e
│   └── :B
│       ├── ·acdc49a (🏘️)
│       └── ·f0117e0 (🏘️)
└── ≡:D on d4f537e
    ├── :D
    │   └── ·9895054 (🏘️)
    └── :C
        ├── ·de625cc (🏘️)
        ├── ·23419f8 (🏘️)
        └── ·5dc4389 (🏘️)

"#]]
    );
    Ok(())
}

/// One line per seed: role, tip, and what the role means for pacing.
fn seeds_table(ws: &Workspace) -> String {
    use but_graph::walk::SeedRole;
    let mut out = String::new();
    for seed in ws.commit_graph().seeds() {
        let role = match &seed.role {
            SeedRole::Workspace => "workspace     ".to_string(),
            SeedRole::TargetRemote => "target        ".to_string(),
            SeedRole::TargetLocal {
                local_ref_name,
                behind_target,
            } => format!(
                "target-local  {}{}",
                local_ref_name.as_bstr(),
                if *behind_target {
                    "  (proven behind: a fact, not a walk)"
                } else {
                    ""
                }
            ),
            SeedRole::WorkspaceStackBranch { desired_ref_name } => {
                format!("stack-branch  {}", desired_ref_name.as_bstr())
            }
            SeedRole::Reachable => "reachable     ".to_string(),
        };
        out.push_str(&format!(
            "{ep}{id} {role}{name}\n",
            ep = if seed.is_entrypoint { "👉" } else { "  " },
            id = seed.id.to_hex_with_len(7),
            name = seed
                .ref_name
                .as_ref()
                .map(|r| format!(" ►{}", r.as_bstr()))
                .unwrap_or_default(),
        ));
    }
    out
}

/// The frame's four verdicts, one per line.
fn frame_report(ws: &Workspace) -> String {
    use but_graph::workspace::WorkspaceKind;
    format!(
        "kind:        {kind}\nentrypoint:  {ep}\ntarget:      {target}\nlower bound: {bound}\n",
        kind = match ws.kind() {
            WorkspaceKind::Managed { .. } => "managed (a workspace merge governs the view)",
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. } =>
                "managed, but the workspace commit is missing",
            WorkspaceKind::AdHoc => "ad-hoc (any HEAD position, no managed merge)",
        },
        ep = ws
            .entrypoint_commit_id()
            .ok()
            .flatten()
            .map(|id| id.to_hex_with_len(7).to_string())
            .unwrap_or_else(|| "<none>".into()),
        target = ws
            .target_ref
            .as_ref()
            .map(|t| {
                format!(
                    "{}, {} commit(s) not yet integrated",
                    t.ref_name.as_bstr(),
                    ws.incoming_target_commit_ids()
                        .map(|ids| ids.len())
                        .unwrap_or_default()
                )
            })
            .unwrap_or_else(|| "<none>".into()),
        bound = ws
            .lower_bound()
            .map(|id| id.to_hex_with_len(7).to_string())
            .unwrap_or_else(|| "<none>".into()),
    )
}

/// The stored partition, one stack per line: segments tip→bottom with commit counts,
/// then what the stack rests on. Fork/merge edges print only when the shape is a DAG.
fn stored_partition(ws: &Workspace) -> String {
    let mut out = String::new();
    for stack in &ws.stacks {
        let segments = stack
            .segments
            .iter()
            .map(|s| {
                format!(
                    "{}({})",
                    s.ref_name
                        .as_ref()
                        .map(|r| r.as_ref().shorten().to_string())
                        .unwrap_or_else(|| ":anon:".into()),
                    s.commits.len()
                )
            })
            .collect::<Vec<_>>()
            .join(" → ");
        let plain_adjacency = stack.edges.iter().copied().eq((0..stack
            .segments
            .len()
            .saturating_sub(1))
            .map(|i| (i, i + 1)));
        out.push_str(&format!(
            "{segments}{edges} on {base}\n",
            edges = if plain_adjacency {
                "".to_string()
            } else {
                format!("  [edges: {:?}]", stack.edges)
            },
            base = stack
                .base
                .map(|id| id.to_hex_with_len(7).to_string())
                .unwrap_or_else(|| "<root>".into()),
        ));
    }
    out
}

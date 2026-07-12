//! LOCAL-ONLY fuzz harnesses for GraphEditor operations, driven through their real
//! `but-workspace` callers. Deliberately kept out of version control: random testing stays
//! out of CI. Every finding gets pinned as a deterministic repro test in the committed
//! suite instead (see `move_branch.rs`).
//!
//! Wiring (all uncommitted): `mod fuzz;` in `branch/mod.rs`, `rand` in `[dev-dependencies]`.
#![allow(dead_code)]

use but_core::RefMetadata;
use but_graph::walk::Options;
use but_meta::{BranchOrderMetadata, VirtualBranchesTomlMetadata};
use but_rebase::graph_rebase::Editor;
use but_testsupport::invoke_bash;
use rand::{Rng, SeedableRng};
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack_with_segments, named_writable_scenario,
    named_writable_scenario_with_description_and_graph,
};
use crate::utils::r;

// ─── shared helpers (duplicated from the committed tests; local-only file) ───

fn segment_names(ws: &but_graph::Workspace) -> Vec<gix::refs::FullName> {
    ws.display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|seg| seg.ref_name().map(ToOwned::to_owned))
        .collect()
}

fn projected_sorted(ws: &but_graph::Workspace) -> Vec<String> {
    // A SET: shared parent segments legitimately appear in several stacks.
    let mut v: Vec<String> = segment_names(ws)
        .iter()
        .map(|name| name.as_bstr().to_string())
        .collect();
    v.sort();
    v.dedup();
    v
}

fn disk_heads(repo: &gix::Repository) -> anyhow::Result<Vec<String>> {
    let mut v: Vec<String> = repo
        .references()?
        .prefixed("refs/heads/")?
        .filter_map(Result::ok)
        .map(|r| r.name().as_bstr().to_string())
        .collect();
    v.sort();
    Ok(v)
}

fn all_commits(ws: &but_graph::Workspace) -> Vec<(String, gix::ObjectId)> {
    ws.display_stacks()
        .expect("displayable")
        .iter()
        .flat_map(|stack| &stack.segments)
        .flat_map(|seg| {
            let name = seg
                .ref_name()
                .map(|n| n.shorten().to_string())
                .unwrap_or_else(|| "anon".into());
            seg.commits
                .iter()
                .map(move |commit| (name.clone(), commit.id))
        })
        .collect()
}

fn set_workspace_metadata(
    meta: &mut impl RefMetadata,
    ws: &but_graph::Workspace,
    ws_meta: Option<but_core::ref_metadata::Workspace>,
) -> anyhow::Result<()> {
    if let Some((ws_meta, ref_name)) = ws_meta.zip(ws.ref_name()) {
        let mut md = meta.workspace(ref_name)?;
        *md = ws_meta;
        meta.set_workspace(&md)?;
    }
    Ok(())
}

fn managed_move_and_apply(
    repo: &gix::Repository,
    meta: &mut VirtualBranchesTomlMetadata,
    ws: &mut but_graph::Workspace,
    subject: &gix::refs::FullNameRef,
    target: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), meta, repo)?;
    but_workspace::branch::move_branch(editor, ws, subject, target)?.apply(ws, repo)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(repo, meta, project_meta)?;
    Ok(())
}

/// Run an editor-consuming op (move_commit, squash, …) and re-project the workspace.
fn managed_editor_op_and_apply(
    repo: &gix::Repository,
    meta: &mut VirtualBranchesTomlMetadata,
    ws: &mut but_graph::Workspace,
    op: impl FnOnce(
        Editor<'_, VirtualBranchesTomlMetadata>,
    ) -> anyhow::Result<
        but_rebase::graph_rebase::RebasedEditor<'_, VirtualBranchesTomlMetadata>,
    >,
) -> anyhow::Result<()> {
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), meta, repo)?;
    let rebase = op(editor)?;
    let (graph, _) = rebase.materialize()?;
    ws.refresh_from_commit_graph(graph, repo, meta)?;
    let project_meta = ws.project_meta().clone();
    ws.refresh_from_head(repo, meta, project_meta)?;
    Ok(())
}

// single-branch (ad-hoc) helpers

fn branch_order_meta(repo: &gix::Repository) -> anyhow::Result<BranchOrderMetadata> {
    BranchOrderMetadata::from_paths(repo.path().join("virtual-branches.toml"), repo.path())
}

fn project_meta(_meta: &impl RefMetadata) -> but_core::ref_metadata::ProjectMeta {
    but_core::ref_metadata::ProjectMeta {
        target_ref: Some(
            "refs/remotes/origin/main"
                .try_into()
                .expect("statically known to be valid"),
        ),
        ..Default::default()
    }
}

fn persist_order(
    meta: &mut BranchOrderMetadata,
    order: &Option<Vec<gix::refs::FullName>>,
) -> anyhow::Result<()> {
    if let Some(order) = order {
        meta.set_branch_stack_order(order)?;
    }
    Ok(())
}

fn move_branch_and_apply(
    repo: &gix::Repository,
    meta: &mut BranchOrderMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
    subject: &gix::refs::FullNameRef,
    target: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<Vec<gix::refs::FullName>>> {
    let ws = but_graph::Workspace::from_head(repo, meta, project_meta, Options::limited())?;
    let editor = Editor::create(ws.commit_graph(), ws.project_meta(), meta, repo)?;
    let but_workspace::branch::move_branch::Outcome {
        rebase,
        ws_meta,
        new_tip,
        branch_stack_order,
        ..
    } = but_workspace::branch::move_branch(editor, &ws, subject, target)?;
    assert!(
        ws_meta.is_none(),
        "ad-hoc reorder lives in branch_order, not workspace metadata"
    );
    rebase.materialize()?;
    persist_order(meta, &branch_stack_order)?;
    if let Some(new_tip) = new_tip {
        invoke_bash(&format!("git checkout {}\n", new_tip.shorten()), repo);
    }
    Ok(branch_stack_order)
}

// ─── managed-workspace scenarios ───

type MetaSetup = fn(&mut VirtualBranchesTomlMetadata);

fn two_owner_stacks(meta: &mut VirtualBranchesTomlMetadata) {
    add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(meta, 2, "C", StackState::InWorkspace, &["B"]);
}
fn stack_and_empty(meta: &mut VirtualBranchesTomlMetadata) {
    add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
    add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
}
fn empties_between_owners(meta: &mut VirtualBranchesTomlMetadata) {
    add_stack_with_segments(
        meta,
        1,
        "e3",
        StackState::InWorkspace,
        &["e2", "B", "e1", "A"],
    );
    add_stack_with_segments(meta, 2, "F", StackState::InWorkspace, &[]);
}
fn triple_files(meta: &mut VirtualBranchesTomlMetadata) {
    add_stack_with_segments(meta, 1, "D", StackState::InWorkspace, &["A"]);
    add_stack_with_segments(meta, 2, "E", StackState::InWorkspace, &["C", "B"]);
}

const MANAGED_SCENARIOS: &[(&str, MetaSetup)] = &[
    (
        "ws-ref-ws-commit-single-stack-double-stack",
        two_owner_stacks,
    ),
    ("ws-with-empty-stack", stack_and_empty),
    (
        "ws-ref-ws-commit-empties-between-owners",
        empties_between_owners,
    ),
    (
        "ws-ref-ws-commit-double-stack-triple-stack-files",
        triple_files,
    ),
    // origin/main advanced past the fork point: the base segment is anonymous, and both
    // branches have a remote-tracking ref. Same A+B metadata as the empty-stack scenario.
    (
        "ws-ref-ws-commit-two-stacks-advanced-remote",
        stack_and_empty,
    ),
];

// ─── diagnosis replay (edit the sequence, run with --nocapture) ───

#[ignore = "diagnosis driver; edit the sequence to replay a fuzz finding"]
#[test]
fn replay_managed_finding() -> anyhow::Result<()> {
    use but_core::ref_metadata::StackId;

    fn test_stack_id(rn: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(rn.as_bstr().iter().map(|&b| b as u128).sum())
    }
    let (fixture, setup) = MANAGED_SCENARIOS[0]; // single-stack-double-stack
    let (_tmp, mut ws, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(fixture, setup)?;
    let dump = |ws: &but_graph::Workspace, repo: &gix::Repository| -> anyhow::Result<()> {
        eprintln!(
            "--- commits ---\n{}--- workspace ---\n{}",
            but_testsupport::visualize_commit_graph_all(repo)?,
            but_testsupport::graph_workspace(ws)
        );
        Ok(())
    };
    eprintln!("initial:");
    dump(&ws, &repo)?;

    // mixed-ops seed 511: unapply A, create fz-1 (anchor None), remove C,
    // create fz-3 (anchor None), unapply fz-1 → BUG: no edge to sever.
    let unapply = |ws: &mut but_graph::Workspace,
                   repo: &gix::Repository,
                   meta: &mut VirtualBranchesTomlMetadata,
                   name: &str|
     -> anyhow::Result<()> {
        let out = but_workspace::branch::unapply(
            r(name),
            ws,
            repo,
            meta,
            but_workspace::branch::unapply::Options {
                workspace_disposition:
                    but_workspace::branch::unapply::WorkspaceDisposition::KeepWorkspaceReference,
            },
        )?;
        ws.adopt(out.workspace);
        Ok(())
    };
    let create = |ws: &mut but_graph::Workspace,
                  repo: &gix::Repository,
                  meta: &mut VirtualBranchesTomlMetadata,
                  name: &str|
     -> anyhow::Result<()> {
        but_workspace::branch::create_reference(
            r(name),
            None,
            repo,
            ws,
            meta,
            test_stack_id,
            None,
        )?;
        let pm = ws.project_meta().clone();
        ws.refresh_from_head(repo, meta, pm)?;
        Ok(())
    };

    eprintln!("step 0: unapply A");
    unapply(&mut ws, &repo, &mut meta, "refs/heads/A")?;
    dump(&ws, &repo)?;

    eprintln!("step 1: create fz-1 (anchor None)");
    create(&mut ws, &repo, &mut meta, "refs/heads/fz-1")?;
    dump(&ws, &repo)?;

    eprintln!("step 2: remove C");
    but_workspace::branch::remove_reference(
        r("refs/heads/C"),
        &repo,
        &ws,
        &mut meta,
        Default::default(),
    )?;
    let pm = ws.project_meta().clone();
    ws.refresh_from_head(&repo, &meta, pm)?;
    dump(&ws, &repo)?;

    eprintln!("step 3: create fz-3 (anchor None)");
    create(&mut ws, &repo, &mut meta, "refs/heads/fz-3")?;
    dump(&ws, &repo)?;

    eprintln!("step 4: unapply fz-1 (expect BUG)");
    match unapply(&mut ws, &repo, &mut meta, "refs/heads/fz-1") {
        Ok(()) => {
            eprintln!("unapply fz-1 OK?!");
            dump(&ws, &repo)?;
        }
        Err(e) => eprintln!("  err: {e:?}"),
    }
    Ok(())
}

// ─── seed-range chunking (FUZZ_FROM / FUZZ_TO override the default sweep) ───

/// The seed range to sweep: `[FUZZ_FROM, FUZZ_TO)` when set, else `[0, default_end)`.
/// Lets long sweeps run in bounded chunks without editing the harness.
fn seed_range(default_end: u64) -> std::ops::Range<u64> {
    let get = |name: &str| std::env::var(name).ok().and_then(|s| s.parse().ok());
    get("FUZZ_FROM").unwrap_or(0)..get("FUZZ_TO").unwrap_or(default_end)
}

// ─── move-after binding oracle ───

/// Random move-commit SEQUENCES with a BINDING oracle: after every successful
/// `move_commit`, the workspace commit's stamped parent binding must be TRUTHFUL —
/// each named slot's ref points exactly at that slot's real parent commit, and when
/// the projection's stack count matches the slot count, each named slot must be
/// attributed to the stack the projection shows owning that leg (catches swapped
/// attribution between stacks sharing a resting commit, which the roster reader's
/// rest-on-slot validation cannot see). Chases the "move --after mis-stamp" known
/// issue from one-engine-handover.md.
#[ignore = "diagnosis driver for the move-after binding mis-stamp"]
#[test]
fn move_after_binding_truthful() {
    use but_rebase::graph_rebase::mutate::InsertSide;

    for (fixture, setup) in MANAGED_SCENARIOS {
        for seed in seed_range(200) {
            let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
            let trace_in = trace.clone();
            let outcome = catch_unwind(AssertUnwindSafe(|| -> anyhow::Result<()> {
                let (_tmp, mut ws, repo, mut meta, _description) =
                    named_writable_scenario_with_description_and_graph(fixture, *setup)?;
                let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
                for _step in 0..8 {
                    let commits = all_commits(&ws);
                    if commits.len() < 2 {
                        break;
                    }
                    let (sb, subject) = commits[rng.random_range(0..commits.len())].clone();
                    let (tb, target) = loop {
                        let c = commits[rng.random_range(0..commits.len())].clone();
                        if c.1 != subject {
                            break c;
                        }
                    };
                    // Above-biased: `--after` is the suspect path.
                    let side = if rng.random_range(0..4u8) < 3 {
                        InsertSide::Above
                    } else {
                        InsertSide::Below
                    };
                    let desc = format!("move {subject} ({sb}) {side:?} {target} ({tb})");
                    trace_in.borrow_mut().push(desc.clone());
                    match managed_editor_op_and_apply(&repo, &mut meta, &mut ws, |editor| {
                        but_workspace::commit::move_commits(
                            editor,
                            [subject],
                            but_rebase::graph_rebase::anchor::Anchor::Commit(target),
                            side,
                        )
                    }) {
                        Ok(()) => {}
                        Err(e) => {
                            let msg = format!("{e:?}");
                            assert!(!msg.contains("BUG:"), "editor invariant, {desc}: {msg}");
                        }
                    }
                }
                Ok(())
            }));
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("fuzz {fixture} seed {seed}: unexpected error: {e:?}"),
                Err(_) => panic!(
                    "fuzz {fixture} seed {seed}: binding oracle violated (panic above) after {:?}",
                    trace.borrow()
                ),
            }
        }
        eprintln!("OK {fixture}");
    }
}

/// Fuzz the BUILDER directly: seeded random repo states — a small commit DAG, branch refs
/// at random commits, a possibly-advanced target, and a managed workspace merge whose
/// parents may DISAGREE with the metadata chains (the mid-flight class the collect.rs docs
/// call out) — projected and round-tripped through the editor. Oracles: the projection's
/// own applied-stacks assert (a panic), editor creation + rebase well-formedness
/// (`BUG:`-classed errors), a no-op editor round-trip touches no ref but the workspace
/// merge itself, and the projected branch set survives materialization unchanged.
#[ignore = "local fuzz harness; run explicitly"]
#[test]
fn fuzz_builder_configs() {
    use but_meta::VirtualBranchesTomlMetadata;
    use rand::{Rng, SeedableRng};
    use std::collections::BTreeMap;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::process::Command;

    fn bash(dir: &std::path::Path, script: &str) {
        let out = Command::new("bash")
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "author")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000")
            .env("GIT_COMMITTER_NAME", "committer")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .env("GIT_COMMITTER_DATE", "2000-01-01 00:00:00 +0000")
            .arg("-euc")
            .arg(script)
            .output()
            .expect("bash runs");
        assert!(
            out.status.success(),
            "script failed: {}\n{}",
            script,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn all_refs(repo: &gix::Repository) -> BTreeMap<String, String> {
        repo.references()
            .expect("iterable")
            .prefixed("refs/")
            .expect("prefix")
            .filter_map(Result::ok)
            .map(|r| (r.name().as_bstr().to_string(), r.id().detach().to_string()))
            .collect()
    }

    const BRANCH_NAMES: &[&str] = &["A", "B", "C", "D", "E", "F"];

    let only: Option<u64> = std::env::var("FUZZ_SEED").ok().and_then(|s| s.parse().ok());
    for seed in seed_range(1600) {
        if only.is_some_and(|s| s != seed) {
            continue;
        }
        // Seeds >= 1600 run an EXTENDED regime: bigger DAGs, more branches, denser
        // merges. Seeds < 1600 keep the historical generator bit-for-bit so old seed
        // numbers stay comparable across sweeps.
        let extended = seed >= 1600;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        // ── generate the repo state ──
        let n_commits = rng.random_range(3..=if extended { 10usize } else { 7usize });
        // parents per commit: (primary, optional merge side), indices into earlier commits
        let mut parents: Vec<(usize, Option<usize>)> = Vec::new();
        for i in 1..n_commits {
            let p1 = rng.random_range(0..i);
            let p2 = (i >= 2 && rng.random_range(0..100) < if extended { 25 } else { 15 })
                .then(|| rng.random_range(0..i))
                .filter(|&p2| p2 != p1);
            parents.push((p1, p2));
        }
        let n_branches = rng.random_range(2..=if extended { 6usize } else { 5usize });
        let branch_at: Vec<usize> = (0..n_branches)
            .map(|_| rng.random_range(0..n_commits))
            .collect();
        let target_at = rng.random_range(0..n_commits);
        let target_advanced = rng.random_range(0..100) < 50;
        let n_ws_parents =
            rng.random_range(1..=if extended { 4usize } else { 3usize }.min(n_commits));
        let mut ws_parents: Vec<usize> = Vec::new();
        while ws_parents.len() < n_ws_parents {
            let c = rng.random_range(0..n_commits);
            if !ws_parents.contains(&c) {
                ws_parents.push(c);
            }
        }
        // chains: shuffle the branches, chop into 1..=3 chunks (4 extended), order as shuffled
        let mut shuffled: Vec<usize> = (0..n_branches).collect();
        for i in (1..shuffled.len()).rev() {
            shuffled.swap(i, rng.random_range(0..=i));
        }
        let n_chains = rng.random_range(1..=if extended { 4usize } else { 3usize }.min(n_branches));
        let mut chains: Vec<Vec<usize>> = vec![Vec::new(); n_chains];
        for (i, b) in shuffled.iter().enumerate() {
            chains[i % n_chains].push(*b);
        }

        let mut script = String::from("git init -q .\ntree=$(printf '' | git mktree)\n");
        script.push_str("c0=$(git commit-tree -m m0 $tree)\n");
        for (i, (p1, p2)) in parents.iter().enumerate() {
            let i = i + 1;
            match p2 {
                Some(p2) => script.push_str(&format!(
                    "c{i}=$(git commit-tree -p $c{p1} -p $c{p2} -m m{i} $tree)\n"
                )),
                None => script.push_str(&format!(
                    "c{i}=$(git commit-tree -p $c{p1} -m m{i} $tree)\n"
                )),
            }
        }
        for (b, at) in branch_at.iter().enumerate() {
            script.push_str(&format!(
                "git update-ref refs/heads/{} $c{at}\n",
                BRANCH_NAMES[b]
            ));
        }
        if target_advanced {
            script.push_str(&format!(
                "t=$(git commit-tree -p $c{target_at} -m advanced $tree)\ngit update-ref refs/remotes/origin/main $t\n"
            ));
        } else {
            script.push_str(&format!(
                "git update-ref refs/remotes/origin/main $c{target_at}\n"
            ));
        }
        let ws_parent_args: String = ws_parents.iter().map(|c| format!("-p $c{c} ")).collect();
        script.push_str(&format!(
            "ws=$(git commit-tree {ws_parent_args}-m 'GitButler Workspace Commit' $tree)\n"
        ));
        script.push_str("git update-ref refs/heads/gitbutler/workspace $ws\n");
        script.push_str("git symbolic-ref HEAD refs/heads/gitbutler/workspace\n");
        // Target wiring as the fixtures do it: a local main at the pre-advance position,
        // a configured origin with a fetch refspec, and main tracking origin/main.
        script.push_str(&format!("git update-ref refs/heads/main $c{target_at}\n"));
        script.push_str(
            "git config remote.origin.url ./fake/local/path\n\
             git config remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'\n\
             git config branch.main.remote origin\n\
             git config branch.main.merge refs/heads/main\n",
        );

        let config = format!(
            "seed {seed}: commits {n_commits} parents {parents:?} branches {branch_at:?} \
             target c{target_at} advanced {target_advanced} ws_parents {ws_parents:?} chains {chains:?}"
        );
        eprintln!("RUN {config}");
        let outcome = catch_unwind(AssertUnwindSafe(|| -> anyhow::Result<()> {
            let tmp = tempfile::tempdir()?;
            bash(tmp.path(), &script);
            let repo = gix::open(tmp.path())?;
            let mut meta =
                VirtualBranchesTomlMetadata::from_path(repo.path().join("virtual-branches.toml"))?;
            for (i, chain) in chains.iter().enumerate() {
                let names: Vec<&str> = chain.iter().map(|&b| BRANCH_NAMES[b]).collect();
                add_stack_with_segments(
                    &mut meta,
                    (i + 1) as u128,
                    names[0],
                    StackState::InWorkspace,
                    &names[1..],
                );
            }
            let before = all_refs(&repo);

            // Oracle 1: projecting never panics (the applied-stacks assert is armed).
            let mut ws = but_graph::Workspace::from_head(
                &repo,
                &meta,
                project_meta(&meta),
                Options::limited(),
            )?;
            if std::env::var_os("FUZZ_PROBE").is_some() {
                // K-arc convergence probe: three TOLERANT round trips — no oracles —
                // reporting the ws-commit trajectory and where it stabilizes.
                let ws_id = |repo: &gix::Repository| -> anyhow::Result<gix::ObjectId> {
                    Ok(repo
                        .find_reference("refs/heads/gitbutler/workspace")?
                        .peel_to_id()?
                        .detach())
                };
                let mut ids = vec![ws_id(&repo)?];
                let mut projs = vec![projected_sorted(&ws)];
                for round in 1..=3 {
                    let outcome = catch_unwind(AssertUnwindSafe(|| -> anyhow::Result<()> {
                        let editor =
                            Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
                        editor.rebase()?.materialize()?;
                        let pm = ws.project_meta().clone();
                        ws.refresh_from_head(&repo, &meta, pm)?;
                        Ok(())
                    }));
                    match outcome {
                        Ok(Ok(())) => {
                            ids.push(ws_id(&repo)?);
                            projs.push(projected_sorted(&ws));
                        }
                        Ok(Err(e)) => {
                            eprintln!("PROBE seed {seed}: ERROR round {round}: {e}");
                            return Ok(());
                        }
                        Err(_) => {
                            eprintln!("PROBE seed {seed}: PANIC round {round}");
                            return Ok(());
                        }
                    }
                }
                let stable_at =
                    (1..ids.len()).find(|&i| ids[i] == ids[i - 1] && projs[i] == projs[i - 1]);
                let short: Vec<String> = ids.iter().map(|id| id.to_string()[..7].into()).collect();
                match stable_at {
                    Some(n) => eprintln!(
                        "PROBE seed {seed}: CONVERGED after {} heal(s) (ws: {})",
                        n - 1,
                        short.join(" -> ")
                    ),
                    None => eprintln!(
                        "PROBE seed {seed}: NON-CONVERGENT (ws: {})",
                        short.join(" -> ")
                    ),
                }
                return Ok(());
            }
            if only.is_some() {
                eprintln!(
                    "--- commits ---\n{}--- workspace ---\n{}",
                    but_testsupport::visualize_commit_graph_all(&repo)?,
                    but_testsupport::graph_workspace(&ws)
                );
            }
            let projected_before = projected_sorted(&ws);
            let dbg_ws_parents = |ws: &but_graph::Workspace, label: &str| {
                if std::env::var("DBG_G").is_err() {
                    return;
                }
                let cg = ws.commit_graph();
                if let Some(layout) = cg.layout() {
                    eprintln!("DBG {label} stored: {:?}", layout.amended_ws_parents);
                }
                if let Some(tip) = ws.tip_commit_id() {
                    eprintln!(
                        "DBG {label} real: {:?}",
                        cg.parents(tip).collect::<Vec<_>>()
                    );
                }
            };
            dbg_ws_parents(&ws, "before");

            // Oracle 2: the editor ingests and rebases without a well-formedness bail.
            let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
            let rebase = editor.rebase()?;
            let (graph, _) = rebase.materialize()?;
            ws.refresh_from_commit_graph(graph, &repo, &meta)?;

            // Oracle 3: a no-op round-trip may only re-merge the workspace commit.
            let after = all_refs(&repo);
            for (name, oid) in &before {
                if name == "refs/heads/gitbutler/workspace" {
                    continue;
                }
                assert_eq!(after.get(name), Some(oid), "no-op round-trip moved {name}");
            }
            assert_eq!(
                before.len(),
                after.len(),
                "no-op round-trip added/removed refs: {before:?} vs {after:?}"
            );

            // Oracle 4: the projected branch set survives materialization.
            let pm = ws.project_meta().clone();
            ws.refresh_from_head(&repo, &meta, pm)?;
            if only.is_some() {
                eprintln!(
                    "--- after commits ---\n{}--- after workspace ---\n{}",
                    but_testsupport::visualize_commit_graph_all(&repo)?,
                    but_testsupport::graph_workspace(&ws)
                );
            }
            dbg_ws_parents(&ws, "after");
            let projected_after = projected_sorted(&ws);
            assert_eq!(
                projected_before, projected_after,
                "the projection changed across a no-op round-trip"
            );

            // Oracle 5 — CONVERGENCE (the K-arc ruling): stored inventions are legal
            // only as one-step transients that the first materialize makes real. A
            // SECOND round trip must be a bit-stable fixed point: the ws commit id
            // and the projection must not change again. Non-convergence is the
            // finding-K core.
            let ws_tip_r1 = repo
                .find_reference("refs/heads/gitbutler/workspace")?
                .peel_to_id()?
                .detach();
            let editor = Editor::create(ws.commit_graph(), ws.project_meta(), &mut meta, &repo)?;
            editor.rebase()?.materialize()?;
            let pm = ws.project_meta().clone();
            ws.refresh_from_head(&repo, &meta, pm)?;
            let ws_tip_r2 = repo
                .find_reference("refs/heads/gitbutler/workspace")?
                .peel_to_id()?
                .detach();
            assert_eq!(
                ws_tip_r1, ws_tip_r2,
                "NON-CONVERGENT: the second no-op round-trip changed the ws commit again"
            );
            assert_eq!(
                projected_after,
                projected_sorted(&ws),
                "NON-CONVERGENT: the second no-op round-trip changed the projection"
            );
            Ok(())
        }));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let msg = format!("{e:?}");
                assert!(
                    !msg.contains("BUG:"),
                    "editor invariant violated for {config}: {msg}"
                );
                panic!("unexpected error for {config}: {msg}");
            }
            Err(_) => panic!("invariant violated (panic above) for {config}"),
        }
    }
}

// ─── harnesses ───

/// Random `move_branch` sequences in single-branch (ad-hoc) mode must never trip an editor
/// well-formedness error (a `BUG:`-classed bail from `rebase()`), never delete/add a branch
/// ref, and never drop a branch from the projection.
#[ignore = "local fuzz harness; run explicitly"]
#[test]
fn fuzz_single_branch_moves() {
    // (fixture, initial tip→base branch order as full ref names)
    let scenarios: &[(&str, &[&str])] = &[
        (
            "single-branch-commit-with-empties",
            &[
                "refs/heads/empty-top",
                "refs/heads/empty-low",
                "refs/heads/commit-branch",
                "refs/heads/single-branch-fixture",
            ],
        ),
        (
            "single-branch-three-branch-stack",
            &[
                "refs/heads/C",
                "refs/heads/B",
                "refs/heads/A",
                "refs/heads/main",
            ],
        ),
    ];

    for (fixture, order) in scenarios {
        for seed in seed_range(300) {
            let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::<(String, String)>::new()));
            let trace_in = trace.clone();
            let outcome = catch_unwind(AssertUnwindSafe(|| -> anyhow::Result<()> {
                let (_tmp, repo, legacy_meta) = named_writable_scenario(fixture)?;
                let project_meta = project_meta(&legacy_meta);
                let mut meta = branch_order_meta(&repo)?;
                meta.set_branch_stack_order(
                    &order.iter().map(|&n| r(n).to_owned()).collect::<Vec<_>>(),
                )?;
                let expected = disk_heads(&repo)?;

                let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
                let mut moves: Vec<(String, String)> = Vec::new();
                for _step in 0..6 {
                    let ws = but_graph::Workspace::from_head(
                        &repo,
                        &meta,
                        project_meta.clone(),
                        Options::limited(),
                    )?;
                    let names = segment_names(&ws);
                    if names.len() < 2 {
                        break;
                    }
                    let subject = names[rng.random_range(0..names.len())].clone();
                    let target = loop {
                        let t = names[rng.random_range(0..names.len())].clone();
                        if t != subject {
                            break t;
                        }
                    };
                    trace_in
                        .borrow_mut()
                        .push((subject.to_string(), target.to_string()));
                    // Unsupported moves bail with an ordinary error — skip. An editor
                    // well-formedness violation bails with a `BUG:`-prefixed error.
                    match move_branch_and_apply(
                        &repo,
                        &mut meta,
                        project_meta.clone(),
                        subject.as_ref(),
                        target.as_ref(),
                    ) {
                        Ok(_) => {
                            moves.push((subject.to_string(), target.to_string()));
                            assert_eq!(
                                disk_heads(&repo)?,
                                expected,
                                "a reorder deleted or added a branch ref, moves {moves:?}"
                            );
                            // The projection must keep showing every branch: a successful
                            // reorder that hides one means metadata and refs diverged.
                            let ws = but_graph::Workspace::from_head(
                                &repo,
                                &meta,
                                project_meta.clone(),
                                Options::limited(),
                            )?;
                            assert_eq!(
                                projected_sorted(&ws),
                                expected,
                                "a reorder dropped a branch from the projection, moves {moves:?}"
                            );
                        }
                        Err(e) => {
                            let msg = format!("{e:?}");
                            assert!(
                                !msg.contains("BUG:"),
                                "editor invariant violated after moves {:?}: {msg}",
                                trace_in.borrow()
                            );
                        }
                    }
                }
                Ok(())
            }));
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("fuzz {fixture} seed {seed}: unexpected error: {e:?}"),
                Err(_) => panic!(
                    "fuzz {fixture} seed {seed}: invariant violated (panic above) after moves {:?}",
                    trace.borrow()
                ),
            }
        }
    }
}

/// Random managed-workspace `move_branch` sequences (within and across stacks) under the
/// same oracles as the single-branch harness.
#[ignore = "local fuzz harness; run explicitly"]
#[test]
fn fuzz_managed_moves() {
    for (fixture, setup) in MANAGED_SCENARIOS {
        for seed in seed_range(150) {
            let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::<(String, String)>::new()));
            let trace_in = trace.clone();
            let outcome = catch_unwind(AssertUnwindSafe(|| -> anyhow::Result<()> {
                let (_tmp, mut ws, repo, mut meta, _description) =
                    named_writable_scenario_with_description_and_graph(fixture, setup)?;
                let expected_heads = disk_heads(&repo)?;
                let expected_projected = projected_sorted(&ws);

                let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
                let mut moves: Vec<(String, String)> = Vec::new();
                for _step in 0..6 {
                    let names = segment_names(&ws);
                    if names.len() < 2 {
                        break;
                    }
                    let subject = names[rng.random_range(0..names.len())].clone();
                    let target = loop {
                        let t = names[rng.random_range(0..names.len())].clone();
                        if t != subject {
                            break t;
                        }
                    };
                    trace_in
                        .borrow_mut()
                        .push((subject.to_string(), target.to_string()));
                    match managed_move_and_apply(
                        &repo,
                        &mut meta,
                        &mut ws,
                        subject.as_ref(),
                        target.as_ref(),
                    ) {
                        Ok(()) => {
                            moves.push((subject.to_string(), target.to_string()));
                            assert_eq!(
                                disk_heads(&repo)?,
                                expected_heads,
                                "a move deleted or added a branch ref, moves {moves:?}"
                            );
                            assert_eq!(
                                projected_sorted(&ws),
                                expected_projected,
                                "a move dropped a branch from the projection, moves {moves:?}"
                            );
                        }
                        Err(e) => {
                            let msg = format!("{e:?}");
                            assert!(
                                !msg.contains("BUG:"),
                                "editor invariant violated after moves {:?}: {msg}",
                                trace_in.borrow()
                            );
                        }
                    }
                }
                Ok(())
            }));
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("fuzz {fixture} seed {seed}: unexpected error: {e:?}"),
                Err(_) => panic!(
                    "fuzz {fixture} seed {seed}: invariant violated (panic above) after moves {:?}",
                    trace.borrow()
                ),
            }
        }
    }
}

/// Random mixed sequences of branch moves, commit moves, squashes, and reference
/// creations/removals over managed workspaces: cross-op interleavings reach editor states
/// no single-op test exercises. Oracles: no `BUG:`-classed errors, the branch ref set on
/// disk matches the tracked expectation (create adds, remove deletes, everything else
/// leaves it alone), and the projection keeps showing every expected branch.
#[ignore = "local fuzz harness; run explicitly"]
#[test]
fn fuzz_managed_mixed_ops() {
    use but_core::ref_metadata::StackId;
    use but_rebase::graph_rebase::mutate::InsertSide;
    use but_workspace::branch::create_reference::{Anchor, Position};

    fn test_stack_id(rn: &gix::refs::FullNameRef) -> StackId {
        StackId::from_number_for_testing(rn.as_bstr().iter().map(|&b| b as u128).sum())
    }

    for (fixture, setup) in MANAGED_SCENARIOS {
        for seed in seed_range(150) {
            eprintln!("RUN {fixture} seed {seed}");
            let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
            let trace_in = trace.clone();
            let outcome = catch_unwind(AssertUnwindSafe(|| -> anyhow::Result<()> {
                let (_tmp, mut ws, repo, mut meta, _description) =
                    named_writable_scenario_with_description_and_graph(fixture, setup)?;
                let mut expected_heads: std::collections::BTreeSet<String> =
                    disk_heads(&repo)?.into_iter().collect();
                // The projection oracle is CONTRACT-scoped per op (asserted in the
                // arms): unapply/apply legitimately move the visibility cone, so
                // exact cross-op set tracking is not predictable from op effects
                // alone. Disk heads stay exactly tracked — every op knows its disk
                // effect.

                let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
                for step in 0..8 {
                    let names = segment_names(&ws);
                    let commits = all_commits(&ws);
                    let op = rng.random_range(0..8u8);
                    let description;
                    let result = match op {
                        0 if names.len() >= 2 => {
                            let subject = names[rng.random_range(0..names.len())].clone();
                            let target = loop {
                                let t = names[rng.random_range(0..names.len())].clone();
                                if t != subject {
                                    break t;
                                }
                            };
                            description = format!("move-branch {subject} -> {target}");
                            eprintln!("  op: {description}");
                            trace_in.borrow_mut().push(description.clone());
                            managed_move_and_apply(
                                &repo,
                                &mut meta,
                                &mut ws,
                                subject.as_ref(),
                                target.as_ref(),
                            )
                        }
                        1 if commits.len() >= 2 => {
                            let (sb, subject) = commits[rng.random_range(0..commits.len())].clone();
                            let (tb, target) = loop {
                                let c = commits[rng.random_range(0..commits.len())].clone();
                                if c.1 != subject {
                                    break c;
                                }
                            };
                            let side = if rng.random_range(0..2u8) == 0 {
                                InsertSide::Above
                            } else {
                                InsertSide::Below
                            };
                            description =
                                format!("move-commit {subject} ({sb}) {side:?} {target} ({tb})");
                            eprintln!("  op: {description}");
                            trace_in.borrow_mut().push(description.clone());
                            managed_editor_op_and_apply(&repo, &mut meta, &mut ws, |editor| {
                                but_workspace::commit::move_commits(
                                    editor,
                                    [subject],
                                    but_rebase::graph_rebase::anchor::Anchor::Commit(target),
                                    side,
                                )
                            })
                        }
                        2 if commits.len() >= 2 => {
                            let (sb, subject) = commits[rng.random_range(0..commits.len())].clone();
                            let (tb, target) = loop {
                                let c = commits[rng.random_range(0..commits.len())].clone();
                                if c.1 != subject {
                                    break c;
                                }
                            };
                            description = format!("squash {subject} ({sb}) into {target} ({tb})");
                            eprintln!("  op: {description}");
                            trace_in.borrow_mut().push(description.clone());
                            managed_editor_op_and_apply(&repo, &mut meta, &mut ws, |editor| {
                                Ok(but_workspace::commit::squash_commits(
                                    editor,
                                    vec![subject],
                                    target,
                                    but_workspace::commit::squash_commits::MessageCombinationStrategy::KeepBoth,
                                )?
                                .rebase)
                            })
                        }
                        3 => {
                            // Create a new reference — half the time anchored to a random
                            // segment (above/below), half the time free-standing.
                            let new_name: gix::refs::FullName =
                                format!("refs/heads/fz-{step}").try_into()?;
                            let anchor = if names.is_empty() || rng.random_range(0..2u8) == 0 {
                                None
                            } else {
                                let seg = names[rng.random_range(0..names.len())].clone();
                                let position = if rng.random_range(0..2u8) == 0 {
                                    Position::Above
                                } else {
                                    Position::Below
                                };
                                Some(Anchor::AtSegment {
                                    ref_name: std::borrow::Cow::Owned(seg),
                                    position,
                                })
                            };
                            description = format!("create {new_name} anchor={anchor:?}");
                            eprintln!("  op: {description}");
                            trace_in.borrow_mut().push(description.clone());
                            match but_workspace::branch::create_reference(
                                new_name.as_ref(),
                                anchor,
                                &repo,
                                &ws,
                                &mut meta,
                                test_stack_id,
                                None,
                            ) {
                                Ok(_) => {
                                    expected_heads.insert(new_name.as_bstr().to_string());
                                    let pm = ws.project_meta().clone();
                                    ws.refresh_from_head(&repo, &meta, pm)?;
                                    assert!(
                                        projected_sorted(&ws)
                                            .contains(&new_name.as_bstr().to_string()),
                                        "created reference must be projected, after {:?}",
                                        trace_in.borrow()
                                    );
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        4 if !names.is_empty() => {
                            let seg = names[rng.random_range(0..names.len())].clone();
                            description = format!("remove {seg}");
                            eprintln!("  op: {description}");
                            trace_in.borrow_mut().push(description.clone());
                            match but_workspace::branch::remove_reference(
                                seg.as_ref(),
                                &repo,
                                &ws,
                                &mut meta,
                                Default::default(),
                            ) {
                                Ok(Some(_)) => {
                                    expected_heads.remove(&seg.as_bstr().to_string());
                                    let pm = ws.project_meta().clone();
                                    ws.refresh_from_head(&repo, &meta, pm)?;
                                    assert!(
                                        !projected_sorted(&ws).contains(&seg.as_bstr().to_string()),
                                        "removed reference must leave the projection, after {:?}",
                                        trace_in.borrow()
                                    );
                                    Ok(())
                                }
                                // Nothing was deleted — a refused/no-op removal.
                                Ok(None) => Ok(()),
                                Err(e) => Err(e),
                            }
                        }
                        5 if !names.is_empty() => {
                            // Unapply a random segment. Under Keep* dispositions the op
                            // must never create or delete a disk branch — the shared
                            // heads assertion below is the oracle. The projection
                            // changes by contract, so it re-baselines from the outcome.
                            let seg = names[rng.random_range(0..names.len())].clone();
                            let disposition = if rng.random_range(0..2u8) == 0 {
                                but_workspace::branch::unapply::WorkspaceDisposition::KeepWorkspaceCommit
                            } else {
                                but_workspace::branch::unapply::WorkspaceDisposition::KeepWorkspaceReference
                            };
                            description = format!("unapply {seg} ({disposition:?})");
                            eprintln!("  op: {description}");
                            trace_in.borrow_mut().push(description.clone());
                            match but_workspace::branch::unapply(
                                seg.as_ref(),
                                &ws,
                                &repo,
                                &mut meta,
                                but_workspace::branch::unapply::Options {
                                    workspace_disposition: disposition,
                                },
                            ) {
                                Ok(out) => {
                                    ws.adopt(out.workspace);
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        6 => {
                            // Apply a random disk branch that isn't projected — an
                            // unapplied stack, an earlier fz-* creation, or a fixture
                            // extra. Apply may create the workspace ref, so both
                            // baselines re-derive from the outcome; the deep oracles
                            // are the BUG check and the roster tripwires.
                            let projected: std::collections::BTreeSet<String> =
                                projected_sorted(&ws).into_iter().collect();
                            let candidates: Vec<String> = disk_heads(&repo)?
                                .into_iter()
                                .filter(|name| {
                                    !projected.contains(name)
                                        && !name.starts_with("refs/heads/gitbutler/")
                                })
                                .collect();
                            if candidates.is_empty() {
                                continue;
                            }
                            let name = candidates[rng.random_range(0..candidates.len())].clone();
                            let name: gix::refs::FullName = name.as_str().try_into()?;
                            description = format!("apply {name}");
                            eprintln!("  op: {description}");
                            trace_in.borrow_mut().push(description.clone());
                            match but_workspace::branch::apply(
                                name.as_ref(),
                                &ws,
                                &repo,
                                &mut meta,
                                but_workspace::branch::apply::Options {
                                    workspace_merge:
                                        but_workspace::branch::apply::WorkspaceMerge::MergeIfNeeded,
                                    on_workspace_conflict: but_workspace::branch::OnWorkspaceMergeConflict::AbortAndReportConflictingStacks,
                                    workspace_reference_naming: Default::default(),
                                    order: None,
                                    new_stack_id: Some(test_stack_id),
                                },
                            ) {
                                Ok(_) => {
                                    let pm = ws.project_meta().clone();
                                    ws.refresh_from_head(&repo, &meta, pm)?;
                                    // Apply may create the managed workspace ref.
                                    expected_heads = disk_heads(&repo)?.into_iter().collect();
                                    assert!(
                                        projected_sorted(&ws)
                                            .contains(&name.as_bstr().to_string()),
                                        "applied branch must be projected, after {:?}",
                                        trace_in.borrow()
                                    );
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        _ => continue,
                    };
                    match result {
                        Ok(()) => {
                            let heads: std::collections::BTreeSet<String> =
                                disk_heads(&repo)?.into_iter().collect();
                            assert_eq!(
                                heads,
                                expected_heads,
                                "disk refs diverged from expectation, after {:?}",
                                trace_in.borrow()
                            );
                        }
                        Err(e) => {
                            let msg = format!("{e:?}");
                            assert!(
                                !msg.contains("BUG:"),
                                "editor invariant violated after {:?}: {msg}",
                                trace_in.borrow()
                            );
                        }
                    }
                }
                Ok(())
            }));
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("fuzz {fixture} seed {seed}: unexpected error: {e:?}"),
                Err(_) => panic!(
                    "fuzz {fixture} seed {seed}: invariant violated (panic above) after ops {:?}",
                    trace.borrow()
                ),
            }
        }
    }
}

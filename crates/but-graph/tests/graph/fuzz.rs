//! Randomized walk+projection invariants: random DAG repositories, random ref and
//! workspace setups, every branch tried as the entrypoint, several pacing options each.
//! What it checks is not shapes (the corpus owns those) but LAWS: `validated()` holds,
//! derivation is deterministic, an unlimited walk is complete, and the display always
//! derives — which also drives every debug-build tripwire across shapes nobody authored.
//!
//! Deterministic by construction: commits are hand-written objects with fixed
//! signatures and monotone timestamps, and the generator is a seeded xorshift.
//! Run explicitly, like the op fuzzers:
//! `cargo test -p but-graph --test graph fuzz_walk_invariants -- --ignored --no-capture`

use but_graph::Workspace;
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::graph_dag;
use gix::prelude::ObjectIdExt;

use crate::walk::utils::{
    StackState, add_stack, add_workspace, default_project_meta, standard_options,
};

/// Good-enough randomness with reproducible seeds.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
}

/// One random repository: a DAG of empty-tree commits plus random local branches,
/// sometimes a remote target, sometimes a managed-looking workspace ref.
struct RandomRepo {
    repo: gix::Repository,
    commits: Vec<gix::ObjectId>,
    /// `(name, tip)` of every local branch, the fuzz entrypoints.
    branches: Vec<(gix::refs::FullName, gix::ObjectId)>,
    /// Raw parent lists as written, the completeness oracle.
    parents: std::collections::HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
    /// Where `origin/main` points, when it exists — history in its cone is BELOW THE
    /// FLOOR and the walk may lawfully withhold it, budget or no budget.
    target_tip: Option<gix::ObjectId>,
    /// A managed workspace merge and the branch indices it merges, when synthesized.
    workspace: Option<(gix::ObjectId, Vec<usize>)>,
}

fn random_repo(rng: &mut XorShift, dir: &std::path::Path) -> anyhow::Result<RandomRepo> {
    let repo = gix::init(dir)?;
    let sig = gix::actor::Signature {
        name: "fuzz".into(),
        email: "fuzz@example.com".into(),
        time: gix::date::Time::new(1_700_000_000, 0),
    };
    let empty_tree = repo.write_object(gix::objs::Tree::empty())?.detach();
    let mut commits: Vec<gix::ObjectId> = Vec::new();
    let mut parents_of = std::collections::HashMap::new();
    let commit_count = 4 + rng.below(36);
    for i in 0..commit_count {
        let mut parents: Vec<gix::ObjectId> = Vec::new();
        if !commits.is_empty() {
            parents.push(commits[rng.below(commits.len())]);
            if rng.chance(30) {
                let extra = commits[rng.below(commits.len())];
                if !parents.contains(&extra) {
                    parents.push(extra);
                }
            }
        }
        let mut sig = sig.clone();
        sig.time.seconds += i as i64;
        let commit = gix::objs::Commit {
            tree: empty_tree,
            parents: parents.iter().copied().collect(),
            author: sig.clone(),
            committer: sig,
            encoding: None,
            message: format!("c{i}").into(),
            extra_headers: Vec::new(),
        };
        let id = repo.write_object(&commit)?.detach();
        parents_of.insert(id, parents);
        commits.push(id);
    }
    let mut branches = Vec::new();
    for b in 0..(2 + rng.below(5)) {
        let name: gix::refs::FullName = format!("refs/heads/b{b}").try_into()?;
        let tip = commits[rng.below(commits.len())];
        repo.reference(
            name.clone(),
            tip,
            gix::refs::transaction::PreviousValue::Any,
            "fuzz",
        )?;
        branches.push((name, tip));
    }
    // Sometimes a MANAGED workspace: a merge of 1-3 branch tips wearing the marker
    // message, under the workspace ref. The caller declares matching metadata stacks.
    let workspace = if rng.chance(50) && !branches.is_empty() {
        let mut members: Vec<usize> = Vec::new();
        for _ in 0..(1 + rng.below(3.min(branches.len()))) {
            let pick = rng.below(branches.len());
            if !members.contains(&pick) {
                members.push(pick);
            }
        }
        let mut sig = sig.clone();
        sig.time.seconds += 1000;
        let merge = gix::objs::Commit {
            tree: empty_tree,
            parents: members.iter().map(|&m| branches[m].1).collect(),
            author: sig.clone(),
            committer: sig,
            encoding: None,
            message: "GitButler Workspace Commit".into(),
            extra_headers: Vec::new(),
        };
        let ws_tip = repo.write_object(&merge)?.detach();
        parents_of.insert(ws_tip, members.iter().map(|&m| branches[m].1).collect());
        repo.reference(
            "refs/heads/gitbutler/workspace",
            ws_tip,
            gix::refs::transaction::PreviousValue::Any,
            "fuzz",
        )?;
        Some((ws_tip, members))
    } else {
        None
    };
    let target_tip = rng.chance(70).then(|| {
        let tip = commits[rng.below(commits.len())];
        repo.reference(
            "refs/remotes/origin/main",
            tip,
            gix::refs::transaction::PreviousValue::Any,
            "fuzz",
        )
        .map(|_| tip)
    });
    let target_tip = target_tip.transpose()?;
    Ok(RandomRepo {
        repo,
        commits,
        branches,
        parents: parents_of,
        target_tip,
        workspace,
    })
}

/// Everything reachable from `tip` over the WRITTEN parent lists — the oracle the
/// graph's contents are checked against.
fn reachable(
    tip: gix::ObjectId,
    parents: &std::collections::HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
) -> std::collections::HashSet<gix::ObjectId> {
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![tip];
    while let Some(id) = stack.pop() {
        if seen.insert(id) {
            stack.extend(parents.get(&id).into_iter().flatten().copied());
        }
    }
    seen
}

#[test]
#[ignore = "run explicitly: many repositories, seconds each"]
fn fuzz_walk_invariants() -> anyhow::Result<()> {
    let iterations: usize = std::env::var("FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150);
    let only: Option<u64> = std::env::var("FUZZ_SEED").ok().and_then(|v| v.parse().ok());
    for seed in only.map(|s| s..=s).unwrap_or(1..=iterations as u64) {
        let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
        let case = random_repo(&mut rng, tmp.path())?;
        let mut meta = VirtualBranchesTomlMetadata::from_path(
            tmp.path().join("should-never-be-written.toml"),
        )?;
        if case.workspace.is_some() || rng.chance(50) {
            add_workspace(&mut meta);
        }
        if let Some((_, members)) = &case.workspace {
            for (order, &m) in members.iter().enumerate() {
                add_stack(
                    &mut meta,
                    order + 1,
                    &format!("b{m}"),
                    StackState::InWorkspace,
                );
            }
        }
        let mut entries: Vec<(gix::refs::FullName, gix::ObjectId)> = case.branches.clone();
        if let Some((ws_tip, _)) = &case.workspace {
            entries.push(("refs/heads/gitbutler/workspace".try_into()?, *ws_tip));
        }
        for (name, tip) in &entries {
            for limit in [None, Some(1 + rng.below(8))] {
                let mut options = standard_options();
                options.commits_limit_hint = limit;
                if rng.chance(20) {
                    options.extra_target_commit_id =
                        Some(case.commits[rng.below(case.commits.len())]);
                }
                let ws = Workspace::from_tip(
                    tip.attach(&case.repo),
                    name.clone(),
                    &meta,
                    default_project_meta(),
                    options.clone(),
                )
                .map_err(|err| {
                    anyhow::anyhow!("seed {seed}, entry {name}, limit {limit:?}: {err}")
                })?
                .validated()
                .map_err(|err| {
                    anyhow::anyhow!("seed {seed}, entry {name}, limit {limit:?}: {err}")
                })?;

                // LAW: the display always derives, whatever the shape.
                ws.display_stacks().map_err(|err| {
                    anyhow::anyhow!("seed {seed}, entry {name}, limit {limit:?}: display: {err}")
                })?;

                // LAW: derivation is a function of its inputs.
                let again = Workspace::from_tip(
                    tip.attach(&case.repo),
                    name.clone(),
                    &meta,
                    default_project_meta(),
                    options,
                )?;
                assert_eq!(
                    graph_dag(&ws),
                    graph_dag(&again),
                    "seed {seed}, entry {name}, limit {limit:?}: derivation must be deterministic"
                );

                // LAW (rule 8): the stored stacks are a PARTITION — no commit is owned
                // by two segments, and every owned commit exists in the arena. (The
                // display may duplicate shared tails; the store must not.)
                let mut owned = std::collections::HashSet::new();
                for stack in &ws.stacks {
                    for segment in &stack.segments {
                        for id in &segment.commits {
                            assert!(
                                owned.insert(*id),
                                "seed {seed}, entry {name}: commit {id} owned twice \
                                 in the stored partition"
                            );
                            assert!(
                                ws.commit_graph().node(*id).is_some(),
                                "seed {seed}, entry {name}: stored partition owns {id} \
                                 which is not in the arena"
                            );
                        }
                    }
                }

                // LAW: the entrypoint's own tip is always collected.
                let graph_ids: std::collections::HashSet<_> =
                    ws.commit_graph().commit_ids().collect();
                assert!(
                    graph_ids.contains(tip),
                    "seed {seed}, entry {name}: the entry tip must always be in the graph"
                );

                // LAW: an unlimited walk withholds ONLY below-floor history — commits in
                // the target's cone, which the integrated exhaust prunes by design. Any
                // other reachable commit must be present.
                if limit.is_none() {
                    let below_floor = case
                        .target_tip
                        .map(|t| reachable(t, &case.parents))
                        .unwrap_or_default();
                    for id in reachable(*tip, &case.parents) {
                        if !graph_ids.contains(&id) && !below_floor.contains(&id) {
                            eprintln!("MISSED {id}; graph:\n{}", graph_dag(&ws));
                            eprintln!("entry {name} tip {tip}");
                            for (i, c) in case.commits.iter().enumerate() {
                                eprintln!("  c{i} {c} <- {:?}", case.parents[c]);
                            }
                            for (bn, bt) in &case.branches {
                                eprintln!("  {bn} -> {bt}");
                            }
                            panic!(
                                "seed {seed}, entry {name}: unlimited walk missed {id} \
                                 reachable from {tip} and not below the floor",
                            );
                        }
                    }
                }
            }
        }
        // The toml metadata writes on plain drop; forget it like the scenario helpers do.
        let _ = std::mem::ManuallyDrop::new(meta);
    }
    Ok(())
}

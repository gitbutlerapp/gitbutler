use std::collections::HashMap;

use anyhow::Result;
use bstr::{BStr, ByteSlice};
use but_core::RefMetadata as _;
use but_graph::Graph;
use but_workspace::branch::create_reference::{Anchor, Position::*};
use but_workspace::ref_info::LocalCommitRelation;
use but_workspace::ui::PushStatus;
use but_workspace::worktrees::{WorktreeBase, WorktreeBranch};

use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack, add_workspace};
use crate::utils::writable_scenario_slow;

/// Build a graph seeded with every active linked worktree of `repo`, the way `but-ctx`
/// does, and project the result.
fn ref_info_with_worktree_tips(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
) -> Result<but_workspace::RefInfo> {
    but_workspace::graph_to_ref_info(
        &workspace_with_worktree_tips(repo, meta)?,
        repo,
        but_workspace::ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        },
    )
}

fn workspace_with_worktree_tips(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
) -> Result<but_graph::Workspace> {
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some(but_core::ref_metadata::LocalRefName::new(
            "refs/heads/main".try_into()?,
        )?),
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        push_remote: None,
    };
    let mut db = but_testsupport::in_memory_db();
    // Adoption already ran, so the fixture worktrees count as active.
    db.worktree_meta_mut().mark_adopted()?;
    let graph = Graph::from_head(
        repo,
        meta,
        project_meta,
        &mut db,
        but_graph::init::Options {
            worktrees: true,
            ..but_graph::init::Options::limited()
        },
    )?
    .validated()?;
    graph.into_workspace()
}

#[test]
fn worktree_orders_skip_deleted_branches() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    but_testsupport::invoke_bash_at_dir(
        "git branch wt-lower A~1",
        repo.workdir().expect("fixture has a working directory"),
    );
    let mut meta = but_meta::BranchOrderMetadata::from_paths(
        repo.path().join("virtual-branches.toml"),
        repo.path(),
    )?;
    let r = |name: &str| crate::utils::r(name).to_owned();
    meta.set_branch_stack_order(&[
        r("refs/heads/wt-inside"),
        r("refs/heads/deleted"),
        r("refs/heads/wt-lower"),
    ])?;

    let ws = workspace_with_worktree_tips(&repo, &meta)?;
    let lane: Vec<_> = ws
        .worktrees
        .iter()
        .find(|worktree| worktree.name == "wt-inside")
        .expect("wt-inside has a lane")
        .segments
        .iter()
        .filter_map(|segment| Some(segment.ref_name()?.shorten().to_string()))
        .collect();
    assert_eq!(
        lane,
        ["wt-inside", "wt-lower"],
        "a deleted branch drops out of the order without cutting off the branches below it"
    );
    Ok(())
}

#[test]
fn worktrees_are_projected_onto_the_workspace() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    add_workspace(&mut meta);
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);

    let mut info = ref_info_with_worktree_tips(&repo, &meta)?;
    let summary: Vec<_> = info
        .worktrees
        .iter()
        .map(|wt| {
            (
                wt.name.to_string(),
                wt.segments
                    .iter()
                    .map(|segment| {
                        (
                            segment
                                .ref_info
                                .as_ref()
                                .map_or("<anon>".to_string(), |ri| {
                                    ri.ref_name.shorten().to_string()
                                }),
                            segment
                                .commits
                                .iter()
                                .map(|c| c.message.trim().as_bstr().to_string())
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>(),
                wt.base,
            )
        })
        .collect();

    let a1 = repo.rev_parse_single("A~1")?.detach();
    let a2 = repo.rev_parse_single("A")?.detach();
    let w1 = repo.rev_parse_single("wt-inside")?.detach();
    let mid1 = repo.rev_parse_single("mid~1")?.detach();
    let m0 = repo.rev_parse_single("main~1")?.detach();
    let m1 = repo.rev_parse_single("main")?.detach();
    let segment = |name: &str, commits: &[&str]| {
        (
            name.to_string(),
            commits.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        )
    };
    assert_eq!(
        summary,
        [
            (
                "wt-at".to_string(),
                // Detached, so anonymous even though it sits right on `A`.
                vec![segment("<anon>", &[])],
                // Its `HEAD` *is* a workspace commit, so it owns nothing and rests right there.
                Some(WorktreeBase::InWorkspace(a2))
            ),
            (
                "wt-below".to_string(),
                vec![segment("wt-below", &["U1"])],
                // Branches off below the target without sitting on the target commit itself -
                // only its base being reachable from the target reveals it is outside.
                Some(WorktreeBase::Outside(m0))
            ),
            (
                "wt-disjoint".to_string(),
                vec![segment("disjoint", &["D1"])],
                // Unrelated history - the walk runs out of graph without finding a base.
                None
            ),
            (
                "wt-inside".to_string(),
                vec![segment("wt-inside", &["W1"])],
                // Its commit branches off a commit that stack A owns.
                Some(WorktreeBase::InWorkspace(a1))
            ),
            (
                "wt-mid".to_string(),
                // Detached in the middle of `mid`: owns the commit below, not the branch.
                vec![segment("<anon>", &["MID1"])],
                Some(WorktreeBase::InWorkspace(a1))
            ),
            (
                "wt-outside".to_string(),
                vec![segment("wt-outside", &["O1"])],
                // The target commit stops the walk before it can reach the workspace.
                Some(WorktreeBase::Outside(m1))
            ),
            (
                "wt-pushed".to_string(),
                vec![segment("wt-pushed", &["P2", "P1"])],
                Some(WorktreeBase::Outside(m1))
            ),
            (
                "wt-stacked".to_string(),
                vec![segment("wt-stacked", &["S1"])],
                // Stacked on wt-inside, which is listed first and thus owns W1 exclusively.
                Some(WorktreeBase::InWorkspace(w1))
            ),
            (
                "wt-top".to_string(),
                // A stack: `mid` is a branch of its own below `top`, cut short by `wt-mid`.
                vec![segment("top", &["TOP1"]), segment("mid", &["MID2"])],
                Some(WorktreeBase::InWorkspace(mid1))
            ),
        ]
    );

    let p1 = repo.rev_parse_single("wt-pushed~1")?.detach();
    let statuses: Vec<_> = info
        .worktrees
        .iter()
        .flat_map(|wt| {
            wt.segments.iter().map(|segment| {
                (
                    wt.name.to_string(),
                    segment.push_status,
                    segment
                        .commits
                        .iter()
                        .map(|c| c.relation)
                        .collect::<Vec<_>>(),
                )
            })
        })
        .collect();
    use LocalCommitRelation::*;
    use PushStatus::*;
    // Push status and remote relations are derived for worktree segments just like for stacks:
    // never-pushed worktree commits must not pretend to be on a remote.
    assert_eq!(
        statuses,
        [
            ("wt-at".to_string(), CompletelyUnpushed, vec![]),
            ("wt-below".to_string(), CompletelyUnpushed, vec![LocalOnly]),
            (
                "wt-disjoint".to_string(),
                CompletelyUnpushed,
                vec![LocalOnly]
            ),
            ("wt-inside".to_string(), CompletelyUnpushed, vec![LocalOnly]),
            ("wt-mid".to_string(), CompletelyUnpushed, vec![LocalOnly]),
            (
                "wt-outside".to_string(),
                CompletelyUnpushed,
                vec![LocalOnly]
            ),
            (
                "wt-pushed".to_string(),
                UnpushedCommits,
                vec![LocalOnly, LocalAndRemote(p1)]
            ),
            (
                "wt-stacked".to_string(),
                CompletelyUnpushed,
                vec![LocalOnly]
            ),
            ("wt-top".to_string(), CompletelyUnpushed, vec![LocalOnly]),
            ("wt-top".to_string(), CompletelyUnpushed, vec![LocalOnly]),
        ]
    );

    info.apply_forge_review_associations(&HashMap::from([(
        "origin/wt-pushed".to_string(),
        (42, true, None),
    )]));
    let pushed = info
        .worktrees
        .iter()
        .find(|wt| wt.name == "wt-pushed")
        .expect("the pushed worktree is projected");
    assert_eq!(
        pushed.segments[0]
            .metadata
            .as_ref()
            .and_then(|meta| meta.review.pull_request),
        Some(42),
        "a worktree branch associates with the review opened from it like a stack branch does"
    );
    Ok(())
}

#[test]
fn lane_chains_follow_what_worktrees_rest_on() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    add_workspace(&mut meta);
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let info = ref_info_with_worktree_tips(&repo, &meta)?;

    let chain = |branch: &str| -> Vec<Vec<String>> {
        let branch = gix::refs::Category::LocalBranch
            .to_full_name(branch)
            .expect("valid fixture branch name");
        info.lane_chain(branch.as_ref())
            .into_iter()
            .map(|(lane, index)| {
                lane.segments_from(index)
                    .iter()
                    .map(|segment| {
                        segment
                            .ref_name()
                            .map_or("<anon>".to_string(), |name| name.shorten().to_string())
                    })
                    .collect()
            })
            .collect()
    };
    let lanes = |lanes: &[&[&str]]| -> Vec<Vec<String>> {
        lanes
            .iter()
            .map(|lane| lane.iter().map(|s| s.to_string()).collect())
            .collect()
    };

    assert_eq!(chain("A"), lanes(&[&["A"]]), "a stack rests on the target");
    assert_eq!(
        chain("wt-outside"),
        lanes(&[&["wt-outside"]]),
        "a worktree based on the target rests on nothing"
    );
    assert_eq!(
        chain("disjoint"),
        lanes(&[&["disjoint"]]),
        "unrelated history has nothing beneath it"
    );
    assert_eq!(
        chain("wt-stacked"),
        lanes(&[&["wt-stacked"], &["wt-inside"], &["A"]]),
        "a worktree on a worktree on a stack walks through both"
    );
    assert_eq!(
        chain("top"),
        lanes(&[&["top", "mid"], &["<anon>"], &["A"]]),
        "the chain enters the detached worktree at its anonymous segment"
    );
    assert_eq!(
        chain("mid"),
        lanes(&[&["mid"], &["<anon>"], &["A"]]),
        "a lower branch of a worktree stack starts its chain at its own segment"
    );
    assert!(
        chain("nope").is_empty(),
        "a branch outside every lane has no chain"
    );
    Ok(())
}

#[test]
fn a_worktree_on_a_padding_commit_rests_on_the_lane_owning_it() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    but_testsupport::invoke_bash_at_dir(
        &format!(
            "git -C wt-inside commit -q --allow-empty -m '{}' && \
             git worktree add -q -b on-padding wt-on-padding wt-inside && \
             git -C wt-on-padding commit -q --allow-empty -m on-padding",
            but_core::commit::padding::TITLE
        ),
        repo.workdir().expect("fixture has a working directory"),
    );
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    add_workspace(&mut meta);
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    let info = ref_info_with_worktree_tips(&repo, &meta)?;

    let chain: Vec<_> = info
        .lane_chain("refs/heads/on-padding".try_into()?)
        .into_iter()
        .map(|(lane, index)| {
            lane.segments_from(index)[0]
                .ref_name()
                .map(|name| name.shorten().to_string())
        })
        .collect();
    assert_eq!(
        chain,
        [
            Some("on-padding".to_string()),
            Some("wt-inside".to_string()),
            Some("A".to_string())
        ],
        "the padding commit is hidden from `wt-inside`, so resting on it means resting on the commit it pads"
    );
    Ok(())
}

#[test]
fn worktrees_are_empty_without_seeded_tips() -> Result<()> {
    let mut db = but_testsupport::in_memory_db();
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    let graph = Graph::from_head(
        &repo,
        &meta,
        Default::default(),
        &mut db,
        but_graph::init::Options::limited(),
    )?;
    let info =
        but_workspace::graph_to_ref_info(&graph.into_workspace()?, &repo, Default::default())?;
    assert!(
        info.worktrees.is_empty(),
        "worktrees are only projected when the traversal was seeded with their tips"
    );
    Ok(())
}

#[test]
fn deep_disjoint_history_is_never_mistaken_for_being_below_the_target() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-disjoint-deep");
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    add_workspace(&mut meta);
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    let info = ref_info_with_worktree_tips(&repo, &meta)?;
    let wt = &info.worktrees[0];
    assert_eq!(wt.name.to_string(), "wt-deep");
    assert_eq!(
        wt.commits()
            .map(|c| c.message.trim().as_bstr().to_string())
            .collect::<Vec<_>>(),
        ["D5", "D4", "D3", "D2", "D1"],
        "every commit of the unrelated history is owned by the worktree"
    );
    assert_eq!(
        wt.base, None,
        "unrelated history has no base, no matter how deep its own chain is"
    );
    Ok(())
}

/// Seconds since the epoch of `2000-01-<day> 00:00:00 +0000`, the committer dates the
/// `worktree-listing` fixture stamps its reflog entries with.
fn day(day: i64) -> i64 {
    946_684_800 + (day - 1) * 86_400
}

#[test]
fn updated_at_is_the_newest_entry_of_the_head_and_branch_reflogs() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let at = |name: &str| {
        but_workspace::worktrees::updated_at(&repo, BStr::new(name)).map(|t| t.map(|t| t.seconds))
    };

    // The day-5 commit inside the worktree is newer than its day-3 checkout.
    assert_eq!(at("wt-a")?, Some(day(5)));
    // The branch was moved from the main checkout on day 6 - only the branch log sees that,
    // the worktree's own HEAD log stops at the day-4 checkout.
    assert_eq!(at("wt-b")?, Some(day(6)));
    // Detached: only the HEAD log exists, written with the default fixture committer date.
    assert_eq!(at("wt-detached")?, Some(day(2)));
    assert_eq!(at("wt-nolog")?, None);
    Ok(())
}

#[test]
fn remove_refuses_dirty_checkouts_without_force() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let path = repo
        .worktree_proxy_by_id(BStr::new("wt-a"))?
        .expect("fixture worktree")
        .base()?;

    let err = but_workspace::worktrees::remove(&repo, BStr::new("wt-a"), false).unwrap_err();
    assert!(err.to_string().contains("--force"), "{err}");
    assert!(path.is_dir(), "a refused removal leaves the checkout alone");

    but_workspace::worktrees::remove(&repo, BStr::new("wt-a"), true)?;
    assert!(!path.exists());
    assert!(
        repo.worktree_proxy_by_id(BStr::new("wt-a"))?.is_none(),
        "the administrative files are gone too"
    );
    Ok(())
}

#[test]
fn remove_overrides_a_lock_and_restores_it_when_removal_is_refused() -> Result<()> {
    use but_testsupport::CommandExt as _;

    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    for name in ["wt-a", "wt-detached"] {
        but_testsupport::git(&repo)
            .args(["worktree", "lock", "--reason", "kept", name])
            .run();
    }
    let worktree = |name: &str| repo.worktree_proxy_by_id(BStr::new(name));

    but_workspace::worktrees::remove(&repo, BStr::new("wt-detached"), false)?;
    assert!(
        worktree("wt-detached")?.is_none(),
        "a lock does not keep a clean checkout from being removed"
    );

    let err = but_workspace::worktrees::remove(&repo, BStr::new("wt-a"), false).unwrap_err();
    assert!(err.to_string().contains("--force"), "{err}");
    assert_eq!(
        worktree("wt-a")?
            .expect("removal was refused")
            .lock_reason(),
        Some("kept".into()),
        "a refused removal leaves the lock as it was"
    );

    but_workspace::worktrees::remove(&repo, BStr::new("wt-a"), true)?;
    assert!(
        worktree("wt-a")?.is_none(),
        "a lock does not keep a dirty checkout from being removed forcibly"
    );
    Ok(())
}

#[test]
fn remove_with_force_overrides_an_unreadable_lock() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let name = BStr::new("wt-a");
    let worktree = repo.worktree_proxy_by_id(name)?.expect("fixture worktree");
    // A directory is a lock marker whose reason cannot be read, on every platform.
    std::fs::create_dir(worktree.git_dir().join("locked"))?;
    assert!(worktree.is_locked(), "the lock marker is present");
    assert!(
        worktree.lock_reason().is_none(),
        "the lock reason is unreadable"
    );

    but_workspace::worktrees::remove(&repo, name, true)?;
    assert!(
        repo.worktree_proxy_by_id(name)?.is_none(),
        "forced removal overrides the lock without reading its reason"
    );
    Ok(())
}

#[test]
#[ignore = "scheduling-sensitive removal stress; run explicitly with --ignored --nocapture"]
fn remove_finishes_when_a_bounded_writer_adds_files_during_deletion() -> Result<()> {
    use std::{
        fs,
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };

    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let path = repo.common_dir().join("concurrent-removal");
    let branch = "refs/heads/concurrent-removal".try_into()?;
    let name = but_workspace::worktrees::add(
        &repo,
        &path,
        WorktreeBranch::New {
            name: branch,
            base: repo.head_id()?.detach(),
        },
    )?;
    let git_dir = repo
        .worktree_proxy_by_id(name.as_bstr())?
        .expect("registered worktree")
        .git_dir()
        .to_owned();
    let marker = path.join("marker");
    fs::write(&marker, b"watch for deletion")?;
    // Keep deletion busy long enough for the writer to observe the marker disappearing.
    for i in 0..1_000 {
        fs::write(path.join(format!("padding-{i}")), b"padding")?;
    }
    let stop = AtomicBool::new(false);
    let (removed, writes) = std::thread::scope(|scope| {
        let writer = scope.spawn(|| -> Result<usize> {
            let deadline = Instant::now() + Duration::from_secs(10);
            while marker.exists() && !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
                std::thread::yield_now();
            }
            let mut writes = 0;
            if !marker.exists() {
                for i in 0..32 {
                    match fs::write(path.join(format!("late-{i}")), b"late write") {
                        Ok(()) => writes += 1,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => break,
                        Err(err) => return Err(err.into()),
                    }
                }
            }
            Ok(writes)
        });
        let removed = but_workspace::worktrees::remove(&repo, name.as_bstr(), true);
        stop.store(true, Ordering::Relaxed);
        (removed, writer.join().expect("writer did not panic"))
    });
    let writes = writes?;
    eprintln!(
        "late writes={writes}, result={removed:?}, checkout={}, administration={}",
        path.exists(),
        git_dir.exists()
    );
    assert!(
        writes > 0,
        "the writer must overlap deletion for this stress test to be meaningful"
    );
    removed?;
    assert!(
        !path.exists() && !git_dir.exists(),
        "both removal roots must disappear after the writer stops"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn add_runs_post_checkout_and_retains_the_worktree_if_the_hook_fails() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let hooks = repo.common_dir().join("hooks");
    std::fs::create_dir_all(&hooks)?;
    let hook = hooks.join("post-checkout");
    std::fs::write(
        &hook,
        br#"#!/bin/sh
printf '%s\n' "$@" > checkout-args
exit 1
"#,
    )?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    let path = repo.common_dir().join("hook-worktree");
    let base = repo.head_id()?.detach();
    let err = but_workspace::worktrees::add(
        &repo,
        &path,
        WorktreeBranch::New {
            name: "refs/heads/hook-worktree".try_into()?,
            base,
        },
    )
    .expect_err("hook failure is reported after checkout");
    assert!(
        err.to_string()
            .contains("Post-checkout hook failed (exit status: 1)"),
        "the exit status explains failure even when the hook writes nothing to stderr: {err}"
    );
    assert!(
        path.join(".git").is_file(),
        "hook failure retains the completed worktree"
    );
    assert_eq!(
        std::fs::read_to_string(path.join("checkout-args"))?,
        format!("{}\n{base}\n1\n", repo.object_hash().null()),
        "worktree hooks run in the new checkout with a null previous HEAD"
    );
    Ok(())
}

#[test]
fn add_checks_out_an_existing_branch_without_moving_it() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let tip = repo.rev_parse_single("HEAD~1")?.detach();
    let branch: &gix::refs::FullNameRef = "refs/heads/loose".try_into()?;
    repo.reference(
        branch,
        tip,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "test",
    )?;

    let path = repo.common_dir().join("loose");
    let name = but_workspace::worktrees::add(&repo, &path, WorktreeBranch::Existing(branch))?;
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, name.as_bstr())?;
    assert_eq!(
        wt_repo.head_name()?.as_ref().map(|name| name.as_ref()),
        Some(branch),
        "the worktree has the existing branch checked out"
    );
    assert_eq!(
        repo.find_reference(branch)?.peel_to_id()?.detach(),
        tip,
        "the existing branch stays where it was"
    );

    let other_path = repo.common_dir().join("loose-again");
    assert!(
        but_workspace::worktrees::add(&repo, &other_path, WorktreeBranch::Existing(branch))
            .is_err(),
        "a branch checked out in a worktree cannot be checked out again"
    );
    let missing = "refs/heads/missing".try_into()?;
    assert!(
        but_workspace::worktrees::add(&repo, &other_path, WorktreeBranch::Existing(missing))
            .is_err(),
        "a missing branch is not created"
    );
    assert!(!other_path.exists(), "refusals leave no checkout");
    Ok(())
}

#[test]
fn add_rejects_existing_branches_and_paths_before_changing_them() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let path = repo.common_dir().join("rejected-worktree");
    let existing = "refs/heads/main".try_into()?;
    assert!(
        but_workspace::worktrees::add(
            &repo,
            &path,
            WorktreeBranch::New {
                name: existing,
                base: repo.head_id()?.detach()
            }
        )
        .is_err(),
        "an existing branch cannot be reset"
    );
    assert!(!path.exists(), "branch refusal leaves no checkout");
    std::fs::create_dir(&path)?;
    let new_branch = "refs/heads/rejected-worktree".try_into()?;
    assert!(
        but_workspace::worktrees::add(
            &repo,
            &path,
            WorktreeBranch::New {
                name: new_branch,
                base: repo.head_id()?.detach()
            }
        )
        .is_err(),
        "existing destination is rejected"
    );
    assert!(
        repo.try_find_reference(new_branch)?.is_none(),
        "destination refusal happens before branch creation"
    );
    Ok(())
}

#[test]
fn add_checks_out_a_new_branch_at_the_base_and_names_the_worktree_after_the_path() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let base = repo.head_id()?.detach();
    let path = repo.common_dir().join("gb-wts").join("wt-new");
    let branch: &gix::refs::FullNameRef = "refs/heads/wt-new".try_into()?;

    let name =
        but_workspace::worktrees::add(&repo, &path, WorktreeBranch::New { name: branch, base })?;
    assert_eq!(
        name, "wt-new",
        "the worktree name comes from the last path component"
    );
    assert_eq!(
        repo.worktree_proxy_by_id(name.as_bstr())?
            .expect("registered")
            .base()?,
        gix::path::realpath(&path)?
    );
    assert_eq!(
        repo.find_reference(branch)?.peel_to_id()?.detach(),
        base,
        "the new branch starts where it was told to"
    );

    let err =
        but_workspace::worktrees::add(&repo, &path, WorktreeBranch::New { name: branch, base })
            .unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
    assert!(
        err.is::<but_workspace::worktrees::DestinationExists>(),
        "occupied destinations are safe to retry at a different path"
    );

    let other_path = path.with_file_name("other");
    let err = but_workspace::worktrees::add(
        &repo,
        &other_path,
        WorktreeBranch::New { name: branch, base },
    )
    .unwrap_err();
    assert!(
        !err.is::<but_workspace::worktrees::DestinationExists>(),
        "an existing branch is not a retryable destination collision"
    );
    assert!(
        !other_path.exists(),
        "branch refusal leaves no checkout behind"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn add_restores_new_directory_permissions_after_copying() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let source = repo.workdir().expect("fixture has a working directory");
    but_testsupport::invoke_bash_at_dir(
        r#"
        set -eu
        mkdir existing
        printf 'tracked\n' > existing/tracked
        printf '/included/\n/existing/artifact\n' > .gitignore
        git add .gitignore existing/tracked
        git commit -qm 'Track existing directory and ignore artifacts'
        mkdir -p included/nested
        printf 'nested artifact\n' > included/nested/artifact
        printf 'existing artifact\n' > existing/artifact
        printf 'included/\nexisting/\n' > .worktreeinclude
        "#,
        source,
    );
    for (path, mode) in [
        ("included", 0o700),
        ("included/nested", 0o500),
        ("existing", 0o500),
    ] {
        std::fs::set_permissions(source.join(path), std::fs::Permissions::from_mode(mode))?;
    }
    let destination = repo.common_dir().join("gb-wts/wt-directory-permissions");
    let branch: &gix::refs::FullNameRef = "refs/heads/wt-directory-permissions".try_into()?;
    let result = but_workspace::worktrees::add(
        &repo,
        &destination,
        WorktreeBranch::New {
            name: branch,
            base: repo.head_id()?.detach(),
        },
    );
    let modes: Vec<_> = ["included", "included/nested", "existing"]
        .map(|path| {
            std::fs::metadata(destination.join(path)).map(|meta| meta.permissions().mode() & 0o777)
        })
        .into_iter()
        .collect();

    // Restore write access before assertions so temporary directories can be cleaned up on failure.
    for root in [source, destination.as_path()] {
        for path in ["included", "included/nested", "existing"] {
            let path = root.join(path);
            if path.is_dir() {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
            }
        }
    }
    result?;
    let modes = modes.into_iter().collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(
        modes[0], 0o700,
        "intermediate new directories retain source permissions"
    );
    assert_eq!(
        modes[1], 0o500,
        "read-only permissions are restored after populating directory"
    );
    assert_ne!(
        modes[2] & 0o200,
        0,
        "existing checkout directory must not inherit source's read-only mode"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join("included/nested/artifact"))?,
        "nested artifact\n"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join("existing/artifact"))?,
        "existing artifact\n"
    );
    Ok(())
}

#[test]
fn add_does_not_copy_nested_destination_into_itself() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let source = repo.workdir().expect("fixture has a working directory");
    but_testsupport::invoke_bash_at_dir(
        r#"
        set -eu
        printf 'tracked contents\n' > tracked
        printf '/included/\n' > .gitignore
        git add tracked .gitignore
        git commit -qm 'Track file and ignore nested worktree location'
        mkdir included
        printf 'source artifact\n' > included/artifact
        printf 'included/new/tracked\nincluded/artifact\n' > .worktreeinclude
        "#,
        source,
    );
    let destination = source.join("included/new");
    let branch: &gix::refs::FullNameRef = "refs/heads/wt-nested-destination".try_into()?;

    // Nested destinations remain usable; only the destination subtree is excluded from copying.
    but_workspace::worktrees::add(
        &repo,
        &destination,
        WorktreeBranch::New {
            name: branch,
            base: repo.head_id()?.detach(),
        },
    )?;

    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join("tracked"))?,
        "tracked contents\n"
    );
    // Select only a leaf from the nested checkout to expose self-copying without allowing
    // an unfixed traversal to recurse indefinitely into its own output.
    assert!(
        !destination.join("included/new").exists(),
        "source traversal must not copy files from the new checkout back into itself"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join("included/artifact"))?,
        "source artifact\n"
    );
    Ok(())
}

#[test]
fn add_copies_only_ignored_directory_selected_by_untracked_worktreeinclude() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let source = repo.workdir().expect("fixture has a working directory");
    but_testsupport::invoke_bash_at_dir(
        r#"
        printf '/included/\n/control/\n' > .gitignore
        git add .gitignore
        git commit -qm 'Ignore local artifact directories'
        mkdir -p included/nested control
        printf 'included artifact\n' > included/nested/artifact
        printf 'control artifact\n' > control/artifact
        printf '/included/\n' > .worktreeinclude
        git check-ignore -q included/nested/artifact
        git check-ignore -q control/artifact
        "#,
        source,
    );
    assert!(
        repo.index()?
            .entry_by_path(".worktreeinclude".into())
            .is_none(),
        "include configuration exists only in the source working directory, not its index"
    );
    let base = repo.head_id()?.detach();
    assert!(
        repo.find_commit(base)?
            .tree()?
            .lookup_entry_by_path(".worktreeinclude")?
            .is_none(),
        "include configuration is not part of the destination commit"
    );
    let destination = repo
        .common_dir()
        .join("gb-wts")
        .join("wt-included-artifacts");
    let branch: &gix::refs::FullNameRef = "refs/heads/wt-included-artifacts".try_into()?;

    but_workspace::worktrees::add(
        &repo,
        &destination,
        WorktreeBranch::New { name: branch, base },
    )?;

    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join(".gitignore"))?,
        "/included/\n/control/\n"
    );
    but_testsupport::invoke_bash_at_dir(
        "git check-ignore -q included/nested/artifact; git check-ignore -q control/artifact",
        &destination,
    );
    assert!(
        !destination.join(".worktreeinclude").exists(),
        "untracked source configuration need not exist in the destination"
    );
    assert!(
        !destination.join("control").exists(),
        "being gitignored is insufficient: the control directory is not included"
    );
    assert!(
        destination.join("included/nested/artifact").is_file(),
        "source .worktreeinclude must copy selected ignored directory contents into the new worktree"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join("included/nested/artifact"))?,
        "included artifact\n"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(source.join("included/nested/artifact"))?,
        "included artifact\n"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(source.join("control/artifact"))?,
        "control artifact\n"
    );
    Ok(())
}

#[test]
fn add_worktreeinclude_copies_only_destination_ignored_files_untracked_in_both_indexes() {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let source = repo.workdir().expect("fixture has a working directory");
    but_testsupport::invoke_bash_at_dir(
        r#"
        set -eu
        mkdir included
        printf '/included/ignored-untracked\n/included/source-tracked\n/included/destination-tracked\n' > .gitignore
        printf 'destination committed contents\n' > included/destination-tracked
        git add .gitignore
        git add -f included/destination-tracked
        git commit -qm 'Destination tracks one ignored file'
        "#,
        source,
    );
    let destination_base = repo.head_id().unwrap().detach();
    but_testsupport::invoke_bash_at_dir(
        r#"
        set -eu
        git rm --cached included/destination-tracked
        printf 'source tracked contents\n' > included/source-tracked
        git add -f included/source-tracked
        git commit -qm 'Source tracks a different ignored file'
        printf 'source contents must not overwrite destination\n' > included/destination-tracked
        printf 'copy this artifact\n' > included/ignored-untracked
        printf 'ordinary untracked contents\n' > included/not-ignored
        printf '/included/\n' > .worktreeinclude
        git check-ignore -q included/ignored-untracked
        if git check-ignore -q included/not-ignored; then
            echo 'included/not-ignored must not be ignored in source' >&2
            exit 1
        fi
        git check-ignore --no-index -q included/source-tracked
        git check-ignore --no-index -q included/destination-tracked
        "#,
        source,
    );
    let source_index = repo.index().unwrap();
    for (path, tracked) in [
        ("included/ignored-untracked", false),
        ("included/not-ignored", false),
        ("included/source-tracked", true),
        ("included/destination-tracked", false),
        (".worktreeinclude", false),
    ] {
        assert_eq!(
            source_index.entry_by_path(path.into()).is_some(),
            tracked,
            "source tracking state for {path}"
        );
    }
    let destination_tree = repo
        .find_commit(destination_base)
        .unwrap()
        .tree_id()
        .unwrap();
    let destination_index = repo.index_from_tree(&destination_tree).unwrap();
    for (path, tracked) in [
        ("included/ignored-untracked", false),
        ("included/not-ignored", false),
        ("included/source-tracked", false),
        ("included/destination-tracked", true),
    ] {
        assert_eq!(
            destination_index.entry_by_path(path.into()).is_some(),
            tracked,
            "destination tracking state for {path}"
        );
    }
    let destination = repo
        .common_dir()
        .join("gb-wts")
        .join("wt-filtered-artifacts");
    let branch: &gix::refs::FullNameRef = "refs/heads/wt-filtered-artifacts".try_into().unwrap();

    but_workspace::worktrees::add(
        &repo,
        &destination,
        WorktreeBranch::New {
            name: branch,
            base: destination_base,
        },
    )
    .unwrap();

    but_testsupport::invoke_bash_at_dir(
        r#"
        set -eu
        git check-ignore --no-index -q included/ignored-untracked
        if git check-ignore --no-index -q included/not-ignored; then
            echo 'included/not-ignored must not be ignored in destination' >&2
            exit 1
        fi
        git check-ignore --no-index -q included/source-tracked
        git check-ignore --no-index -q included/destination-tracked
        "#,
        &destination,
    );
    assert!(
        !destination.join("included/not-ignored").exists(),
        "inclusion alone must not copy an ordinary untracked file"
    );
    assert!(
        !destination.join("included/source-tracked").exists(),
        "files tracked only in source must not be copied"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join("included/destination-tracked")).unwrap(),
        "destination committed contents\n"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(destination.join("included/ignored-untracked")).unwrap(),
        "copy this artifact\n"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(source.join("included/destination-tracked")).unwrap(),
        "source contents must not overwrite destination\n"
    );
}

#[test]
fn references_can_be_created_at_commits_of_worktree_lanes() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("virtual-branches.toml"),
    )?;
    add_workspace(&mut meta);
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    let ws = workspace_with_worktree_tips(&repo, &meta)?;
    snapbox::assert_data_eq!(
        but_testsupport::graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳@repo] <> ✓refs/heads/main on cad9051
├── ≡📙:A on cad9051 {[..]}
│   └── 📙:A
│       ├── ·19f4fc0 (🏘️)
│       └── ·0a62dfe (🏘️)
├── ≡📙:B on cad9051 {[..]}
│   └── 📙:B
│       └── ·5881e28 (🏘️)
├── 📁wt-at on 19f4fc0 (🏘️)
│   └── 📙:anon:
├── 📁wt-below on d4d66e2
│   └── :wt-below[📁]
│       └── ·53aafe9
├── 📁wt-disjoint
│   └── :disjoint[📁wt-disjoint]
│       └── ·3c0fa35
├── 📁wt-inside on 0a62dfe (🏘️)
│   └── :wt-inside[📁]
│       └── ·6a13321
├── 📁wt-mid on 0a62dfe (🏘️)
│   └── :anon:
│       └── ·0bc05bf
├── 📁wt-outside on cad9051
│   └── :wt-outside[📁]
│       └── ·4fcfc93
├── 📁wt-pushed on cad9051
│   └── :wt-pushed[📁] <> origin/wt-pushed⇡1
│       ├── ·198b592
│       └── ❄️88c9775
├── 📁wt-stacked on 6a13321 (🏘️)
│   └── :wt-stacked[📁]
│       └── ·fc6f8f5
└── 📁wt-top on 0bc05bf (🏘️)
    ├── :top[📁wt-top]
    │   └── ·d175c00
    └── :mid
        └── ·b743926

"#]]
    );

    let p1 = repo.rev_parse_single("wt-pushed~1")?.detach();
    let mut db = but_testsupport::in_memory_db();
    db.worktree_meta_mut().mark_adopted()?;
    let mut ws = ws;
    let ws = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut meta,
        &mut db,
        crate::utils::r("refs/heads/below-pushed"),
        Anchor::AtCommit {
            commit_id: p1,
            position: Above,
        },
        None,
    )?;
    snapbox::assert_data_eq!(
        but_testsupport::graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳@repo] <> ✓refs/heads/main on cad9051
├── ≡📙:A on cad9051 {[..]}
│   └── 📙:A
│       ├── ·19f4fc0 (🏘️)
│       └── ·0a62dfe (🏘️)
├── ≡📙:B on cad9051 {[..]}
│   └── 📙:B
│       └── ·5881e28 (🏘️)
├── 📁wt-at on 19f4fc0 (🏘️)
│   └── 📙:anon:
├── 📁wt-below on d4d66e2
│   └── :wt-below[📁]
│       └── ·53aafe9
├── 📁wt-disjoint
│   └── :disjoint[📁wt-disjoint]
│       └── ·3c0fa35
├── 📁wt-inside on 0a62dfe (🏘️)
│   └── :wt-inside[📁]
│       └── ·6a13321
├── 📁wt-mid on 0a62dfe (🏘️)
│   └── :anon:
│       └── ·0bc05bf
├── 📁wt-outside on cad9051
│   └── :wt-outside[📁]
│       └── ·4fcfc93
├── 📁wt-pushed on cad9051
│   ├── :wt-pushed[📁] <> origin/wt-pushed⇡1
│   │   └── ·198b592
│   └── :below-pushed
│       └── ❄88c9775
├── 📁wt-stacked on 6a13321 (🏘️)
│   └── :wt-stacked[📁]
│       └── ·fc6f8f5
└── 📁wt-top on 0bc05bf (🏘️)
    ├── :top[📁wt-top]
    │   └── ·d175c00
    └── :mid
        └── ·b743926

"#]]
    );
    assert!(
        !meta
            .workspace(ws.ref_name().expect("managed"))?
            .contains_ref(
                crate::utils::r("refs/heads/below-pushed"),
                but_core::ref_metadata::StackKind::Applied
            ),
        "worktree lanes have no applied stack to record the new branch in"
    );
    Ok(())
}

#[test]
fn branches_can_be_ordered_in_worktree_lanes() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let mut meta = but_meta::BranchOrderMetadata::from_paths(
        repo.path().join("virtual-branches.toml"),
        repo.path(),
    )?;
    let mut db = but_testsupport::in_memory_db();
    db.worktree_meta_mut().mark_adopted()?;
    let r = crate::utils::r;
    let mut create = |new_ref: &str, anchor: Anchor<'_>, worktree: &str| -> Result<String> {
        let mut ws = workspace_with_worktree_tips(&repo, &meta)?;
        let preview = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            r(new_ref),
            anchor,
            None,
        )?;
        let lane = worktree_lane(&workspace_with_worktree_tips(&repo, &meta)?, worktree);
        assert_eq!(
            worktree_lane(&preview, worktree),
            lane,
            "the editor previews the lane that is read back"
        );
        let head = std::fs::read_to_string(
            repo.common_dir()
                .join("worktrees")
                .join(worktree)
                .join("HEAD"),
        )?;
        let head = head.trim().trim_start_matches("ref: ");
        let order = meta
            .branch_stack_order(r(head))?
            .unwrap_or_default()
            .iter()
            .map(|name| name.shorten().to_string())
            .collect::<Vec<_>>()
            .join(" > ");
        Ok(format!("{head}: {order}\n{lane}"))
    };

    // Above the checked-out branch, the new branch becomes the worktree's checkout.
    snapbox::assert_data_eq!(
        create(
            "refs/heads/above-wt",
            Anchor::at_segment(r("refs/heads/wt-below"), Above),
            "wt-below"
        )?,
        snapbox::str![[r#"
refs/heads/above-wt: above-wt > wt-below
├── 📁wt-below on d4d66e2
│   ├── :above-wt[📁wt-below]
│   └── :wt-below
│       └── ·53aafe9

"#]]
    );
    // Below the checked-out branch, at the same commit.
    snapbox::assert_data_eq!(
        create(
            "refs/heads/below-wt",
            Anchor::at_reference(r("refs/heads/wt-outside"), Below),
            "wt-outside"
        )?,
        snapbox::str![[r#"
refs/heads/wt-outside: wt-outside > below-wt
├── 📁wt-outside on cad9051
│   ├── :wt-outside[📁]
│   └── :below-wt
│       └── ·4fcfc93

"#]]
    );
    // Below a branch owning commits, onto the commit a detached worktree has checked out.
    snapbox::assert_data_eq!(
        create(
            "refs/heads/onto-detached",
            Anchor::at_segment(r("refs/heads/mid"), Below),
            "wt-top"
        )?,
        snapbox::str![[r#"
refs/heads/top: top > mid > onto-detached
└── 📁wt-top on 0bc05bf (🏘️)
    ├── :top[📁wt-top]
    │   └── ·d175c00
    ├── :mid
    │   └── ·b743926
    └── :onto-detached

"#]]
    );
    // Below a branch that isn't checked out, at the same commit.
    snapbox::assert_data_eq!(
        create(
            "refs/heads/below-mid",
            Anchor::at_reference(r("refs/heads/mid"), Below),
            "wt-top"
        )?,
        snapbox::str![[r#"
refs/heads/top: top > mid > below-mid > onto-detached
└── 📁wt-top on 0bc05bf (🏘️)
    ├── :top[📁wt-top]
    │   └── ·d175c00
    ├── :mid
    ├── :below-mid
    │   └── ·b743926
    └── :onto-detached

"#]]
    );
    // Below the worktree's commits, onto a workspace commit that stays in its stack.
    snapbox::assert_data_eq!(
        create(
            "refs/heads/below-inside",
            Anchor::at_segment(r("refs/heads/wt-inside"), Below),
            "wt-inside"
        )?,
        snapbox::str![[r#"
refs/heads/wt-inside: wt-inside > below-inside
├── 📁wt-inside on 0a62dfe (🏘️)
│   ├── :wt-inside[📁]
│   │   └── ·6a13321
│   └── :below-inside

"#]]
    );
    // Ordering branches in worktree lanes leaves the workspace stacks as they were.
    snapbox::assert_data_eq!(
        stacks(&workspace_with_worktree_tips(&repo, &meta)?),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳@repo] <> ✓refs/heads/main on cad9051
├── ≡📙:B on cad9051 {[..]}
│   └── 📙:B
│       └── ·5881e28 (🏘️)
├── ≡📙:A on cad9051 {[..]}
│   └── 📙:A
│       ├── ·19f4fc0 (🏘️)
│       └── ·0a62dfe (🏘️)

"#]]
    );
    Ok(())
}

#[test]
fn worktree_orders_leave_branches_claimed_elsewhere_alone() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    but_testsupport::invoke_bash_at_dir(
        "git worktree add -q -b on-a wt-on-a A && git -C wt-on-a commit -q --allow-empty -m on-A",
        repo.workdir().expect("fixture has a working directory"),
    );
    let mut stacks_meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("virtual-branches.toml"),
    )?;
    add_stack(&mut stacks_meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut stacks_meta, 2, "B", StackState::InWorkspace);
    stacks_meta.write_unreconciled()?;
    let mut meta = but_meta::BranchOrderMetadata::from_paths(stacks_meta.path(), repo.path())?;
    let r = |name: &str| crate::utils::r(name).to_owned();
    // Stale orders reaching into the stack `A` and into the branch `wt-inside` has checked out.
    meta.set_branch_stack_order(&[r("refs/heads/on-a"), r("refs/heads/A")])?;
    meta.set_branch_stack_order(&[r("refs/heads/wt-stacked"), r("refs/heads/wt-inside")])?;
    let ws = workspace_with_worktree_tips(&repo, &meta)?;

    // `A` stays the workspace stack it is listed as.
    snapbox::assert_data_eq!(
        stacks(&ws),
        snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳@repo] <> ✓refs/heads/main on cad9051
├── ≡📙:A on cad9051 {1}
│   └── 📙:A
│       ├── ·19f4fc0 (🏘️)
│       └── ·0a62dfe (🏘️)
├── ≡📙:B on cad9051 {2}
│   └── 📙:B
│       └── ·5881e28 (🏘️)

"#]]
    );
    // Each worktree lane holds only its own branch.
    snapbox::assert_data_eq!(
        worktree_lane(&ws, "wt-on-a")
            + &worktree_lane(&ws, "wt-stacked")
            + &worktree_lane(&ws, "wt-inside"),
        snapbox::str![[r#"
├── 📁wt-on-a on 19f4fc0 (🏘️)
│   └── :on-a[📁wt-on-a]
│       └── ·d9ed4f1
├── 📁wt-stacked on 6a13321 (🏘️)
│   └── :wt-stacked[📁]
│       └── ·fc6f8f5
├── 📁wt-inside on 0a62dfe (🏘️)
│   └── :wt-inside[📁]
│       └── ·6a13321

"#]]
    );
    Ok(())
}

fn worktree_lane(ws: &but_graph::Workspace, worktree: &str) -> String {
    let graph = but_testsupport::graph_workspace(ws).to_string();
    let is_lane_header = |line: &str| line.starts_with("├── 📁") || line.starts_with("└── 📁");
    let header = |line: &str| {
        line.split_once("📁")
            .is_some_and(|(_, rest)| rest.split(' ').next() == Some(worktree))
    };
    graph
        .lines()
        .skip_while(|line| !(is_lane_header(line) && header(line)))
        .enumerate()
        .take_while(|(idx, line)| *idx == 0 || !is_lane_header(line))
        .map(|(_, line)| format!("{line}\n"))
        .collect()
}

fn stacks(ws: &but_graph::Workspace) -> String {
    let graph = but_testsupport::graph_workspace(ws).to_string();
    graph
        .lines()
        .take_while(|line| !line.starts_with("├── 📁") && !line.starts_with("└── 📁"))
        .map(|line| format!("{line}\n"))
        .collect()
}

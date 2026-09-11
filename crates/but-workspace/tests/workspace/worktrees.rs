use std::collections::HashMap;

use anyhow::Result;
use bstr::{BStr, ByteSlice};
use but_graph::Graph;
use but_workspace::ref_info::LocalCommitRelation;
use but_workspace::ui::PushStatus;
use but_workspace::worktrees::WorktreeBase;

use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack, add_workspace};
use crate::utils::writable_scenario_slow;

/// Build a graph seeded with every active linked worktree of `repo`, the way `but-ctx`
/// does when the `worktreeManipulation` flag is on, and project the result.
fn ref_info_with_worktree_tips(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
) -> Result<but_workspace::RefInfo> {
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
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
    but_workspace::graph_to_ref_info(
        &graph.into_workspace()?,
        repo,
        but_workspace::ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        },
    )
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

    info.apply_forge_review_associations(
        &repo,
        &HashMap::from([("wt-pushed".to_string(), (42, true, None))]),
    );
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
fn remove_defers_to_git_for_dirty_checkouts() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let path = repo
        .worktree_proxy_by_id(BStr::new("wt-a"))
        .expect("fixture worktree")
        .base()?;

    let err = but_workspace::worktrees::remove(&repo, &path, false).unwrap_err();
    assert!(err.to_string().contains("--force"), "{err}");
    assert!(path.is_dir(), "a refused removal leaves the checkout alone");

    but_workspace::worktrees::remove(&repo, &path, true)?;
    assert!(!path.exists());
    assert!(
        repo.worktree_proxy_by_id(BStr::new("wt-a")).is_none(),
        "the administrative files are gone too"
    );
    Ok(())
}

#[test]
fn add_checks_out_a_new_branch_at_the_base_and_names_the_worktree_after_the_path() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let base = repo.head_id()?.detach();
    let path = repo.common_dir().join("gb-wts").join("wt-new");
    let branch: &gix::refs::FullNameRef = "refs/heads/wt-new".try_into()?;

    let name = but_workspace::worktrees::add(&repo, &path, branch, base)?;
    assert_eq!(
        name, "wt-new",
        "git names the worktree after the last path component"
    );
    assert_eq!(
        repo.worktree_proxy_by_id(name.as_bstr())
            .expect("registered")
            .base()?,
        gix::path::realpath(&path)?
    );
    assert_eq!(
        repo.find_reference(branch)?.peel_to_id()?.detach(),
        base,
        "the new branch starts where it was told to"
    );

    let err = but_workspace::worktrees::add(&repo, &path, branch, base).unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
    Ok(())
}

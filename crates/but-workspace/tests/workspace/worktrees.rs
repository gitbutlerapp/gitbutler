use anyhow::Result;
use bstr::{BString, ByteSlice};
use but_graph::Graph;
use but_workspace::worktrees::{WorktreeBase, WorktreeSource, list_worktrees};

use crate::utils::writable_scenario_slow;

/// Build [`WorktreeSource`]s with the archived state taken from `db`'s
/// `worktree_meta` table. Unlike `but-ctx`, prunable worktrees are not skipped -
/// [`list_worktrees()`] is agnostic to the caller's enumeration policy.
fn worktree_sources(repo: &gix::Repository, db: &but_db::DbHandle) -> Result<Vec<WorktreeSource>> {
    let mut out = Vec::new();
    for proxy in repo.worktrees()? {
        let name: BString = proxy.id().to_owned();
        let path = proxy.base()?;
        let wt_repo = proxy.into_repo_with_possibly_inaccessible_worktree()?;
        let mut head = wt_repo.head()?;
        let ref_name = head.referent_name().map(ToOwned::to_owned);
        let id = head.peel_to_commit()?.id;
        out.push(WorktreeSource {
            archived: db
                .worktree_meta()
                .get(&name)?
                .is_some_and(|row| row.archived),
            path,
            name,
            ref_name,
            head: id,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Set the archived state the way `but-ctx` does, without its adoption side effect.
fn archive(db: &mut but_db::DbHandle, name: &str, archived: bool) -> Result<()> {
    db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
        name: name.into(),
        archived,
    })?;
    Ok(())
}

#[test]
fn list_worktrees_splits_by_archived_state() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;

    let sources = worktree_sources(&repo, &db)?;
    assert_eq!(
        sources.len(),
        4,
        "the pruned 'wt-gone' still enumerates - only its checkout is missing"
    );
    let listing = list_worktrees(sources);
    assert_eq!(
        listing
            .active
            .iter()
            .map(|wt| wt.name.to_string())
            .collect::<Vec<_>>(),
        ["wt-a", "wt-b", "wt-detached", "wt-gone"],
        "worktrees without explicitly set archived state list as active"
    );
    assert_eq!(listing.archived.len(), 0);

    archive(&mut db, "wt-b", true)?;
    let sources = worktree_sources(&repo, &db)?;
    let listing = list_worktrees(sources);

    let active_names: Vec<_> = listing
        .active
        .iter()
        .map(|wt| wt.name.to_string())
        .collect();
    assert_eq!(
        active_names,
        ["wt-a", "wt-detached", "wt-gone"],
        "non-archived worktrees are listed as stacks, pruned checkouts included"
    );
    let archived_names: Vec<_> = listing
        .archived
        .iter()
        .map(|wt| wt.name.to_string())
        .collect();
    assert_eq!(archived_names, ["wt-b"], "archived worktrees are split out");
    assert_eq!(
        listing.archived[0]
            .ref_name
            .as_ref()
            .map(|name| name.as_bstr().to_string()),
        Some("refs/heads/feat-b".into()),
        "archived worktrees keep their identity information"
    );

    let wt_a = &listing.active[0];
    assert_eq!(
        wt_a.ref_name
            .as_ref()
            .map(|name| name.as_bstr().to_string()),
        Some("refs/heads/feat-a".into())
    );
    assert_eq!(wt_a.head, repo.rev_parse_single("feat-a")?.detach());
    assert_eq!(
        wt_a.path.file_name().and_then(|name| name.to_str()),
        Some("wt-a"),
        "the checkout path is reported"
    );

    let detached = &listing.active[1];
    assert_eq!(
        detached.ref_name, None,
        "a detached-HEAD worktree lists without a ref name"
    );
    assert_eq!(
        detached.head,
        repo.rev_parse_single("@~1")?.detach(),
        "the detached HEAD is resolved directly"
    );

    archive(&mut db, "wt-b", false)?;
    let sources = worktree_sources(&repo, &db)?;
    let listing = list_worktrees(sources);
    assert_eq!(
        listing
            .active
            .iter()
            .map(|wt| wt.name.to_string())
            .collect::<Vec<_>>(),
        ["wt-a", "wt-b", "wt-detached", "wt-gone"],
        "unarchiving brings the worktree back into the active listing"
    );
    Ok(())
}

/// Seed every linked worktree of `repo` as a traversal tip, the way `but-ctx` does when the
/// `worktreeManipulation` flag is on, and project the result.
fn ref_info_with_worktree_tips(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
) -> Result<but_workspace::RefInfo> {
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        push_remote: None,
    };
    let mut options = but_graph::init::Options::limited();
    let mut db = but_testsupport::worktree_db(repo, &[])?;
    options.worktrees = true;
    let graph = Graph::from_head(repo, meta, project_meta, options, &mut db)?.validated()?;
    but_workspace::graph_to_ref_info(
        &graph.into_workspace()?,
        repo,
        but_workspace::ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        },
        &db,
    )
}

#[test]
fn worktrees_are_projected_onto_the_workspace() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    crate::ref_info::with_workspace_commit::utils::add_workspace(&mut meta);
    crate::ref_info::with_workspace_commit::utils::add_stack(
        &mut meta,
        1,
        "A",
        crate::ref_info::with_workspace_commit::utils::StackState::InWorkspace,
    );
    crate::ref_info::with_workspace_commit::utils::add_stack(
        &mut meta,
        2,
        "B",
        crate::ref_info::with_workspace_commit::utils::StackState::InWorkspace,
    );

    let info = ref_info_with_worktree_tips(&repo, &meta)?;
    let summary: Vec<_> = info
        .worktrees
        .iter()
        .map(|wt| {
            (
                wt.name.to_string(),
                wt.commits
                    .iter()
                    .map(|c| c.message.trim().as_bstr().to_string())
                    .collect::<Vec<_>>(),
                wt.base,
            )
        })
        .collect();

    let a1 = repo.rev_parse_single("A~1")?.detach();
    let a2 = repo.rev_parse_single("A")?.detach();
    let m1 = repo.rev_parse_single("main")?.detach();
    assert_eq!(
        summary,
        [
            (
                "wt-at".to_string(),
                Vec::new(),
                // Its `HEAD` *is* a workspace commit, so it owns nothing and rests right there.
                Some(WorktreeBase::InWorkspace(a2))
            ),
            (
                "wt-inside".to_string(),
                vec!["W1".to_string()],
                // Its commit branches off a commit that stack A owns.
                Some(WorktreeBase::InWorkspace(a1))
            ),
            (
                "wt-outside".to_string(),
                vec!["O1".to_string()],
                // The target commit stops the walk before it can reach the workspace.
                Some(WorktreeBase::Outside(m1))
            ),
        ]
    );
    Ok(())
}

#[test]
fn worktrees_are_empty_without_seeded_tips() -> Result<()> {
    let mut db = but_testsupport::in_memory_db()?;
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    let graph = Graph::from_head(
        &repo,
        &meta,
        Default::default(),
        but_graph::init::Options::limited(),
        &mut db,
    )?;
    let info =
        but_workspace::graph_to_ref_info(&graph.into_workspace()?, &repo, Default::default(), &db)?;
    assert!(
        info.worktrees.is_empty(),
        "worktrees are only projected when the traversal was seeded with their tips"
    );
    Ok(())
}

use anyhow::Result;
use bstr::BString;
use but_workspace::worktrees::{WorktreeSource, list_worktrees};

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

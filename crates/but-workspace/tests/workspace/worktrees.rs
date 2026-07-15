use anyhow::Result;
use bstr::BString;
use but_workspace::worktrees::{
    MAX_COMMITS_PER_WORKTREE, WorktreeSource, list_worktrees, set_worktree_archived,
    worktree_changes_by_name,
};

use crate::utils::writable_scenario_slow;

/// Build [`WorktreeSource`]s the way `but-ctx` would, with the archived state taken
/// from `db`'s `worktree_meta` table.
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
            tip: but_graph::init::WorktreeTip { name, ref_name, id },
        });
    }
    out.sort_by(|a, b| a.tip.name.cmp(&b.tip.name));
    Ok(out)
}

#[test]
fn list_worktrees_splits_by_archived_state_and_computes_commits() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    set_worktree_archived(&mut db, "wt-b".into(), true)?;
    let sources = worktree_sources(&repo, &db)?;
    assert_eq!(
        sources.len(),
        3,
        "the pruned 'wt-gone' still enumerates - only its checkout is missing"
    );

    let target_id = repo.head_id()?.detach();
    let listing = list_worktrees(&repo, sources.clone(), Some(target_id))?;

    let active_names: Vec<_> = listing
        .active
        .iter()
        .map(|wt| wt.name.to_string())
        .collect();
    assert_eq!(
        active_names,
        ["wt-a", "wt-gone"],
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

    let base_id = repo.rev_parse_single("@~1")?.detach();
    let wt_a = &listing.active[0];
    assert_eq!(
        wt_a.ref_name
            .as_ref()
            .map(|name| name.as_bstr().to_string()),
        Some("refs/heads/feat-a".into())
    );
    assert_eq!(wt_a.head, repo.rev_parse_single("feat-a")?.detach());
    assert_eq!(
        wt_a.base,
        Some(base_id),
        "the base is the merge base with the target"
    );
    assert_eq!(
        wt_a.commits
            .iter()
            .map(|commit| commit.message.to_string())
            .collect::<Vec<_>>(),
        ["a1\n"],
        "only commits not reachable from the target are listed"
    );
    assert_eq!(
        wt_a.path.file_name().and_then(|name| name.to_str()),
        Some("wt-a"),
        "the checkout path is reported"
    );

    let wt_gone = &listing.active[1];
    assert_eq!(
        wt_gone.commits.len(),
        0,
        "a head that is an ancestor of the target has no own commits"
    );
    assert_eq!(wt_gone.base, Some(base_id));

    let listing = list_worktrees(&repo, sources, None)?;
    let wt_a = &listing.active[0];
    assert_eq!(
        wt_a.commits.len(),
        0,
        "without a target there is no lower bound, so the commit list degrades to empty"
    );
    assert_eq!(wt_a.base, None, "no target, no merge base");
    Ok(())
}

#[test]
fn list_worktrees_caps_commits_and_tolerates_undecodable_commits() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    // 'wt-orphan' checks out a history disjoint from the target, so the target
    // hides nothing and only the cap stops the walk. 'wt-broken' sits on top of
    // an undecodable commit object.
    but_testsupport::invoke_bash(
        r#"
tree=$(git hash-object -w -t tree /dev/null)
parent_arg=()
for i in $(seq 30); do
  commit=$(git commit-tree "$tree" "${parent_arg[@]}" -m "orphan $i")
  parent_arg=(-p "$commit")
done
git branch orphan "$commit"
git worktree add wt-orphan orphan >/dev/null 2>&1

bogus=$(printf 'not a commit' | git hash-object -w -t commit --stdin --literally)
broken=$(git commit-tree "$tree" -p "$bogus" -m "on top of bogus")
git branch broken "$broken"
git worktree add wt-broken broken >/dev/null 2>&1
"#,
        &repo,
    );

    let db = but_db::DbHandle::new_at_path(":memory:")?;
    let sources = worktree_sources(&repo, &db)?;
    let target_id = repo.head_id()?.detach();
    let listing = list_worktrees(&repo, sources, Some(target_id))?;

    let orphan = listing
        .active
        .iter()
        .find(|wt| wt.name == "wt-orphan")
        .expect("the orphan worktree is listed");
    assert_eq!(
        orphan.commits.len(),
        MAX_COMMITS_PER_WORKTREE,
        "the walk of a disjoint history is capped instead of running to the root"
    );
    assert_eq!(
        orphan.commits[0].message.to_string(),
        "orphan 30\n",
        "the walk starts at the worktree head"
    );
    assert_eq!(orphan.base, None, "a disjoint history has no merge base");

    // That `list_worktrees()` succeeded is the main point here: an undecodable
    // commit ends the affected worktree's commit list with a warning instead of
    // failing the entire listing.
    let broken = listing
        .active
        .iter()
        .find(|wt| wt.name == "wt-broken")
        .expect("the broken worktree is listed");
    assert_eq!(
        broken.commits.len(),
        0,
        "the walk ends at the first commit whose parent cannot be decoded - the \
         date-ordered traversal reads parents when yielding a commit"
    );
    Ok(())
}

#[test]
fn worktree_changes_by_name_diffs_linked_worktrees() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");

    let changes = worktree_changes_by_name(&repo, "wt-a".into())?;
    assert_eq!(
        changes
            .changes
            .iter()
            .map(|change| change.path.to_string())
            .collect::<Vec<_>>(),
        ["a1"],
        "the uncommitted change in the linked worktree is found"
    );

    let changes = worktree_changes_by_name(&repo, "wt-b".into())?;
    assert_eq!(changes.changes.len(), 0, "a clean worktree has no changes");

    let err = worktree_changes_by_name(&repo, "wt-gone".into()).unwrap_err();
    assert!(
        err.to_string().contains("wt-gone is not accessible"),
        "pruned checkouts error early instead of diffing nothing: {err}"
    );

    let err = worktree_changes_by_name(&repo, "just-not-there".into()).unwrap_err();
    assert!(
        err.to_string().contains("does not exist"),
        "unknown names error cleanly: {err}"
    );
    Ok(())
}

#[test]
fn set_worktree_archived_upserts_rows() -> Result<()> {
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;

    set_worktree_archived(&mut db, "wt".into(), true)?;
    assert_eq!(
        db.worktree_meta().get(b"wt")?.map(|row| row.archived),
        Some(true),
        "unknown worktrees are adopted on first write"
    );

    set_worktree_archived(&mut db, "wt".into(), false)?;
    assert_eq!(
        db.worktree_meta().get(b"wt")?.map(|row| row.archived),
        Some(false),
        "the same call unarchives"
    );
    Ok(())
}

use std::fmt::Write as _;

use anyhow::Result;
use bstr::{BStr, ByteSlice};
use but_graph::Graph;
use but_workspace::RefInfo;
use but_workspace::worktrees::WorktreeBase;
use snapbox::str;

use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack, add_workspace};
use crate::utils::writable_scenario_slow;

/// Build a graph seeded with every active linked worktree of `repo`, the way `but-ctx`
/// does when the `worktreeManipulation` flag is on, and project the result.
fn ref_info_with_worktree_tips(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
) -> Result<RefInfo> {
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

/// One line per projected worktree with its base and the commits it owns, naming base
/// commits by the first of `revspecs` that resolves to them.
fn render(repo: &gix::Repository, info: &RefInfo, revspecs: &[&str]) -> Result<String> {
    let names = revspecs
        .iter()
        .map(|&spec| Ok((repo.rev_parse_single(spec)?.detach(), spec)))
        .collect::<Result<Vec<_>>>()?;
    let name_of = |id: gix::ObjectId| {
        names
            .iter()
            .find(|(known, _)| *known == id)
            .map_or_else(|| id.to_string(), |(_, name)| name.to_string())
    };
    let mut out = String::new();
    for wt in &info.worktrees {
        let base = match wt.base {
            Some(WorktreeBase::InWorkspace(id)) => format!("InWorkspace({})", name_of(id)),
            Some(WorktreeBase::Outside(id)) => format!("Outside({})", name_of(id)),
            None => "None".to_string(),
        };
        let commits: Vec<_> = wt
            .commits
            .iter()
            .map(|commit| {
                format!(
                    "{} ({:?})",
                    commit.message.trim().as_bstr(),
                    commit.relation
                )
            })
            .collect();
        writeln!(
            out,
            "{}: base={base} commits=[{}]",
            wt.name,
            commits.join(", ")
        )?;
    }
    Ok(out)
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

    let info = ref_info_with_worktree_tips(&repo, &meta)?;
    // wt-at: its `HEAD` *is* a workspace commit, so it owns nothing and rests right there.
    // wt-below: branches off below the target without sitting on the target commit itself -
    //           only its base being reachable from the target reveals it is outside.
    // wt-disjoint: unrelated history - the walk runs out of graph without finding a base.
    // wt-inside: its commit branches off a commit that stack A owns.
    // wt-outside: the target commit stops the walk before it can reach the workspace.
    // wt-stacked: stacked on wt-inside, which is listed first and thus owns W1 exclusively.
    // Never-pushed worktree commits must not pretend to be on a remote.
    snapbox::assert_data_eq!(
        render(&repo, &info, &["A", "A~1", "main", "main~1", "wt-inside"])?,
        str![[r#"
wt-at: base=InWorkspace(A) commits=[]
wt-below: base=Outside(main~1) commits=[U1 (LocalOnly)]
wt-disjoint: base=None commits=[D1 (LocalOnly)]
wt-inside: base=InWorkspace(A~1) commits=[W1 (LocalOnly)]
wt-outside: base=Outside(main) commits=[O1 (LocalOnly)]
wt-stacked: base=InWorkspace(wt-inside) commits=[S1 (LocalOnly)]

"#]]
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
    // Every commit of the unrelated history is owned by the worktree, and it has no base
    // no matter how deep its own chain is.
    snapbox::assert_data_eq!(
        render(&repo, &info, &["A", "main"])?,
        str![[r#"
wt-deep: base=None commits=[D5 (LocalOnly), D4 (LocalOnly), D3 (LocalOnly), D2 (LocalOnly), D1 (LocalOnly)]

"#]]
    );
    Ok(())
}

/// The day of January 2000 that `seconds` since the epoch falls on - the `worktree-listing`
/// fixture stamps its reflog entries with `2000-01-<day> 00:00:00 +0000`.
fn day(seconds: i64) -> i64 {
    (seconds - 946_684_800) / 86_400 + 1
}

#[test]
fn updated_at_is_the_newest_entry_of_the_head_and_branch_reflogs() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let mut listing = String::new();
    for name in ["wt-a", "wt-b", "wt-detached", "wt-nolog"] {
        let updated_at = but_workspace::worktrees::updated_at(&repo, BStr::new(name))?;
        writeln!(
            listing,
            "{name}: {}",
            updated_at.map_or("none".to_string(), |time| format!(
                "day {}",
                day(time.seconds)
            ))
        )?;
    }
    // wt-a: the day-5 commit inside the worktree is newer than its day-3 checkout.
    // wt-b: the branch was moved from the main checkout on day 6 - only the branch log sees
    //       that, the worktree's own HEAD log stops at the day-4 checkout.
    // wt-detached: only the HEAD log exists, written with the default fixture committer date.
    snapbox::assert_data_eq!(
        listing,
        str![[r#"
wt-a: day 5
wt-b: day 6
wt-detached: day 2
wt-nolog: none

"#]]
    );
    Ok(())
}

/// Every worktree git knows administrative files for, and whether its checkout is on disk.
fn worktree_listing(repo: &gix::Repository) -> Result<String> {
    let mut out = String::new();
    for proxy in repo.worktrees()? {
        let checkout = if proxy.base()?.is_dir() {
            "checked out"
        } else {
            "missing"
        };
        writeln!(out, "{}: {checkout}", proxy.id())?;
    }
    Ok(out)
}

#[test]
fn remove_defers_to_git_for_dirty_checkouts() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-listing");
    let path = repo
        .worktree_proxy_by_id(BStr::new("wt-a"))
        .expect("fixture worktree")
        .base()?;
    snapbox::assert_data_eq!(
        worktree_listing(&repo)?,
        str![[r#"
wt-a: checked out
wt-b: checked out
wt-detached: checked out
wt-gone: missing
wt-nolog: checked out

"#]]
    );

    let err = but_workspace::worktrees::remove(&repo, &path, false).unwrap_err();
    snapbox::assert_data_eq!(
        format!("{err:#}"),
        str!["fatal: '[..]/wt-a' contains modified or untracked files, use --force to delete it"]
    );
    // A refused removal leaves the checkout alone.
    snapbox::assert_data_eq!(
        worktree_listing(&repo)?,
        str![[r#"
wt-a: checked out
wt-b: checked out
wt-detached: checked out
wt-gone: missing
wt-nolog: checked out

"#]]
    );

    but_workspace::worktrees::remove(&repo, &path, true)?;
    // The administrative files are gone too.
    snapbox::assert_data_eq!(
        worktree_listing(&repo)?,
        str![[r#"
wt-b: checked out
wt-detached: checked out
wt-gone: missing
wt-nolog: checked out

"#]]
    );
    Ok(())
}

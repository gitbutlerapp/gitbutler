use anyhow::Result;
use but_api::commit::json::ChangesSource;

use crate::support::{checkout_branch_in_linked_worktree, repo_with_feature_branch, write_file};

/// Discard `file.txt` as a whole file.
fn whole_file() -> Vec<but_core::DiffSpec> {
    vec![but_core::DiffSpec {
        previous_path: None,
        path: "file.txt".into(),
        hunk_headers: vec![],
    }]
}

#[test]
fn worktree_source_discards_in_the_linked_checkout_only() -> Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    but_api::worktrees::worktree_set_archived(&mut ctx, "wt".into(), false)?;

    let linked = linked.path().join("wt");
    write_file(tmp.path(), "file.txt", "main worktree change\n")?;
    write_file(&linked, "file.txt", "linked change\n")?;

    let refused = but_api::legacy::workspace::discard_worktree_changes(
        &mut ctx,
        ChangesSource::Worktree("wt".into()),
        whole_file(),
    )?;

    assert!(refused.is_empty(), "the whole file can be discarded");
    assert_eq!(
        std::fs::read_to_string(linked.join("file.txt"))?,
        "one\n",
        "the linked worktree's file is back at its branch's version"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("file.txt"))?,
        "main worktree change\n",
        "discarding a linked worktree's file must not touch the main checkout"
    );
    Ok(())
}

#[test]
fn head_source_discards_in_the_main_checkout_only() -> Result<()> {
    let (repo, tmp) = repo_with_feature_branch()?;
    let linked = checkout_branch_in_linked_worktree(tmp.path(), "feature")?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();

    let linked = linked.path().join("wt");
    write_file(tmp.path(), "file.txt", "main worktree change\n")?;
    write_file(&linked, "file.txt", "linked change\n")?;

    let refused = but_api::legacy::workspace::discard_worktree_changes(
        &mut ctx,
        ChangesSource::Head,
        whole_file(),
    )?;

    assert!(refused.is_empty(), "the whole file can be discarded");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("file.txt"))?,
        "two\n",
        "the main checkout's file is back at its branch's version"
    );
    assert_eq!(
        std::fs::read_to_string(linked.join("file.txt"))?,
        "linked change\n",
        "discarding in the main checkout must not touch the linked worktree"
    );
    Ok(())
}

use super::*;
use but_testsupport::{CommandExt, git_at_dir, open_repo};

#[test]
fn failed_sql_commit_restores_attached_and_detached_linked_worktrees() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["branch"]);
    // Adoption has already run, so both newly added worktrees participate in rewrites.
    env.db().worktree_meta_mut().mark_adopted()?;
    let linked = but_testsupport::gix_testtools::tempfile::tempdir()?;
    let mut before = Vec::new();
    for detached in [false, true] {
        let path = linked
            .path()
            .join(if detached { "detached" } else { "attached" });
        let mut command = git_at_dir(env.projects_root());
        command.args(["worktree", "add"]);
        if detached {
            command.arg("--detach");
        }
        command.arg(&path).arg("branch").run();
        std::fs::write(path.join("file-one"), "staged contents\n")?;
        git_at_dir(&path).args(["add", "file-one"]).run();
        std::fs::write(path.join("file-one"), "staged and unstaged contents\n")?;
        std::fs::write(path.join("untracked"), "keep untracked contents\n")?;
        let repo = open_repo(&path)?;
        before.push((
            path.clone(),
            repo.head_name()?,
            repo.head_id()?.detach(),
            std::fs::read(repo.index_path())?,
            std::fs::read(path.join("file-three"))?,
        ));
    }
    let mut ctx =
        Context::from_repo_for_testing(open_repo(env.projects_root())?)?.with_memory_app_cache();
    ctx.settings.feature_flags.worktree_manipulation = true;
    {
        let (_guard, _repo, _workspace, _db) = ctx.workspace_and_db()?;
    }
    let observer = env.db();
    let metadata_before = observer.virtual_branches().get_snapshot()?;
    let order_before = observer.branch_order().get_snapshot()?;
    let branch_before = open_repo(env.projects_root())?
        .rev_parse_single("branch")?
        .detach();
    let new_ref: FullName = "refs/heads/transaction-only".try_into()?;
    let sql = rusqlite::Connection::open(ctx.project_data_dir.join("but.sqlite"))?;
    sql.execute_batch(
        "CREATE TABLE late_failure_parent(id INTEGER PRIMARY KEY);
         CREATE TABLE late_failure_child(parent INTEGER REFERENCES late_failure_parent(id)
             DEFERRABLE INITIALLY DEFERRED);
         CREATE TRIGGER fail_at_commit AFTER INSERT ON vb_stack_heads
         WHEN NEW.name = 'transaction-only'
         BEGIN INSERT INTO late_failure_child VALUES (1); END;",
    )?;
    let error = with_transaction(
        &mut ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        DryRun::No,
        |mut tx| {
            // Rewriting the tip removes file-three from both linked checkouts before COMMIT.
            tx.discard_changes_from_commit(branch_before, vec![diff_spec_for_file("file-three")])?;
            tx.create_reference(
                new_ref.as_ref(),
                None,
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;
            Ok(())
        },
    )
    .expect_err("a deferred foreign-key violation rejects the final SQL commit");
    assert!(
        format!("{error:#}").contains("FOREIGN KEY constraint failed"),
        "the operation reaches SQL commit after linked checkout materialization: {error:#}"
    );
    for (path, head_name, head_id, index, file_three) in before {
        let repo = open_repo(&path)?;
        assert_eq!(
            repo.head_name()?,
            head_name,
            "rollback preserves linked HEAD attachment"
        );
        assert_eq!(
            repo.head_id()?.detach(),
            head_id,
            "rollback restores the linked HEAD commit"
        );
        assert_eq!(
            std::fs::read(path.join("file-three"))?,
            file_three,
            "rollback restores a file removed by the rewritten commit"
        );
        assert_eq!(
            std::fs::read_to_string(path.join("file-one"))?,
            "staged and unstaged contents\n",
            "rollback preserves linked staged and unstaged file contents"
        );
        assert_eq!(
            std::fs::read_to_string(path.join("untracked"))?,
            "keep untracked contents\n",
            "rollback preserves linked untracked contents"
        );
        assert_eq!(
            std::fs::read(repo.index_path())?,
            index,
            "rollback restores the original linked index exactly"
        );
    }
    assert_eq!(
        observer.virtual_branches().get_snapshot()?,
        metadata_before,
        "metadata rolls back with linked worktrees"
    );
    assert_eq!(
        observer.branch_order().get_snapshot()?,
        order_before,
        "branch order rolls back with linked worktrees"
    );
    assert!(
        open_repo(env.projects_root())?
            .try_find_reference(new_ref.as_ref())?
            .is_none(),
        "rollback removes the reference created by the failed transaction"
    );
    assert_num_snapshots(&ctx, 0);
    Ok(())
}

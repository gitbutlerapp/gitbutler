use super::*;
use but_testsupport::CommandExt;

#[test]
fn ref_only_transactions_do_not_store_untracked_contents() -> anyhow::Result<()> {
    use gix::objs::Exists as _;

    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["branch"]);
    let contents = "untracked content that a ref-only transaction must leave alone\n";
    env.file("untracked", contents);
    let repo = but_testsupport::open_repo(env.projects_root())?;
    let blob = gix::objs::compute_hash(
        repo.object_hash(),
        gix::object::Kind::Blob,
        contents.as_bytes(),
    )?;
    let branch = repo.rev_parse_single("branch")?.detach();
    let index = std::fs::read(repo.index_path())?;
    assert!(
        !repo.objects.exists(&blob),
        "the untracked blob is not stored before the operation"
    );
    let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let mut guard = ctx.exclusive_worktree_access();
    crate::with_transaction_with_perm_only(
        &mut ctx,
        guard.write_permission(),
        DryRun::No,
        |mut tx| tx.uncommit_commits([branch]),
    )?;
    let repo = env.open_repo();
    assert!(
        !repo.objects.exists(&blob),
        "a ref-only operation does not store unrelated worktree contents"
    );
    assert_eq!(
        std::fs::read(repo.index_path())?,
        index,
        "uncommitting preserves the exact index"
    );
    assert_eq!(
        env.read_file("untracked")?,
        contents,
        "unrelated contents remain untouched"
    );
    Ok(())
}

#[test]
fn failed_sql_commit_restores_git_and_preserves_dirty_index_and_worktree() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["branch"]);
    env.file("file-one", "staged contents\n");
    but_testsupport::git_at_dir(env.projects_root())
        .args(["add", "file-one"])
        .run();
    env.file("file-one", "staged and unstaged contents\n");
    env.file("untracked", "keep untracked contents\n");
    let repo = but_testsupport::open_repo(env.projects_root())?;
    let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    {
        let (_guard, _repo, _workspace, _db) = ctx.workspace_and_db()?;
    }
    let observer = env.db();
    let metadata_before = observer.meta()?;
    let order_before = observer.branch_order().get_snapshot()?;
    let repo = but_testsupport::open_repo(env.projects_root())?;
    let head_before = repo.head_name()?.expect("fixture HEAD is symbolic");
    let branch: FullName = "refs/heads/branch".try_into()?;
    let branch_before = repo.rev_parse_single(branch.as_ref())?.detach();
    let head_id_before = repo.head_id()?.detach();
    let index_before = std::fs::read(repo.index_path())?;
    let new_ref: FullName = "refs/heads/transaction-only".try_into()?;
    let unrelated_ref: FullName = "refs/remotes/origin/unrelated".try_into()?;
    repo.reference(
        unrelated_ref.as_ref(),
        head_id_before,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "set unrelated ref",
    )?;
    let refresh = ctx.project_data_dir.join("REFRESH");
    if refresh.exists() {
        std::fs::remove_file(&refresh)?;
    }
    assert_num_snapshots(&ctx, 0);
    let sql = rusqlite::Connection::open(ctx.project_data_dir.join("but.sqlite"))?;
    sql.execute_batch(
        "CREATE TABLE late_failure_parent(id INTEGER PRIMARY KEY);
         CREATE TABLE late_failure_child(parent INTEGER REFERENCES late_failure_parent(id)
             DEFERRABLE INITIALLY DEFERRED);
         CREATE TRIGGER fail_at_commit AFTER INSERT ON branch_metadata
         WHEN NEW.ref_name = CAST('refs/heads/transaction-only' AS BLOB)
         BEGIN INSERT INTO late_failure_child VALUES (1); END;",
    )?;
    let error = with_transaction(
        &mut ctx,
        SnapshotDetails::new(OperationKind::CreateCommit),
        DryRun::No,
        |mut tx| {
            let created = tx.create_commit(
                RelativeTo::Reference(branch.clone()),
                InsertSide::Above,
                vec![diff_spec_for_file("file-one")],
                "consume dirty file".into(),
                but_workspace::commit::ChangeSource::Head,
            )?;
            assert!(
                created.rejected_specs.is_empty(),
                "the selected dirty file is committed"
            );
            created.new_commit.expect("one commit was created");
            tx.create_reference(
                new_ref.as_ref(),
                None,
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;
            tx.checkout(new_ref.as_ref())?;
            tx.repo().reference(
                unrelated_ref.as_ref(),
                branch_before,
                gix::refs::transaction::PreviousValue::MustExistAndMatch(
                    gix::refs::Target::Object(head_id_before),
                ),
                "independent ref change",
            )?;
            Ok(())
        },
    )
    .expect_err("a deferred foreign-key violation rejects the final SQL commit");
    assert!(
        format!("{error:#}").contains("FOREIGN KEY constraint failed"),
        "the operation reaches SQL commit after Git materialization: {error:#}"
    );
    let repo = but_testsupport::open_repo(env.projects_root())?;
    assert_eq!(
        repo.head_name()?,
        Some(head_before),
        "rollback restores symbolic HEAD"
    );
    assert_eq!(
        repo.head_id()?.detach(),
        head_id_before,
        "rollback restores the workspace commit"
    );
    assert_eq!(
        repo.rev_parse_single(branch.as_ref())?.detach(),
        branch_before,
        "rollback restores rewritten branch refs"
    );
    assert!(
        repo.try_find_reference(new_ref.as_ref())?.is_none(),
        "rollback removes its created branch"
    );
    assert_eq!(
        repo.rev_parse_single(unrelated_ref.as_ref())?.detach(),
        branch_before,
        "recovery leaves independent reference changes intact"
    );
    assert_eq!(observer.meta()?, metadata_before, "metadata rolls back");
    assert_eq!(
        observer.branch_order().get_snapshot()?,
        order_before,
        "branch order rolls back"
    );
    assert!(!refresh.exists(), "rollback emits no refresh");
    assert_num_snapshots(&ctx, 0);
    assert_eq!(
        env.read_file("file-one")?,
        "staged and unstaged contents\n",
        "dirty contents survive"
    );
    assert_eq!(
        env.read_file("untracked")?,
        "keep untracked contents\n",
        "untracked contents survive"
    );
    assert_eq!(
        std::fs::read(repo.index_path())?,
        index_before,
        "the original index is restored exactly"
    );
    let fresh_ctx =
        Context::from_repo_for_testing(but_testsupport::open_repo(env.projects_root())?)?
            .with_memory_app_cache();
    let (_fresh_guard, _fresh_repo, fresh_workspace, _fresh_db) = fresh_ctx.workspace_and_db()?;
    let (_guard, _repo, workspace, _db) = ctx.workspace_and_db()?;
    assert_eq!(
        but_testsupport::graph_workspace(&workspace).to_string(),
        but_testsupport::graph_workspace(&fresh_workspace).to_string(),
        "a failed transaction cannot leave its rewritten workspace in the context cache"
    );
    Ok(())
}

#[test]
fn failed_metadata_refresh_restores_materialized_refs_and_worktree() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["branch"]);
    let repo = but_testsupport::open_repo(env.projects_root())?;
    let original_head = repo.head_id()?.detach();
    let original_branch = repo.rev_parse_single("branch")?.detach();
    let file_three = env.read_file("file-three")?;
    let original_index = std::fs::read(repo.index_path())?;
    let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let metadata_before = env.db().meta()?;
    let sql = rusqlite::Connection::open(ctx.project_data_dir.join("but.sqlite"))?;
    sql.execute_batch(
        "CREATE TRIGGER corrupt_metadata AFTER INSERT ON branch_metadata
         WHEN NEW.ref_name = CAST('refs/heads/corrupt-metadata' AS BLOB)
         BEGIN UPDATE workspace_stacks SET relation = 'merge-from', merge_commit = X'00'; END;",
    )?;
    let error = with_transaction(
        &mut ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        DryRun::No,
        |mut tx| {
            tx.discard_changes_from_commit(
                original_branch,
                vec![diff_spec_for_file("file-three")],
            )?;
            let (_, db) = tx
                .inner
                .rebase
                .as_mut()
                .expect("rebase exists")
                .repo_and_db_mut();
            // The trigger stores an invalid object ID, rejected by the refresh after checkout.
            db.meta_mut()?.set_branch(
                "refs/heads/corrupt-metadata".try_into()?,
                &Default::default(),
            )?;
            Ok(())
        },
    )
    .expect_err("workspace refresh rejects malformed metadata after materialization");
    assert!(
        format!("{error:#}").contains("Invalid workspace merge commit"),
        "the metadata error is preserved: {error:#}"
    );
    let repo = but_testsupport::open_repo(env.projects_root())?;
    assert_eq!(
        repo.head_id()?.detach(),
        original_head,
        "late refresh errors restore workspace HEAD"
    );
    assert_eq!(
        repo.rev_parse_single("branch")?.detach(),
        original_branch,
        "receipts survive a failing materialize call"
    );
    assert_eq!(
        env.read_file("file-three")?,
        file_three,
        "rollback restores the checked-out file"
    );
    assert_eq!(
        std::fs::read(repo.index_path())?,
        original_index,
        "rollback restores staging after a refresh error"
    );
    assert_eq!(
        env.db().meta()?,
        metadata_before,
        "malformed uncommitted metadata is rolled back"
    );
    assert_num_snapshots(&ctx, 0);
    Ok(())
}

#[test]
fn failed_ref_write_restores_a_completed_checkout() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["branch"]);
    let repo = but_testsupport::open_repo(env.projects_root())?;
    let branch = repo.rev_parse_single("branch")?.detach();
    let head = repo.head_id()?.detach();
    let index = std::fs::read(repo.index_path())?;
    let file_three = env.read_file("file-three")?;
    let lock_path = repo.git_dir().join("refs/heads/branch.lock");
    let mut ctx = Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let error = with_transaction(
        &mut ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
        DryRun::No,
        |mut tx| {
            tx.discard_changes_from_commit(branch, vec![diff_spec_for_file("file-three")])?;
            std::fs::write(&lock_path, "")?;
            Ok(())
        },
    )
    .expect_err("the ref lock rejects materialization after checkout");
    std::fs::remove_file(lock_path)?;
    assert!(
        format!("{error:#}").contains("lock"),
        "the original Git ref failure is preserved: {error:#}"
    );
    let repo = but_testsupport::open_repo(env.projects_root())?;
    assert_eq!(
        repo.head_id()?.detach(),
        head,
        "failed ref writes leave HEAD unchanged"
    );
    assert_eq!(
        repo.rev_parse_single("branch")?.detach(),
        branch,
        "the locked branch is unchanged"
    );
    assert!(
        env.projects_root().join("file-three").exists(),
        "recovery restores the completed checkout even though HEAD never moved"
    );
    assert_eq!(
        env.read_file("file-three")?,
        file_three,
        "the removed file is restored exactly"
    );
    assert_eq!(
        std::fs::read(repo.index_path())?,
        index,
        "recovery restores the original index"
    );
    assert_num_snapshots(&ctx, 0);
    Ok(())
}

#[test]
fn recovery_refuses_independent_staging_and_ref_changes_but_restores_other_refs()
-> anyhow::Result<()> {
    use gix::refs::transaction::{PreviousValue, RefEdit};
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    let repo = but_testsupport::open_repo(env.projects_root())?;
    let head = repo.head_id()?.detach();
    let target = repo.rev_parse_single("main")?.detach();
    let branch = repo.rev_parse_single("branch")?.detach();
    let mut pending = crate::PendingRefChanges::default();
    pending.capture_checkouts(&repo, &[])?;
    but_core::worktree::safe_checkout_from_head(
        target,
        &repo,
        but_core::worktree::checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )?;
    pending
        .checkouts
        .first_mut()
        .expect("main worktree captured")
        .record_checkout(target)?;
    let changed: FullName = "refs/heads/changed-independently".try_into()?;
    let restorable: FullName = "refs/heads/restorable".try_into()?;
    pending.committed.extend(repo.edit_references([
        RefEdit::update(
            repo.head_name()?.expect("fixture HEAD is symbolic"),
            target,
            PreviousValue::Any,
            "checkout",
        ),
        RefEdit::update(changed.clone(), head, PreviousValue::MustNotExist, "create"),
        RefEdit::update(
            restorable.clone(),
            head,
            PreviousValue::MustNotExist,
            "create",
        ),
    ])?);
    pending.record_materialized_heads()?;
    env.file("random-file", "independent staged contents\n");
    but_testsupport::git_at_dir(env.projects_root())
        .args(["add", "random-file"])
        .run();
    let index = std::fs::read(repo.index_path())?;
    repo.reference(
        changed.as_ref(),
        branch,
        PreviousValue::MustExist,
        "independent change",
    )?;
    let error = pending.rollback_error(&repo, anyhow::anyhow!("original transaction failure"));
    let message = format!("{error:#}");
    assert!(
        message.contains("original transaction failure"),
        "recovery preserves the original failure: {message}"
    );
    assert!(
        message.contains("Index changed independently"),
        "recovery reports staging it cannot safely restore: {message}"
    );
    assert!(
        message.contains("Could not restore refs/heads/changed-independently"),
        "recovery reports the independent ref edit: {message}"
    );
    assert_eq!(
        std::fs::read(repo.index_path())?,
        index,
        "independent staging is never overwritten"
    );
    assert_eq!(
        env.read_file("random-file")?,
        "independent staged contents\n",
        "independent contents survive"
    );
    assert_eq!(
        repo.rev_parse_single(changed.as_ref())?.detach(),
        branch,
        "the ref CAS preserves independent changes"
    );
    assert!(
        repo.try_find_reference(restorable.as_ref())?.is_none(),
        "other refs are restored despite recovery errors"
    );
    assert_eq!(
        repo.head_id()?.detach(),
        head,
        "HEAD is restored despite a worktree recovery error"
    );
    Ok(())
}

#[test]
fn recovery_preserves_ref_changes_between_two_recorded_writes() -> anyhow::Result<()> {
    use gix::refs::transaction::{PreviousValue, RefEdit};
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    let repo = but_testsupport::open_repo(env.projects_root())?;
    let original = repo.head_id()?.detach();
    let first = repo.rev_parse_single("branch")?.detach();
    let independent = repo.rev_parse_single("branch~1")?.detach();
    let last = repo.rev_parse_single("main")?.detach();
    for intermediate in [Some(independent), None] {
        let name: FullName = if intermediate.is_some() {
            "refs/heads/intervening-update"
        } else {
            "refs/heads/intervening-delete"
        }
        .try_into()?;
        repo.reference(
            name.as_ref(),
            original,
            PreviousValue::MustNotExist,
            "initial",
        )?;
        let mut pending = crate::PendingRefChanges::default();
        pending
            .committed
            .extend(repo.edit_reference(RefEdit::update(
                name.clone(),
                first,
                PreviousValue::Any,
                "first write",
            ))?);
        match intermediate {
            Some(target) => {
                repo.reference(
                    name.as_ref(),
                    target,
                    PreviousValue::Any,
                    "independent write",
                )?;
            }
            None => {
                repo.edit_reference(RefEdit::delete(name.clone(), PreviousValue::MustExist))?;
            }
        }
        pending
            .committed
            .extend(repo.edit_reference(RefEdit::update(
                name.clone(),
                last,
                PreviousValue::Any,
                "last write",
            ))?);
        pending.rollback(&repo)?;
        assert_eq!(
            repo.try_find_reference(name.as_ref())?
                .map(|reference| reference.target().id().to_owned()),
            intermediate,
            "recovery restores the independently written state before its newest continuous sequence of edits"
        );
    }
    Ok(())
}

#[test]
fn failed_metadata_write_removes_the_ref_created_before_the_error() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["branch"]);
    let mut ctx = Context::from_repo_for_testing(but_testsupport::open_repo(env.projects_root())?)?
        .with_memory_app_cache();
    let sql = rusqlite::Connection::open(ctx.project_data_dir.join("but.sqlite"))?;
    sql.execute_batch("CREATE TRIGGER fail_creation BEFORE INSERT ON branch_metadata WHEN NEW.ref_name = CAST('refs/heads/transaction-only' AS BLOB)
        BEGIN SELECT RAISE(ABORT, 'metadata creation failed'); END;")?;
    let reference: FullName = "refs/heads/transaction-only".try_into()?;
    let error = with_transaction(
        &mut ctx,
        SnapshotDetails::new(OperationKind::CreateBranch),
        DryRun::No,
        |mut tx| {
            tx.create_reference(
                reference.as_ref(),
                None,
                |_| but_core::ref_metadata::StackId::generate(),
                None,
            )?;
            Ok(())
        },
    )
    .expect_err("metadata creation fails after creating the Git ref");
    assert!(
        format!("{error:#}").contains("metadata creation failed"),
        "the SQL error is preserved: {error:#}"
    );
    assert!(
        env.open_repo()
            .try_find_reference(reference.as_ref())?
            .is_none(),
        "the receipt survives the helper's metadata error"
    );
    assert_num_snapshots(&ctx, 0);
    Ok(())
}

use anyhow::{Context as _, Result, anyhow};
use bstr::{BString, ByteSlice as _};
use but_core::{
    RefMetadata, RepositoryExt,
    ref_metadata::{StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch},
};
use but_ctx::Context;
use but_meta::{VirtualBranchesTomlMetadata, virtual_branches_legacy_types::Target};
use but_testsupport::{gix_testtools, open_repo, visualize_commit_graph};
use git2::build::CheckoutBuilder;
use gitbutler_edit_mode::commands::{
    abort_and_return_to_workspace, enter_edit_mode, save_and_return_to_workspace,
};
use gitbutler_operating_modes::INTEGRATION_BRANCH_REF;
use tempfile::TempDir;

fn command_ctx(folder: &str) -> Result<(Context, TempDir)> {
    let folder = folder.to_owned();
    let folder_for_post = folder.clone();
    let (tmp, _) = gix_testtools::scripted_fixture_writable_with_args_with_post(
        "edit_mode.sh",
        None::<String>,
        gix_testtools::Creation::Execute,
        2,
        move |fixture| {
            if fixture.is_uninitialized() {
                let repo = open_repo(&fixture.path().join(&folder_for_post))?;
                seed_metadata(&repo)?;
            }
            Ok(())
        },
    )
    .map_err(anyhow::Error::from_boxed)?;
    let repo = open_repo(tmp.path().join(folder).as_path())?;
    Ok((Context::from_repo(repo)?, tmp))
}

fn seed_metadata(repo: &gix::Repository) -> Result<()> {
    let mut meta = VirtualBranchesTomlMetadata::from_path(
        repo.gitbutler_storage_path()?.join("virtual_branches.toml"),
    )?;
    let mut ws = meta.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    ws.stacks.clear();
    ws.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/branchy".try_into()?,
            head_commit_id: None,
            archived: false,
        }],
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    let target = Target {
        branch: "refs/remotes/origin/main".parse()?,
        remote_url: ".".to_owned(),
        sha: repo.rev_parse_single("refs/remotes/origin/main")?.detach(),
        push_remote_name: Some("origin".to_owned()),
    };
    meta.set_workspace(&ws)?;
    meta.data_mut().default_target = Some(target);
    meta.set_changed_to_necessitate_write();
    meta.write_unreconciled()?;
    Ok(())
}

fn stack_id(ctx: &Context) -> Result<StackId> {
    let guard = ctx.shared_worktree_access();
    let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    ws.stacks
        .first()
        .context("expected workspace stack")?
        .id
        .context("expected workspace stack id")
}

#[test]
fn basic_leaving_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();

    let worktree_dir = repo.workdir().unwrap().to_owned();
    drop(repo);
    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar, stack_id)?;

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;
    std::fs::write(worktree_dir.join("newfile"), "created during edit mode\n")?;

    save_and_return_to_workspace(&mut ctx)?;

    let repo = ctx.repo.get()?;
    let blob = repo.rev_parse_single(b"HEAD^{/foobar}:file")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @"edited during edit mode");
    let blob = repo.rev_parse_single(b"HEAD^{/foobar}:newfile")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @"created during edit mode");

    Ok(())
}

#[test]
fn multiple_commits_created_during_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();

    let worktree_dir = repo.workdir().unwrap().to_owned();
    drop(repo);
    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar, stack_id)?;

    let repo = ctx.repo.get()?;
    let commit = gix::objs::Commit::try_from(repo.find_commit(foobar)?.decode()?)?;
    let first_id = repo.write_object(gix::objs::Commit {
        message: BString::from(b"first commit added"),
        parents: [foobar].into(),
        ..commit.clone()
    })?;
    let second_id = repo.write_object(gix::objs::Commit {
        message: BString::from(b"second commit added"),
        parents: [first_id.detach()].into(),
        ..commit
    })?;
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: b"arbitrary message".into(),
            },
            expected: gix::refs::transaction::PreviousValue::Any,
            new: gix::refs::Target::Object(second_id.detach()),
        },
        name: "HEAD".try_into().unwrap(),
        deref: true,
    })?;
    drop(repo);

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;

    save_and_return_to_workspace(&mut ctx)?;

    let repo = &*ctx.repo.get()?;
    insta::assert_snapshot!(visualize_commit_graph(repo, "refs/heads/gitbutler/workspace")?, @r"
    * 59ab552 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * f6d3539 (branchy) second commit added
    * d39dd61 first commit added
    * 26804c3 foobar
    * 7950f06 (origin/main, origin/HEAD, main, gitbutler/target) init
    ");
    // As usual, any uncommitted changes (to "file" in this test) is applied
    // onto the HEAD commit at the time of exiting edit mode.
    let blob = repo.rev_parse_single(b"HEAD^{/second}:file")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @"edited during edit mode");
    // Rebase also happens correctly.
    let blob = repo.rev_parse_single(b"HEAD:file")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @"edited during edit mode");

    Ok(())
}

#[test]
fn apply_commit_on_itself() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();

    drop(repo);
    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar, stack_id)?;

    let repo = ctx.repo.get()?;
    // Set HEAD to gitbutler/workspace, detached, to see what happens when we
    // try to apply itself on itself.
    let workspace_commit_id = repo
        .find_reference("refs/heads/gitbutler/workspace")?
        .peel_to_commit()?
        .id;
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: b"arbitrary message".into(),
            },
            expected: gix::refs::transaction::PreviousValue::Any,
            new: gix::refs::Target::Object(workspace_commit_id),
        },
        name: "HEAD".try_into().unwrap(),
        deref: true,
    })?;
    drop(repo);

    save_and_return_to_workspace(&mut ctx)?;

    let repo = &*ctx.repo.get()?;
    // It works.
    insta::assert_snapshot!(visualize_commit_graph(repo, "refs/heads/gitbutler/workspace")?, @r"
    * 85cd48c (HEAD -> gitbutler/workspace, gitbutler/edit) GitButler Workspace Commit
    * 6eb9642 (branchy) GitButler Workspace Commit
    * 26804c3 foobar
    * 7950f06 (origin/main, origin/HEAD, main, gitbutler/target) init
    ");

    Ok(())
}

// Fixture:
// * xxx (HEAD -> gitbutler/workspace) GitButler Workspace Commit
// * xxx foobar
// | * 1e2a3a8 (right) right
// |/
// | * f3d2634 (left) left
// |/
// * 7950f06 (origin/main, origin/HEAD, main) init
// Where "left" and "right" contain changes which conflict with each other
#[test]
fn conficted_entries_get_written_when_leaving_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();

    drop(repo);
    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar, stack_id)?;

    let repo = ctx.git2_repo.get()?;
    let init = repo.find_reference("refs/heads/main")?.peel_to_commit()?;
    let left = repo.find_reference("refs/heads/left")?.peel_to_commit()?;
    let right = repo.find_reference("refs/heads/right")?.peel_to_commit()?;

    let mut merge = repo.merge_trees(
        &init.tree()?,
        &left.tree()?,
        &right.tree()?,
        Default::default(),
    )?;

    repo.checkout_index(
        Some(&mut merge),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;

    drop((init, left, right));
    drop(repo);
    save_and_return_to_workspace(&mut ctx)?;

    let repo = ctx.repo.get()?;
    insta::assert_snapshot!(
        std::fs::read_to_string(repo.workdir().unwrap().join("conflict"))?,
        @"
    <<<<<<< ours
    left
    |||||||
    =======
    right
    >>>>>>> theirs
    "
    );

    Ok(())
}

#[test]
fn abort_requires_force_when_changes_were_made() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;
    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();
    drop(repo);

    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar, stack_id)?;

    let repo = ctx.repo.get()?;
    insta::assert_debug_snapshot!(
        repo.head_name()?,
        @r#"
    Some(
        FullName(
            "refs/heads/gitbutler/edit",
        ),
    )
    "#
    );
    let worktree_dir = repo.workdir().unwrap().to_owned();
    drop(repo);

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;

    let result = abort_and_return_to_workspace(&mut ctx, false);
    insta::assert_debug_snapshot!(result.as_ref().map(|_| ()).is_err(), @"true");
    let err = result
        .err()
        .ok_or_else(|| anyhow!("expected forced abort to fail without --force"))?;
    insta::assert_snapshot!(
        err,
        @"
    The working tree differs from the original commit. A forced abort is necessary.
    If you are seeing this message, please report it as a bug. The UI should have prevented this line getting hit.
    "
    );
    insta::assert_debug_snapshot!(
        ctx.repo.get()?.head_name()?,
        @r#"
    Some(
        FullName(
            "refs/heads/gitbutler/edit",
        ),
    )
    "#
    );

    abort_and_return_to_workspace(&mut ctx, true)?;
    insta::assert_debug_snapshot!(
        ctx.repo.get()?.head_name()?,
        @r#"
    Some(
        FullName(
            "refs/heads/gitbutler/workspace",
        ),
    )
    "#
    );

    Ok(())
}

#[test]
fn enter_edit_mode_checks_out_conflicted_commit() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("enter_edit_mode_with_conflicted_commit")?;
    let repo = ctx.repo.get()?;
    let conflicted_commit = repo
        .find_reference("refs/tags/conflicted-target")?
        .peel_to_commit()?
        .id()
        .detach();

    drop(repo);

    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, conflicted_commit, stack_id)?;

    let repo = ctx.repo.get()?;
    insta::assert_debug_snapshot!(
        repo.head_name()?,
        @r#"
    Some(
        FullName(
            "refs/heads/gitbutler/edit",
        ),
    )
    "#
    );
    insta::assert_debug_snapshot!(
        repo.head_commit()?.message()?.summary(),
        @r#""Changes to make millions""#
    );

    insta::assert_snapshot!(
        std::fs::read_to_string(repo.path().parent().unwrap().join("conflict"))?,
        @r"
    <<<<<<< New base: Changes to make millions
    left
    ||||||| Common ancestor
    base
    =======
    right
    >>>>>>> Current commit: Changes to make millions
    "
    );
    drop(repo);

    abort_and_return_to_workspace(&mut ctx, true)?;
    insta::assert_debug_snapshot!(
        ctx.repo.get()?.head_name()?,
        @r#"
    Some(
        FullName(
            "refs/heads/gitbutler/workspace",
        ),
    )
    "#
    );

    Ok(())
}

#[test]
fn enter_edit_mode_works_with_only_integration_ref_present() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let foobar = {
        let repo = ctx.repo.get()?;
        let workspace_head = repo.head_commit()?;
        let foobar = workspace_head.decode()?.parents().next().unwrap();
        repo.reference(
            INTEGRATION_BRANCH_REF,
            workspace_head.id(),
            gix::refs::transaction::PreviousValue::Any,
            "",
        )?;
        repo.edit_reference(gix::refs::transaction::RefEdit {
            change: gix::refs::transaction::Change::Update {
                log: gix::refs::transaction::LogChange {
                    mode: gix::refs::transaction::RefLog::AndReference,
                    force_create_reflog: false,
                    message: b"arbitrary message".into(),
                },
                expected: gix::refs::transaction::PreviousValue::Any,
                new: gix::refs::Target::Symbolic(INTEGRATION_BRANCH_REF.try_into()?),
            },
            name: "HEAD".try_into().unwrap(),
            deref: false,
        })?;
        repo.find_reference("refs/heads/gitbutler/workspace")?
            .delete()
            .context("expected workspace ref to exist")?;
        foobar
    };

    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar, stack_id)?;

    insta::assert_debug_snapshot!(
        ctx.repo.get()?.head_name()?,
        @r#"
    Some(
        FullName(
            "refs/heads/gitbutler/edit",
        ),
    )
    "#
    );

    Ok(())
}

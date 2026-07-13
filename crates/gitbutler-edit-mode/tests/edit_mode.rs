use anyhow::{Context as _, Result, anyhow};
use bstr::BString;
use but_core::{
    RefMetadata, RepositoryExt,
    ref_metadata::{
        ProjectMeta, StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_ctx::Context;
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::{gix_testtools, open_repo, visualize_commit_graph};
use gitbutler_edit_mode::commands::{
    abort_and_return_to_workspace, enter_edit_mode, save_and_return_to_workspace,
};
use gitbutler_operating_modes::{
    EditModeMetadata, INTEGRATION_BRANCH_REF, read_edit_mode_metadata, write_edit_mode_metadata,
};
use snapbox::prelude::*;
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
    let repo = open_repo(tmp.path().join(&folder).as_path())?;
    let ctx = Context::from_repo_for_testing(repo)?;
    Ok((ctx, tmp))
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
            archived: false,
        }],
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    meta.set_workspace(&ws)?;
    meta.set_changed_to_necessitate_write();
    meta.write_unreconciled()?;
    ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
        push_remote: Some("origin".to_owned()),
    }
    .persist(repo)?;
    Ok(())
}

fn stack_id(ws: &but_graph::Workspace) -> Result<StackId> {
    ws.stacks
        .first()
        .context("expected workspace stack")?
        .id
        .context("expected workspace stack id")
}

/// Seed the metadata that [`enter_edit_mode()`] normally writes.
///
/// Some fixtures start directly on `refs/heads/gitbutler/edit` with a hand-crafted
/// conflicted index, so running the normal entry command would replace the state the
/// test is trying to exercise. `save_and_return_to_workspace()` still needs this
/// metadata to know which stack and commit the edit branch belongs to.
///
/// `edit_commit_id` is the Git revision to record as the commit being edited. It is resolved
/// the same way as command input would be, allowing fixtures to name the original
/// edit target without hard-coding an object id.
fn seed_edit_mode_metadata(ctx: &Context, edit_commit_id: &str) -> Result<()> {
    let repo = ctx.repo.get()?;
    let edit_mode_metadata = EditModeMetadata {
        commit_oid: repo.rev_parse_single(edit_commit_id)?.detach(),
        stack_id: StackId::from_number_for_testing(1),
    };
    drop(repo);
    write_edit_mode_metadata(ctx, &edit_mode_metadata)?;
    Ok(())
}

/// Assert that every artifact [`enter_edit_mode`] creates has been removed, so leaving edit
/// mode never leaves a dangling ref or stale metadata behind.
fn assert_edit_mode_cleaned_up(ctx: &Context) -> Result<()> {
    let repo = ctx.repo.get()?;
    for ref_name in [
        "refs/heads/gitbutler/edit",
        "refs/gitbutler/edit-uncommitted-changes",
    ] {
        assert!(
            repo.try_find_reference(ref_name)?.is_none(),
            "{ref_name} should be removed when leaving edit mode"
        );
    }
    drop(repo);
    assert!(
        read_edit_mode_metadata(ctx).is_err(),
        "edit mode metadata should be removed when leaving edit mode"
    );
    Ok(())
}

#[test]
fn basic_leaving_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();

    let worktree_dir = repo.workdir().unwrap().to_owned();
    drop(repo);
    let mut guard = ctx.exclusive_worktree_access();
    let stack_id = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        stack_id(&ws)?
    };
    enter_edit_mode(&mut ctx, foobar, stack_id, guard.write_permission())?;

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;
    std::fs::write(worktree_dir.join("newfile"), "created during edit mode\n")?;

    save_and_return_to_workspace(&mut ctx, guard.write_permission())?;

    let repo = ctx.repo.get()?;
    let blob = repo.rev_parse_single(b"HEAD^{/foobar}:file")?.object()?;
    snapbox::assert_data_eq!(
        &*blob.data,
        snapbox::str![[r#"
edited during edit mode

"#]]
    );
    let blob = repo.rev_parse_single(b"HEAD^{/foobar}:newfile")?.object()?;
    snapbox::assert_data_eq!(
        &*blob.data,
        snapbox::str![[r#"
created during edit mode

"#]]
    );

    Ok(())
}

#[test]
fn evolution_parents_written() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.rev_parse_single(b"HEAD^{/foobar}")?.detach();

    let worktree_dir = repo.workdir().unwrap().to_owned();
    drop(repo);
    let mut guard = ctx.exclusive_worktree_access();
    let stack_id = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        stack_id(&ws)?
    };
    enter_edit_mode(&mut ctx, foobar, stack_id, guard.write_permission())?;

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;

    save_and_return_to_workspace(&mut ctx, guard.write_permission())?;

    let repo = ctx.repo.get()?;
    let session = git_meta_lib::Session::open(repo.path())?;
    let evolution_parent = session
        .target(&git_meta_lib::Target::commit(
            &repo
                .rev_parse_single(b"HEAD^{/foobar}")?
                .to_hex()
                .to_string(),
        )?)
        .get_value("evolution-parent")?;
    snapbox::assert_data_eq!(
        evolution_parent.to_debug(),
        snapbox::str![[r#"
Some(
    Set(
        {
            "26804c33bfc7bf602e778b8dd847283bbf886b6a",
        },
    ),
)

"#]]
    );

    Ok(())
}

#[test]
fn multiple_commits_created_during_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();

    let worktree_dir = repo.workdir().unwrap().to_owned();
    drop(repo);
    let mut guard = ctx.exclusive_worktree_access();
    let stack_id = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        stack_id(&ws)?
    };
    enter_edit_mode(&mut ctx, foobar, stack_id, guard.write_permission())?;

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

    save_and_return_to_workspace(&mut ctx, guard.write_permission())?;

    let repo = &*ctx.repo.get()?;
    snapbox::assert_data_eq!(
        visualize_commit_graph(repo, "refs/heads/gitbutler/workspace")?,
        snapbox::str![[r#"
* 33e3bfc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* f6d3539 (branchy) second commit added
* d39dd61 first commit added
* 26804c3 foobar
* 7950f06 (origin/main, origin/HEAD, main, gitbutler/target) init

"#]]
    );
    // As usual, any uncommitted changes (to "file" in this test) is applied
    // onto the HEAD commit at the time of exiting edit mode.
    let blob = repo.rev_parse_single(b"HEAD^{/second}:file")?.object()?;
    snapbox::assert_data_eq!(
        &*blob.data,
        snapbox::str![[r#"
edited during edit mode

"#]]
    );
    // Rebase also happens correctly.
    let blob = repo.rev_parse_single(b"HEAD:file")?.object()?;
    snapbox::assert_data_eq!(
        &*blob.data,
        snapbox::str![[r#"
edited during edit mode

"#]]
    );

    Ok(())
}

#[test]
fn apply_commit_on_itself() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;

    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();

    drop(repo);
    let mut guard = ctx.exclusive_worktree_access();
    let stack_id = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        stack_id(&ws)?
    };
    enter_edit_mode(&mut ctx, foobar, stack_id, guard.write_permission())?;

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

    save_and_return_to_workspace(&mut ctx, guard.write_permission())?;

    let repo = &*ctx.repo.get()?;
    // It works, and the gitbutler/edit ref is cleaned up on the way out.
    snapbox::assert_data_eq!(
        visualize_commit_graph(repo, "refs/heads/gitbutler/workspace")?,
        snapbox::str![[r#"
* 16b549b (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 6eb9642 (branchy) GitButler Workspace Commit
* 26804c3 foobar
* 7950f06 (origin/main, origin/HEAD, main, gitbutler/target) init

"#]]
    );
    assert_edit_mode_cleaned_up(&ctx)?;

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
    let (mut ctx, _tempdir) =
        command_ctx("conficted_entries_get_written_when_leaving_edit_mode_in_edit_mode")?;
    seed_edit_mode_metadata(&ctx, "refs/heads/branchy")?;
    let mut guard = ctx.exclusive_worktree_access();
    save_and_return_to_workspace(&mut ctx, guard.write_permission())?;

    let repo = ctx.repo.get()?;
    snapbox::assert_data_eq!(
        std::fs::read_to_string(repo.workdir().unwrap().join("conflict"))?,
        snapbox::str![[r#"
<<<<<<< ours
left
|||||||
=======
right
>>>>>>> theirs

"#]]
    );

    Ok(())
}

#[test]
fn abort_requires_force_when_changes_were_made() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.repo.get()?;
    let foobar = repo.head_commit()?.decode()?.parents().next().unwrap();
    drop(repo);

    let mut guard = ctx.exclusive_worktree_access();
    let stack_id = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        stack_id(&ws)?
    };
    enter_edit_mode(&mut ctx, foobar, stack_id, guard.write_permission())?;

    let repo = ctx.repo.get()?;
    snapbox::assert_data_eq!(
        repo.head_name()?.to_debug(),
        snapbox::str![[r#"
Some(
    FullName(
        "refs/heads/gitbutler/edit",
    ),
)

"#]]
    );
    let worktree_dir = repo.workdir().unwrap().to_owned();
    drop(repo);

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;

    let result = abort_and_return_to_workspace(&mut ctx, false, guard.write_permission());
    snapbox::assert_data_eq!(
        result.as_ref().map(|_| ()).is_err().to_debug(),
        snapbox::str![[r#"
true

"#]]
    );
    let err = result
        .err()
        .ok_or_else(|| anyhow!("expected forced abort to fail without --force"))?;
    snapbox::assert_data_eq!(
        err.to_string(),
        snapbox::str!["The working tree differs from the original commit. A forced abort is necessary.
If you are seeing this message, please report it as a bug. The UI should have prevented this line getting hit."]
    );
    snapbox::assert_data_eq!(
        ctx.repo.get()?.head_name()?.to_debug(),
        snapbox::str![[r#"
Some(
    FullName(
        "refs/heads/gitbutler/edit",
    ),
)

"#]]
    );

    abort_and_return_to_workspace(&mut ctx, true, guard.write_permission())?;
    snapbox::assert_data_eq!(
        ctx.repo.get()?.head_name()?.to_debug(),
        snapbox::str![[r#"
Some(
    FullName(
        "refs/heads/gitbutler/workspace",
    ),
)

"#]]
    );
    // Leaving edit mode cleans up every edit-mode artifact instead of leaving them dangling.
    assert_edit_mode_cleaned_up(&ctx)?;

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

    let mut guard = ctx.exclusive_worktree_access();
    let stack_id = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        stack_id(&ws)?
    };
    enter_edit_mode(
        &mut ctx,
        conflicted_commit,
        stack_id,
        guard.write_permission(),
    )?;

    let repo = ctx.repo.get()?;
    snapbox::assert_data_eq!(
        repo.head_name()?.to_debug(),
        snapbox::str![[r#"
Some(
    FullName(
        "refs/heads/gitbutler/edit",
    ),
)

"#]]
    );
    snapbox::assert_data_eq!(
        repo.head_commit()?.message()?.summary().to_debug(),
        snapbox::str![[r#"
"Changes to make millions"

"#]]
    );
    snapbox::assert_data_eq!(
        repo.head_commit()?.decode()?.extra_headers.to_debug(),
        snapbox::str![[r#"
[
    (
        "gitbutler-headers-version",
        "2",
    ),
    (
        "change-id",
        "00000000-0000-0000-0000-000000000001",
    ),
]

"#]]
    );

    snapbox::assert_data_eq!(
        std::fs::read_to_string(repo.path().parent().unwrap().join("conflict"))?,
        snapbox::str![[r#"
<<<<<<< New base: foobar
left
||||||| Common ancestor
base
=======
right
>>>>>>> Current commit: Changes to make millions

"#]]
    );
    drop(repo);

    abort_and_return_to_workspace(&mut ctx, true, guard.write_permission())?;
    snapbox::assert_data_eq!(
        ctx.repo.get()?.head_name()?.to_debug(),
        snapbox::str![[r#"
Some(
    FullName(
        "refs/heads/gitbutler/workspace",
    ),
)

"#]]
    );
    // Leaving edit mode cleans up every edit-mode artifact instead of leaving them dangling.
    assert_edit_mode_cleaned_up(&ctx)?;

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

    let mut guard = ctx.exclusive_worktree_access();
    let stack_id = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        stack_id(&ws)?
    };
    enter_edit_mode(&mut ctx, foobar, stack_id, guard.write_permission())?;

    snapbox::assert_data_eq!(
        ctx.repo.get()?.head_name()?.to_debug(),
        snapbox::str![[r#"
Some(
    FullName(
        "refs/heads/gitbutler/edit",
    ),
)

"#]]
    );

    Ok(())
}

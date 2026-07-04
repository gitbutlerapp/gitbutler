use anyhow::Result;
use but_api::resolve::{
    HunkResolution, ResolutionSpec, commit_conflicts, resolve_commit_conflict_hunks,
};
use but_core::DryRun;
use gitbutler_oplog::OplogExt as _;
use gix::prelude::ObjectIdExt as _;

fn conflicted_context() -> Result<(but_ctx::Context, gix::ObjectId, tempfile::TempDir)> {
    let (repo, tmp) = crate::support::writable_scenario("resolve-ai-conflicted-commit");
    crate::support::persist_default_target(&repo)?;
    let conflicted_commit = repo.rev_parse_single("refs/tags/conflicted")?.detach();
    let ctx = but_ctx::Context::from_repo(repo)?.with_memory_app_cache();
    Ok((ctx, conflicted_commit, tmp))
}

fn spec(path: &str, hunk: usize, resolution: HunkResolution) -> ResolutionSpec {
    ResolutionSpec {
        path: path.into(),
        hunk,
        resolution,
    }
}

#[test]
fn conflicts_are_listed_without_entering_edit_mode() -> Result<()> {
    let (ctx, conflicted_commit, _tmp) = conflicted_context()?;

    let conflicts = commit_conflicts(&ctx, conflicted_commit)?;
    assert_eq!(conflicts.commit_id, conflicted_commit);
    assert_eq!(conflicts.files.len(), 1);
    let file = &conflicts.files[0];
    assert_eq!(file.path, "conflict");
    assert_eq!(file.hunks.len(), 2);
    assert_eq!(file.hunks[0].ours, "line two changed by the new base");
    assert_eq!(file.hunks[0].theirs, "line two changed by this commit");
    assert_eq!(file.hunks[0].base.as_deref(), Some("line two"));
    assert_eq!(file.hunks[0].context_before, "line one");
    assert_eq!(file.hunks[1].ours, "line six changed by the new base");
    assert_eq!(file.hunks[1].context_after, "line seven\n");

    // The synthetic change diffs the base's version against the commit's own.
    let repo = ctx.repo.get()?;
    assert_eq!(file.change.path, "conflict");
    let but_core::ui::TreeStatus::Modification {
        previous_state,
        state,
        ..
    } = &file.change.status
    else {
        panic!("expected a modification, got {:?}", file.change.status);
    };
    let content = |id: gix::ObjectId| -> Result<String> {
        Ok(String::from_utf8(repo.find_blob(id)?.data.clone())?)
    };
    assert!(content(previous_state.id)?.contains("changed by the new base"));
    assert!(content(state.id)?.contains("changed by this commit"));

    // Read-only: the commit and the workspace are untouched.
    let commit = but_core::Commit::from_id(conflicted_commit.attach(&repo))?;
    assert!(commit.is_conflicted());
    Ok(())
}

#[test]
fn partial_resolution_narrows_the_conflict_then_completes() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = conflicted_context()?;

    // Resolve only the first of the two conflicts, with mixed content.
    let first = resolve_commit_conflict_hunks(
        &mut ctx,
        conflicted_commit,
        vec![spec(
            "conflict",
            1,
            HunkResolution::Content("line two merged".into()),
        )],
        DryRun::No,
    )?;
    assert_eq!(first.resolved, 1);
    assert_eq!(first.remaining.len(), 1);
    assert_eq!(first.remaining[0].path, "conflict");
    assert_eq!(first.remaining[0].hunks, 1);

    {
        let repo = ctx.repo.get()?;
        let narrowed = but_core::Commit::from_id(first.new_commit.attach(&repo))?;
        assert!(
            narrowed.is_conflicted(),
            "a partial resolution must leave the commit conflicted"
        );
        assert_eq!(
            narrowed
                .message
                .to_string()
                .matches("GitButler-Conflict")
                .count(),
            1,
            "re-marking the message must be idempotent"
        );
        // The auto-resolution carries the resolution and favors ours elsewhere.
        let auto_blob = repo
            .rev_parse_single(format!("{}:.auto-resolution/conflict", first.new_commit).as_str())?
            .object()?;
        assert_eq!(
            auto_blob.data.as_slice(),
            b"line one\nline two merged\nline three\nline four\nline five\nline six changed by the new base\nline seven\n"
        );
        // The descendant sits on the narrowed commit.
        let descendant = repo
            .rev_parse_single("refs/heads/branchy")?
            .object()?
            .into_commit();
        assert_eq!(
            descendant.decode()?.parents().next(),
            Some(first.new_commit)
        );
    }

    // The narrowed commit reports exactly the one remaining conflict.
    let conflicts = commit_conflicts(&ctx, first.new_commit)?;
    assert_eq!(conflicts.files.len(), 1);
    let file = &conflicts.files[0];
    assert_eq!(file.hunks.len(), 1, "one conflict must remain");
    assert_eq!(file.hunks[0].ours, "line six changed by the new base");
    assert_eq!(file.hunks[0].theirs, "line six changed by this commit");
    assert_eq!(
        file.hunks[0].base.as_deref(),
        Some("line six"),
        "the remaining conflict keeps its common ancestor"
    );

    // Resolve the remaining conflict by taking theirs — the commit normalizes.
    let second = resolve_commit_conflict_hunks(
        &mut ctx,
        first.new_commit,
        vec![spec("conflict", 1, HunkResolution::Theirs)],
        DryRun::No,
    )?;
    assert_eq!(second.resolved, 1);
    assert!(second.remaining.is_empty());

    {
        let repo = ctx.repo.get()?;
        let resolved = but_core::Commit::from_id(second.new_commit.attach(&repo))?;
        assert!(!resolved.is_conflicted());
        assert_eq!(resolved.message.to_string(), "Change line two");
        let blob = repo
            .rev_parse_single(format!("{}:conflict", second.new_commit).as_str())?
            .object()?;
        assert_eq!(
            blob.data.as_slice(),
            b"line one\nline two merged\nline three\nline four\nline five\nline six changed by this commit\nline seven\n"
        );
        let later = repo
            .rev_parse_single("refs/heads/branchy:later")?
            .object()?;
        assert_eq!(later.data.as_slice(), b"descendant\n");
    }

    // Both applies recorded undo points.
    let resolve_snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .filter_map(Result::ok)
        .filter(|snapshot| {
            snapshot.details.as_ref().is_some_and(|details| {
                matches!(
                    details.operation,
                    but_oplog::legacy::OperationKind::ResolveConflicts
                )
            })
        })
        .count();
    assert_eq!(resolve_snapshots, 2);

    Ok(())
}

#[test]
fn taking_one_side_for_all_hunks_resolves_the_commit() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = conflicted_context()?;

    let result = resolve_commit_conflict_hunks(
        &mut ctx,
        conflicted_commit,
        vec![
            spec("conflict", 1, HunkResolution::Ours),
            spec("conflict", 2, HunkResolution::Ours),
        ],
        DryRun::No,
    )?;
    assert_eq!(result.resolved, 2);
    assert!(result.remaining.is_empty());

    let repo = ctx.repo.get()?;
    let resolved = but_core::Commit::from_id(result.new_commit.attach(&repo))?;
    assert!(!resolved.is_conflicted());
    let blob = repo
        .rev_parse_single(format!("{}:conflict", result.new_commit).as_str())?
        .object()?;
    assert_eq!(
        blob.data.as_slice(),
        b"line one\nline two changed by the new base\nline three\nline four\nline five\nline six changed by the new base\nline seven\n"
    );
    Ok(())
}

#[test]
fn invalid_specs_change_nothing() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = conflicted_context()?;

    let err = resolve_commit_conflict_hunks(
        &mut ctx,
        conflicted_commit,
        vec![spec("conflict", 3, HunkResolution::Ours)],
        DryRun::No,
    )
    .unwrap_err();
    assert!(err.to_string().contains("has conflicts 1..2"), "{err}");

    let repo = ctx.repo.get()?;
    let commit = but_core::Commit::from_id(conflicted_commit.attach(&repo))?;
    assert!(commit.is_conflicted(), "nothing may be written on failure");
    Ok(())
}

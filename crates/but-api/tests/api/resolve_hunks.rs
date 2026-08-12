use anyhow::Result;
use but_api::resolve::{
    HunkResolution, ResolutionSpec, commit_conflicts, resolve_commit_conflict_hunks,
};
use gitbutler_oplog::OplogExt as _;
use gix::prelude::ObjectIdExt as _;

fn conflicted_context() -> Result<(but_ctx::Context, gix::ObjectId, tempfile::TempDir)> {
    let (repo, tmp) = crate::support::writable_scenario("resolve-ai-conflicted-commit");
    crate::support::persist_default_target(&repo)?;
    let conflicted_commit = repo.rev_parse_single("refs/tags/conflicted")?.detach();
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
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

    // The synthetic change is parent → the intended result: the removed side
    // is what is applied, the added side is what the commit meant and could
    // not apply.
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
    let previous = content(previous_state.id)?;
    assert!(
        previous.contains("changed by the new base"),
        "the removed side is the parent, i.e. what is actually applied: {previous:?}"
    );
    let intended = content(state.id)?;
    assert!(intended.contains("changed by this commit"));
    assert!(!intended.contains("<<<<<<<"), "never marker text");

    assert_ne!(
        file.hunks[0].id, file.hunks[1].id,
        "distinct conflicts never share an id"
    );

    // The marker text carries display labels; parsing its blocks in order is
    // what a marker-aware renderer does, so it must line up with `hunks`.
    assert_eq!(file.merged_text.matches("<<<<<<< auto resolved").count(), 2);
    assert_eq!(file.merged_text.matches("||||||| original base").count(), 2);
    assert_eq!(file.merged_text.matches(">>>>>>> as authored").count(), 2);
    assert!(
        !file.merged_text.contains("gitbutler-resolve"),
        "sentinels are for the scanner, not for humans: {}",
        file.merged_text
    );

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
        // The stored base stays untouched: the resolution lives in ours and
        // theirs, so a later re-pick onto changed parents replays it instead
        // of dropping it as a region the commit does not change.
        let base_blob = repo
            .rev_parse_single(format!("{}:.conflict-base-0/conflict", first.new_commit).as_str())?
            .object()?;
        assert_eq!(
            base_blob.data.as_slice(),
            b"line one\nline two\nline three\nline four\nline five\nline six\nline seven\n"
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
    let survivor_id = commit_conflicts(&ctx, conflicted_commit)?.files[0].hunks[1]
        .id
        .clone();
    let conflicts = commit_conflicts(&ctx, first.new_commit)?;
    assert_eq!(conflicts.files.len(), 1);
    let file = &conflicts.files[0];
    assert_eq!(file.hunks.len(), 1, "one conflict must remain");
    assert_eq!(
        file.hunks[0].id, survivor_id,
        "a surviving conflict keeps its id while its position compacts"
    );
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
    )?;
    assert_eq!(second.resolved, 1);
    assert!(second.remaining.is_empty());
    assert!(!second.commit_emptied, "the commit keeps its own changes");

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
    )?;
    assert_eq!(result.resolved, 2);
    assert!(result.remaining.is_empty());
    assert!(
        result.commit_emptied,
        "keeping the base everywhere leaves the commit without changes of its own"
    );

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
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("has 2 conflicts, but conflict 3 was addressed"),
        "{err}"
    );

    let repo = ctx.repo.get()?;
    let commit = but_core::Commit::from_id(conflicted_commit.attach(&repo))?;
    assert!(commit.is_conflicted(), "nothing may be written on failure");
    Ok(())
}

#[test]
fn ai_specs_are_narrowed_to_their_hunks_and_applied() -> Result<()> {
    use but_api::resolve::{
        FileResolution, HunkContent, ResolutionResponse, resolve_commit_conflict_hunks_with,
    };

    let (mut ctx, conflicted_commit, _tmp) = conflicted_context()?;

    // One AI-resolved and one side-picked conflict in a single call.
    let result = resolve_commit_conflict_hunks_with(
        &mut ctx,
        conflicted_commit,
        vec![
            spec("conflict", 1, HunkResolution::Ai),
            spec("conflict", 2, HunkResolution::Theirs),
        ],
        |request| {
            // The model must only see the AI-addressed hunk.
            assert_eq!(request.files.len(), 1);
            assert_eq!(request.files[0].hunks.len(), 1);
            assert_eq!(
                request.files[0].hunks[0].ours,
                "line two changed by the new base"
            );
            Ok(ResolutionResponse {
                summary: None,
                resolutions: vec![FileResolution {
                    path: "conflict".into(),
                    hunks: vec![HunkContent {
                        resolved_content: "line two merged by ai".into(),
                    }],
                    reasoning: "combined both sides".into(),
                }],
            })
        },
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
        b"line one\nline two merged by ai\nline three\nline four\nline five\nline six changed by this commit\nline seven\n"
    );

    // A mixed AI + manual apply records an AI undo point.
    let ai_snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .filter_map(Result::ok)
        .filter(|snapshot| {
            snapshot.details.as_ref().is_some_and(|details| {
                matches!(
                    details.operation,
                    but_oplog::legacy::OperationKind::ResolveConflictsAi
                )
            })
        })
        .count();
    assert_eq!(ai_snapshots, 1);
    Ok(())
}

#[test]
fn conflicts_of_a_normal_commit_are_empty_not_an_error() -> Result<()> {
    let (ctx, _conflicted_commit, _tmp) = conflicted_context()?;
    let normal_commit = {
        let repo = ctx.repo.get()?;
        repo.rev_parse_single("refs/heads/main")?.detach()
    };

    let conflicts = commit_conflicts(&ctx, normal_commit)?;
    assert_eq!(conflicts.commit_id, normal_commit);
    assert!(conflicts.files.is_empty());
    Ok(())
}

fn mixed_conflicted_context() -> Result<(but_ctx::Context, gix::ObjectId, tempfile::TempDir)> {
    let (repo, tmp) = crate::support::writable_scenario("resolve-mixed-conflicted-commit");
    crate::support::persist_default_target(&repo)?;
    let conflicted_commit = repo.rev_parse_single("refs/tags/conflicted")?.detach();
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    Ok((ctx, conflicted_commit, tmp))
}

/// A conflict with no hunk representation used to fail the whole request, which
/// hid every other conflict in the commit behind it — and left a caller with no
/// edit mode, like lite, with nothing at all to show.
#[test]
fn a_conflict_without_hunks_is_reported_not_fatal() -> Result<()> {
    let (ctx, conflicted_commit, _tmp) = mixed_conflicted_context()?;

    let conflicts = commit_conflicts(&ctx, conflicted_commit)?;

    // The text conflict is still fully addressable.
    assert_eq!(
        conflicts.files.len(),
        1,
        "the binary must not suppress the file that does decompose into hunks"
    );
    assert_eq!(conflicts.files[0].path, "conflict");
    assert_eq!(conflicts.files[0].hunks.len(), 2);

    // The binary is named, with a reason to show the user.
    assert_eq!(conflicts.manual.len(), 1);
    assert_eq!(conflicts.manual[0].path, "binary");
    assert!(
        conflicts.manual[0].reason.contains("binary"),
        "reason should say why, got {:?}",
        conflicts.manual[0].reason
    );
    Ok(())
}

/// Resolving every addressable hunk still leaves the commit conflicted when a
/// manual-only file remains, so the result must say so rather than reporting a
/// clean commit — and must not trip the "all resolved yet still conflicting"
/// invariant, which only holds when every conflict was addressable.
#[test]
fn resolving_every_hunk_leaves_a_manual_conflict_behind() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = mixed_conflicted_context()?;

    let result = resolve_commit_conflict_hunks(
        &mut ctx,
        conflicted_commit,
        vec![
            spec("conflict", 1, HunkResolution::Theirs),
            spec("conflict", 2, HunkResolution::Theirs),
        ],
    )?;

    assert_eq!(result.resolved, 2);
    assert!(
        result.remaining.is_empty(),
        "every hunk-addressable conflict was resolved"
    );
    assert_eq!(
        result.manual.len(),
        1,
        "the binary is still unresolved, so the commit is not done"
    );

    let repo = ctx.repo.get()?;
    assert!(
        but_core::Commit::from_id(result.new_commit.attach(&repo))?.is_conflicted(),
        "a commit with a manual conflict left stays conflicted"
    );
    Ok(())
}

/// Addressing a file that has no hunks should explain why rather than claim the
/// path is not conflicted at all, which reads as a caller mistake.
#[test]
fn addressing_a_manual_conflict_explains_why() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = mixed_conflicted_context()?;

    let err = resolve_commit_conflict_hunks(
        &mut ctx,
        conflicted_commit,
        vec![spec("binary", 1, HunkResolution::Ours)],
    )
    .unwrap_err();

    let message = format!("{err:#}");
    assert!(
        message.contains("binary") && message.contains("edit mode"),
        "error should name the reason and the way out, got {message:?}"
    );
    Ok(())
}

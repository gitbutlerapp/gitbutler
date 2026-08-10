use anyhow::Result;
use but_api::resolve::{
    FileResolution, HunkContent, ResolutionResponse, resolve_commit_conflicts_with,
};
use but_core::DryRun;
use gitbutler_oplog::OplogExt as _;
use gix::prelude::ObjectIdExt as _;

fn conflicted_context() -> Result<(but_ctx::Context, gix::ObjectId, tempfile::TempDir)> {
    let (repo, tmp) = crate::support::writable_scenario("resolve-ai-conflicted-commit");
    crate::support::persist_default_target(&repo)?;
    let conflicted_commit = repo.rev_parse_single("refs/tags/conflicted")?.detach();
    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    Ok((ctx, conflicted_commit, tmp))
}

fn merged_response(path: &str, contents: &[&str]) -> ResolutionResponse {
    ResolutionResponse {
        summary: Some("### Conflicting changes\nBoth sides changed lines two and six.".into()),
        resolutions: vec![FileResolution {
            path: path.into(),
            hunks: contents
                .iter()
                .map(|content| HunkContent {
                    resolved_content: (*content).into(),
                })
                .collect(),
            reasoning: "Combined both sides of each change.".into(),
        }],
    }
}

#[test]
fn resolves_conflicted_commit_and_rebases_descendants() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = conflicted_context()?;

    let result =
        resolve_commit_conflicts_with(&mut ctx, conflicted_commit, DryRun::No, |request| {
            assert_eq!(request.files.len(), 1, "one conflicted file expected");
            let file = &request.files[0];
            assert_eq!(file.path, "conflict");
            assert_eq!(file.hunks.len(), 2, "two conflict hunks expected");
            let hunk = &file.hunks[0];
            assert_eq!(hunk.ours, "line two changed by the new base");
            assert_eq!(hunk.theirs, "line two changed by this commit");
            assert_eq!(
                hunk.base.as_deref(),
                Some("line two"),
                "diff3 markers should carry the common ancestor"
            );
            assert_eq!(file.hunks[1].ours, "line six changed by the new base");
            assert!(request.commit_message.contains("Change line two"));
            Ok(merged_response(
                "conflict",
                &[
                    "line two changed by both sides",
                    "line six changed by both sides",
                ],
            ))
        })?;

    assert_eq!(result.commit_id, conflicted_commit);
    assert_ne!(result.new_commit, conflicted_commit);
    assert_eq!(result.files.len(), 1);
    assert!(result.summary.is_some());

    {
        let repo = ctx.repo.get()?;

        // The rewritten commit is a normal commit with the resolution spliced in.
        let new_commit = but_core::Commit::from_id(result.new_commit.attach(&repo))?;
        assert!(!new_commit.is_conflicted(), "conflict state must be gone");
        assert_eq!(
            new_commit.message.to_string(),
            "Change line two",
            "conflict markers must be stripped from the message"
        );
        let resolved_blob = repo
            .rev_parse_single(format!("{}:conflict", result.new_commit).as_str())?
            .object()?;
        assert_eq!(
            resolved_blob.data.as_slice(),
            b"line one\nline two changed by both sides\nline three\nline four\nline five\nline six changed by both sides\nline seven\n"
        );
        let untouched_blob = repo
            .rev_parse_single(format!("{}:file", result.new_commit).as_str())?
            .object()?;
        assert_eq!(
            untouched_blob.data.as_slice(),
            b"unrelated\n",
            "non-conflicted files must be preserved byte for byte"
        );

        // The descendant was rebased on top and picked up the resolution.
        let descendant = repo
            .rev_parse_single("refs/heads/branchy")?
            .object()?
            .into_commit();
        assert_eq!(
            descendant.decode()?.parents().next(),
            Some(result.new_commit),
            "the descendant must now sit on the rewritten commit"
        );
        let descendant_conflict = repo
            .rev_parse_single("refs/heads/branchy:conflict")?
            .object()?;
        assert_eq!(
            descendant_conflict.data.as_slice(),
            b"line one\nline two changed by both sides\nline three\nline four\nline five\nline six changed by both sides\nline seven\n",
            "the descendant must inherit the resolution"
        );
        let descendant_later = repo
            .rev_parse_single("refs/heads/branchy:later")?
            .object()?;
        assert_eq!(descendant_later.data.as_slice(), b"descendant\n");
    }

    // An undo point was recorded.
    let snapshots = ctx
        .snapshots_iter(None, Vec::new(), None)?
        .collect::<Result<Vec<_>>>()?;
    assert!(
        snapshots.iter().any(|snapshot| {
            snapshot.details.as_ref().is_some_and(|details| {
                matches!(
                    details.operation,
                    but_oplog::legacy::OperationKind::ResolveConflictsAi
                )
            })
        }),
        "an oplog snapshot with the AI-resolve operation must exist"
    );

    Ok(())
}

#[test]
fn invalid_response_is_retried_once_then_fails_without_changes() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = conflicted_context()?;
    let calls = std::cell::Cell::new(0);

    let err = resolve_commit_conflicts_with(&mut ctx, conflicted_commit, DryRun::No, |_request| {
        calls.set(calls.get() + 1);
        // Wrong path: never matches the requested file.
        Ok(merged_response("wrong-path", &["content", "content"]))
    })
    .unwrap_err();

    assert_eq!(calls.get(), 2, "the model must be retried exactly once");
    assert!(
        err.to_string().contains("wrong-path"),
        "error should name the unrequested path: {err}"
    );

    let repo = ctx.repo.get()?;
    let commit = but_core::Commit::from_id(conflicted_commit.attach(&repo))?;
    assert!(
        commit.is_conflicted(),
        "the conflicted commit must be left untouched on failure"
    );
    Ok(())
}

#[test]
fn resolution_with_leaked_markers_is_rejected() -> Result<()> {
    let (mut ctx, conflicted_commit, _tmp) = conflicted_context()?;

    let err = resolve_commit_conflicts_with(&mut ctx, conflicted_commit, DryRun::No, |_request| {
        Ok(merged_response(
            "conflict",
            &[
                "<<<<<<< ours\nstill conflicted\n=======\noops\n>>>>>>> theirs",
                "fine",
            ],
        ))
    })
    .unwrap_err();

    assert!(
        err.to_string().contains("conflict marker"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn unconflicted_commit_is_rejected() -> Result<()> {
    let (mut ctx, _conflicted_commit, _tmp) = conflicted_context()?;
    let normal_commit = {
        let repo = ctx.repo.get()?;
        repo.rev_parse_single("refs/heads/main")?.detach()
    };

    let err = resolve_commit_conflicts_with(&mut ctx, normal_commit, DryRun::No, |_request| {
        unreachable!("the model must not be called for an unconflicted commit")
    })
    .unwrap_err();

    assert!(
        err.to_string().contains("not conflicted"),
        "unexpected error: {err}"
    );
    Ok(())
}

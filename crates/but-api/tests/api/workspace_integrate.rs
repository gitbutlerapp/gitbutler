use but_core::{DryRun, ref_metadata::ProjectMeta};
use but_testsupport::{CommandExt, git_at_dir, open_repo};

use crate::support::write_file;

#[test]
fn integration_reports_conflicted_files_before_preview_objects_disappear() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path();
    git_at_dir(dir).args(["init", "-b", "main"]).run();
    for (path, content) in [
        ("session.ts", "base\n"),
        ("logo.png", "base\0image"),
        ("deleted.md", "Original documentation\nSecond paragraph\n"),
    ] {
        write_file(dir, path, content)?;
    }
    git_at_dir(dir).args(["add", "."]).run();
    git_at_dir(dir).args(["commit", "-m", "base"]).run();
    git_at_dir(dir)
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    let repo = open_repo(dir)?;
    let base = repo.head_id()?.detach();
    ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(base),
        push_remote: Some("origin".into()),
    }
    .persist(&repo)?;
    git_at_dir(dir).args(["switch", "-c", "feature"]).run();
    for (path, content) in [
        ("session.ts", "feature\n"),
        ("logo.png", "feature\0image"),
        ("deleted.md", "Edited documentation\nSecond paragraph\n"),
        ("clean.txt", "clean\n"),
    ] {
        write_file(dir, path, content)?;
    }
    git_at_dir(dir).args(["add", "."]).run();
    git_at_dir(dir).args(["commit", "-m", "feature"]).run();
    let original = repo.head_id()?.detach();
    git_at_dir(dir).args(["switch", "main"]).run();
    write_file(dir, "session.ts", "upstream\n")?;
    write_file(dir, "logo.png", "upstream\0image")?;
    git_at_dir(dir).args(["rm", "deleted.md"]).run();
    git_at_dir(dir).args(["commit", "-am", "upstream"]).run();
    git_at_dir(dir)
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    git_at_dir(dir).args(["switch", "feature"]).run();
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let updates = vec![but_api::workspace::json::BottomUpdate {
        kind: but_api::workspace::json::BottomUpdateKind::Rebase,
        selector: but_api::commit::json::RelativeTo::Commit(original),
    }];
    let preview = but_api::workspace::workspace_integrate_upstream_only(
        &mut ctx,
        updates.clone(),
        DryRun::Yes,
    )?;
    let preview_id = preview.workspace_state.replaced_commits[&original];
    let preview = serde_json::to_value(
        but_api::workspace::json::WorkspaceIntegrateUpstreamOutcome::try_from(preview)?,
    )?;
    assert_eq!(
        preview["commitConflicts"][preview_id.to_string()],
        serde_json::json!(["deleted.md", "logo.png", "session.ts"]),
        "the preview includes unique paths for text, binary, and delete conflicts, excluding clean files"
    );
    assert_eq!(
        ctx.repo.get()?.head_id()?.detach(),
        original,
        "previewing leaves HEAD unchanged"
    );
    assert!(
        !ctx.repo.get()?.has_object(preview_id),
        "preview commits are not persisted for later file queries"
    );
    assert_eq!(
        ctx.project_meta()?.target_commit_id,
        Some(base),
        "previewing does not advance the target"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("session.ts"))?,
        "feature\n",
        "previewing leaves worktree contents unchanged"
    );
    let actual =
        but_api::workspace::workspace_integrate_upstream_only(&mut ctx, updates, DryRun::No)?;
    let actual_id = actual.workspace_state.replaced_commits[&original];
    let actual = serde_json::to_value(
        but_api::workspace::json::WorkspaceIntegrateUpstreamOutcome::try_from(actual)?,
    )?;
    assert_eq!(
        actual["commitConflicts"][actual_id.to_string()],
        preview["commitConflicts"][preview_id.to_string()],
        "both modes report the same conflicted files under their resulting commit IDs"
    );
    use gix::prelude::ObjectIdExt;
    let repo = ctx.repo.get()?;
    let entries = but_core::Commit::from_id(actual_id.attach(&repo))?
        .conflict_entries()?
        .expect("the applied commit is conflicted");
    // Keep staged paths first, append tree-only conflicts once, and retain each side's order.
    snapbox::assert_data_eq!(
        format!("{entries:?}"),
        snapbox::str![[
            r#"ConflictEntries { ancestor_entries: ["logo.png", "session.ts"], our_entries: ["logo.png", "session.ts", "deleted.md"], their_entries: ["logo.png", "session.ts", "deleted.md"] }"#
        ]],
    );
    let paths: std::collections::BTreeSet<_> = entries
        .ancestor_entries
        .into_iter()
        .chain(entries.our_entries)
        .chain(entries.their_entries)
        .collect();
    assert_eq!(
        serde_json::to_value(paths)?,
        preview["commitConflicts"][preview_id.to_string()],
        "the preview paths match the applied update"
    );
    Ok(())
}

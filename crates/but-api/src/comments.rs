//! API surface for ephemeral comments anchored to lines in diffs, created in the GUI and acted
//! on by agents via the CLI. All logic lives in [`but_comments`]; this module only binds it to
//! [`Context`](but_ctx::Context) (store location, workspace resolution, locking, and wall-clock
//! time).

use but_api_macros::but_api;
use but_comments::{CommentStore, DiffComment, NewComment};
use but_core::sync::RepoShared;
use but_ctx::Context;
use tracing::instrument;

/// The comment store of the project behind `ctx`.
pub fn store(ctx: &Context) -> CommentStore {
    CommentStore::from_project_data_dir(ctx.project_data_dir())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The comments file is invisible to the desktop file monitor, so out-of-process mutations
/// (notably agents running the CLI) must touch the refresh sentinel for the GUI to pick them up.
fn notify_desktop_watcher(ctx: &Context) {
    but_project_handle::write_refresh_sentinel(&ctx.project_data_dir().join("comments.json"));
}

/// Create a new comment anchored to a line in a diff, under caller-held shared worktree access.
///
/// See [`but_comments::create_comment`] for the anchoring semantics.
pub fn comment_create_with_perm(
    ctx: &Context,
    comment: NewComment,
    perm: &RepoShared,
) -> anyhow::Result<DiffComment> {
    let created = {
        let (repo, workspace, _db) = ctx.workspace_and_db_with_perm(perm)?;
        but_comments::create_comment(
            &repo,
            &workspace,
            &store(ctx),
            comment,
            ctx.settings.context_lines,
            now_ms(),
        )?
    };
    notify_desktop_watcher(ctx);
    Ok(created)
}

/// List all unarchived comments, re-anchored against the current diffs, under caller-held
/// shared worktree access.
///
/// See [`but_comments::list_comments`] for the re-anchoring and auto-archiving semantics.
pub fn comments_list_with_perm(
    ctx: &Context,
    perm: &RepoShared,
) -> anyhow::Result<Vec<DiffComment>> {
    let listing = {
        let (repo, workspace, _db) = ctx.workspace_and_db_with_perm(perm)?;
        but_comments::list_comments(
            &repo,
            &workspace,
            &store(ctx),
            ctx.settings.context_lines,
            now_ms(),
        )?
    };
    // Listing can mutate the store (drift, auto-archiving, purging); when it did, other
    // processes' views are stale — notably the GUI after a CLI listing auto-archived something.
    if listing.persisted_changes {
        notify_desktop_watcher(ctx);
    }
    Ok(listing.comments)
}

/// Archive the comment with the given `id`, hiding it from all future listings.
/// Returns `false` if the comment does not exist or was already archived.
#[but_api]
#[instrument(skip(ctx), err(Debug))]
pub fn comment_archive(ctx: &Context, id: String) -> anyhow::Result<bool> {
    let archived = but_comments::archive_comment(&store(ctx), &id, now_ms())?;
    if archived {
        notify_desktop_watcher(ctx);
    }
    Ok(archived)
}

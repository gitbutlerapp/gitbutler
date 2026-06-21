//! Product-level API functions for the How application.

use std::{
    collections::BTreeSet,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use but_api_macros::but_api;
use but_core::{
    DiffSpec, RepositoryExt as _,
    commit::{Headers, SignCommit},
    diff::{worktree_changes, worktree_changes_no_renames},
    snapshot::{create_tree, create_tree::State},
    sync::RepoExclusive,
    worktree::safe_checkout_from_head,
};
use but_ctx::{Context, ProjectHandle};
use but_rebase::{
    commit::DateMode,
    graph_rebase::{Editor, LookupStep as _},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

const CHECKPOINT_PREFIX: &str = "Checkpoint:";
const BOOKMARK_REF_PREFIX: &str = "refs/gitbutler/how/bookmarks";
const BOOKMARKS_FILE_NAME: &str = "how-bookmarks.json";
const ACTIVE_BRANCH_REF: &str = "refs/heads/main";

/// A project opened by How.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HowProject {
    /// Stable project identifier accepted by other SDK APIs.
    pub id: String,
    /// Display title derived from the worktree path.
    pub title: String,
    /// Absolute worktree path.
    pub path: String,
    /// Absolute Git directory path.
    pub git_dir: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowProject);

/// A How checkpoint.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HowCheckpoint {
    /// Commit id backing the checkpoint.
    pub id: String,
    /// Explicit GitButler Change ID stored in the commit headers, if present.
    pub change_id: Option<String>,
    /// Commit subject shown in the timeline.
    pub title: String,
    /// Commit time in milliseconds since Unix epoch.
    pub created_at: i64,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowCheckpoint);

/// Result of creating a How checkpoint.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HowCreateCheckpointResult {
    /// No checkpoint was needed because there were no worktree changes.
    Unchanged,
    /// A checkpoint was created.
    Created {
        /// Commit id backing the checkpoint.
        commit_id: String,
        /// Explicit GitButler Change ID stored in the checkpoint commit.
        change_id: String,
    },
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowCreateCheckpointResult);

/// Why How skipped an async checkpoint message update.
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum HowUpdateCheckpointMessageSkippedReason {
    /// The Change ID no longer belongs to a visible checkpoint on the current line.
    NotFound,
    /// The resolved commit was already published and should not be rewritten.
    Published,
    /// The supplied message was not a How checkpoint message.
    NotCheckpoint,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowUpdateCheckpointMessageSkippedReason);

/// Result of updating a How checkpoint message by Change ID.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HowUpdateCheckpointMessageResult {
    /// The checkpoint message was updated.
    Updated {
        /// The updated checkpoint.
        checkpoint: HowCheckpoint,
    },
    /// The update was safely skipped.
    Skipped {
        /// Why the update was skipped.
        reason: HowUpdateCheckpointMessageSkippedReason,
    },
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowUpdateCheckpointMessageResult);

/// Whether a How bookmark was created by the user or by How as a safety backup.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum HowBookmarkKind {
    /// The user intentionally created or took ownership of the bookmark.
    User,
    /// How created the bookmark to preserve a previous state while switching.
    Auto,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowBookmarkKind);

/// A How bookmark.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HowBookmark {
    /// Stable bookmark identifier, independent from the display name.
    pub id: String,
    /// Display name shown in the product surface.
    pub name: String,
    /// Commit id the bookmark points to.
    pub target_commit_id: String,
    /// Bookmark creation time in milliseconds since Unix epoch.
    pub created_at: i64,
    /// Bookmark update time in milliseconds since Unix epoch.
    pub updated_at: i64,
    /// Whether this bookmark is user-created or auto-preserved.
    pub kind: HowBookmarkKind,
    /// Whether this bookmark points to the active HEAD commit.
    pub is_current: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowBookmark);

/// Project settings used by How.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HowProjectSettings {
    /// Autosave quiet period in milliseconds.
    pub checkpoint_debounce_ms: u32,
    /// Preferred coding agent.
    pub coding_agent: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowProjectSettings);

/// Staged diff payload used for AI checkpoint summaries.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct HowStagedDiff {
    /// Diff stat followed by patch text.
    pub diff: String,
    /// Original byte count before truncation.
    pub original_byte_count: usize,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HowStagedDiff);

/// Open an existing Git repository or a path inside it for How.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn how_open_project(path: String) -> anyhow::Result<HowProject> {
    let repo = discover_repo(Path::new(&path))?;
    project_from_repo(&repo)
}

/// Initialize versioning in `path` if needed, then open it for How.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn how_start_project(path: String) -> anyhow::Result<HowProject> {
    let path = PathBuf::from(path);
    std::fs::create_dir_all(&path)
        .with_context(|| format!("failed to create project directory '{}'", path.display()))?;
    if !path.join(".git").exists() {
        gix::init(&path).with_context(|| {
            format!(
                "failed to initialize Git repository at '{}'",
                path.display()
            )
        })?;
    }
    let repo = discover_repo(&path)?;
    project_from_repo(&repo)
}

/// List How checkpoint commits from the current branch history.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_list_checkpoints(ctx: &Context, limit: usize) -> anyhow::Result<Vec<HowCheckpoint>> {
    let repo = ctx.repo.get()?;
    let Ok(head) = repo.head_id() else {
        return Ok(vec![]);
    };

    let mut checkpoints = Vec::new();
    for info in head.ancestors().first_parent_only().all()? {
        if checkpoints.len() >= limit {
            break;
        }
        let info = info.context("failed to read commit from checkpoint history")?;
        if let Some(checkpoint) = checkpoint_from_commit_id(&repo, info.id)? {
            checkpoints.push(checkpoint);
        }
    }
    Ok(checkpoints)
}

/// Create a How checkpoint commit from the current worktree state.
///
/// Returns `Unchanged` when there are no worktree changes to save.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_create_checkpoint(
    ctx: &mut Context,
    message: String,
) -> anyhow::Result<HowCreateCheckpointResult> {
    validate_checkpoint_message(&message)?;
    let _guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;
    let tree_id = worktree_tree(&repo)?;
    let head_tree_id = repo.head_tree_id_or_empty()?.detach();
    if tree_id == head_tree_id {
        return Ok(HowCreateCheckpointResult::Unchanged);
    }

    let parent = repo.head_id().ok().map(|id| id.detach());
    let commit_id = create_checkpoint_commit(&repo, &message, tree_id, parent.into_iter())
        .context("failed to create checkpoint commit")?;
    write_index_from_tree(&repo, tree_id)?;
    let change_id = explicit_change_id_for_commit(&repo, commit_id)?
        .context("new checkpoint commit did not receive a Change ID")?;
    Ok(HowCreateCheckpointResult::Created {
        commit_id: commit_id.to_string(),
        change_id,
    })
}

/// Update a visible local How checkpoint message by its explicit Change ID.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_update_checkpoint_message_by_change_id(
    ctx: &mut Context,
    change_id: String,
    message: String,
) -> anyhow::Result<HowUpdateCheckpointMessageResult> {
    if !message.starts_with(CHECKPOINT_PREFIX) {
        return Ok(HowUpdateCheckpointMessageResult::Skipped {
            reason: HowUpdateCheckpointMessageSkippedReason::NotCheckpoint,
        });
    }
    let mut guard = ctx.exclusive_worktree_access();
    how_update_checkpoint_message_by_change_id_with_perm(
        ctx,
        change_id,
        message,
        guard.write_permission(),
    )
}

/// Reset the current branch and worktree to a checkpoint commit.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_restore_checkpoint(
    ctx: &mut Context,
    checkpoint_id: gix::ObjectId,
    discard_changes: bool,
) -> anyhow::Result<()> {
    let _guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;

    if discard_changes {
        discard_current_workspace_changes(&repo, ctx.settings.context_lines)
            .context("failed to leave browsing changes behind")?;
    }

    safe_checkout_from_head(checkpoint_id, &repo, Default::default())
        .context("failed to restore checkpoint")?;
    Ok(())
}

/// List How bookmarks stored in local project metadata.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_list_bookmarks(ctx: &Context) -> anyhow::Result<Vec<HowBookmark>> {
    let repo = ctx.repo.get()?;
    list_bookmarks(&repo)
}

/// Create a How bookmark at the active HEAD commit.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_create_bookmark(
    ctx: &Context,
    name: String,
    kind: HowBookmarkKind,
) -> anyhow::Result<HowBookmark> {
    let repo = ctx.repo.get()?;
    let target_commit_id = repo
        .head_id()
        .context("How could not find the current project state.")?
        .detach();
    create_bookmark_at_commit(&repo, name, kind, target_commit_id)
}

/// Create a How bookmark at a specific commit.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_create_bookmark_from_commit(
    ctx: &Context,
    name: String,
    kind: HowBookmarkKind,
    commit_id: gix::ObjectId,
) -> anyhow::Result<HowBookmark> {
    let repo = ctx.repo.get()?;
    repo.find_commit(commit_id)
        .context("How could not find the bookmark state.")?;
    create_bookmark_at_commit(&repo, name, kind, commit_id)
}

/// Switch the internal active line to a bookmark and update the worktree.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_switch_bookmark(ctx: &mut Context, bookmark_id: String) -> anyhow::Result<()> {
    let _guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;
    ensure_active_branch(&repo)?;
    let target_commit_id = bookmark_target_commit_id(&repo, &bookmark_id)?;
    safe_checkout_from_head(target_commit_id, &repo, Default::default())
        .context("failed to switch bookmark")?;
    Ok(())
}

/// Update a How bookmark to the active HEAD commit.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_update_bookmark(ctx: &Context, bookmark_id: String) -> anyhow::Result<HowBookmark> {
    let repo = ctx.repo.get()?;
    let target_commit_id = repo
        .head_id()
        .context("How could not find the current project state.")?
        .detach();
    update_bookmark_target(&repo, &bookmark_id, target_commit_id)
}

/// Rename a How bookmark.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_rename_bookmark(
    ctx: &Context,
    bookmark_id: String,
    name: String,
) -> anyhow::Result<HowBookmark> {
    let repo = ctx.repo.get()?;
    rename_bookmark(&repo, &bookmark_id, name)
}

/// Delete a How bookmark pointer.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_delete_bookmark(ctx: &Context, bookmark_id: String) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    delete_bookmark(&repo, &bookmark_id)
}

/// Return whether the project has worktree changes.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_has_project_changes(ctx: &Context) -> anyhow::Result<bool> {
    let _guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    let changes = worktree_changes_no_renames(&repo)?;
    Ok(!changes.changes.is_empty()
        || !changes.ignored_changes.is_empty()
        || !changes.index_changes.is_empty()
        || !changes.index_conflicts.is_empty())
}

/// Produce the diff payload for summarizing the current checkpoint.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_staged_diff_for_checkpoint_summary(ctx: &Context) -> anyhow::Result<HowStagedDiff> {
    let _guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    let tree_id = worktree_tree(&repo)?;
    let head_tree_id = repo.head_tree_id_or_empty()?.detach();
    let diff = tree_diff_payload(&repo, head_tree_id, tree_id)?;
    let original_byte_count = diff.len();
    Ok(HowStagedDiff {
        diff,
        original_byte_count,
    })
}

/// Read How project settings from local Git config.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_read_project_settings(
    ctx: &Context,
    fallback: HowProjectSettings,
) -> anyhow::Result<HowProjectSettings> {
    let repo = ctx.repo.get()?;
    let config = repo.config_snapshot();
    Ok(HowProjectSettings {
        checkpoint_debounce_ms: config
            .integer("how.checkpointDebounceMs")
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(fallback.checkpoint_debounce_ms),
        coding_agent: config
            .string("how.codingAgent")
            .map(|value| value.to_string())
            .unwrap_or(fallback.coding_agent),
    })
}

/// Write How project settings to local Git config.
#[but_api(napi)]
#[instrument(skip(ctx), err(Debug))]
pub fn how_write_project_settings(
    ctx: &Context,
    settings: HowProjectSettings,
) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let (mut config, lock) = repo.local_common_config_for_editing()?;
    let checkpoint_debounce_ms = settings.checkpoint_debounce_ms.to_string();
    config.set_raw_value("how.checkpointDebounceMs", checkpoint_debounce_ms.as_str())?;
    config.set_raw_value("how.codingAgent", settings.coding_agent.as_str())?;
    repo.write_locked_config(&config, lock)?;
    Ok(())
}

fn discover_repo(path: &Path) -> anyhow::Result<gix::Repository> {
    gix::discover(path)
        .with_context(|| format!("failed to discover Git repository at '{}'", path.display()))
}

fn project_from_repo(repo: &gix::Repository) -> anyhow::Result<HowProject> {
    let worktree_path = repo
        .workdir()
        .context("How requires a non-bare Git repository")?;
    let git_dir = gix::path::realpath(repo.git_dir())?;
    let worktree_path = gix::path::realpath(worktree_path)?;
    let id = ProjectHandle::from_path(&git_dir)?.to_string();
    let title = worktree_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| worktree_path.display().to_string());
    Ok(HowProject {
        id,
        title,
        path: worktree_path.display().to_string(),
        git_dir: git_dir.display().to_string(),
    })
}

fn worktree_tree(repo: &gix::Repository) -> anyhow::Result<gix::ObjectId> {
    let changes = worktree_changes_no_renames(repo)?;
    let selection = changes
        .changes
        .iter()
        .map(|change| change.path.clone())
        .chain(
            changes
                .index_changes
                .iter()
                .map(|change| change.location().as_bstr().to_owned()),
        )
        .chain(changes.index_conflicts.iter().map(|(path, _)| path.clone()))
        .collect::<BTreeSet<_>>();
    let head_tree_id = repo.head_tree_id_or_empty()?;
    let outcome = create_tree(
        head_tree_id,
        State {
            changes,
            selection,
            head: false,
        },
    )?;
    Ok(outcome.worktree.unwrap_or(head_tree_id.detach()))
}

fn create_checkpoint_commit(
    repo: &gix::Repository,
    message: &str,
    tree_id: gix::ObjectId,
    parents: impl IntoIterator<Item = gix::ObjectId>,
) -> anyhow::Result<gix::ObjectId> {
    let (author, committer) = repo.commit_signatures()?;
    let headers = Headers::from_config(&repo.config_snapshot());
    let commit = gix::objs::Commit {
        message: message.into(),
        tree: tree_id,
        author,
        committer,
        encoding: None,
        parents: parents.into_iter().collect(),
        extra_headers: (&headers).into(),
    };
    let commit_id = but_rebase::commit::create(
        repo,
        commit,
        DateMode::CommitterKeepAuthorKeep,
        SignCommit::No,
    )?;
    update_current_head(repo, commit_id, message)?;
    Ok(commit_id)
}

fn update_current_head(
    repo: &gix::Repository,
    target: gix::ObjectId,
    message: &str,
) -> anyhow::Result<()> {
    match repo.head_name()? {
        Some(name) => {
            let name = name.as_bstr().to_string();
            repo.reference(
                name.as_str(),
                target,
                gix::refs::transaction::PreviousValue::Any,
                message,
            )?;
        }
        None => {
            repo.edit_reference(gix::refs::transaction::RefEdit {
                change: gix::refs::transaction::Change::Update {
                    log: gix::refs::transaction::LogChange {
                        mode: gix::refs::transaction::RefLog::AndReference,
                        force_create_reflog: false,
                        message: message.into(),
                    },
                    expected: gix::refs::transaction::PreviousValue::Any,
                    new: gix::refs::Target::Object(target),
                },
                name: "HEAD".try_into()?,
                deref: false,
            })?;
        }
    }
    Ok(())
}

fn validate_checkpoint_message(message: &str) -> anyhow::Result<()> {
    if message.starts_with(CHECKPOINT_PREFIX) {
        Ok(())
    } else {
        bail!("checkpoint message must start with '{CHECKPOINT_PREFIX}'")
    }
}

fn explicit_change_id_for_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Option<String>> {
    let commit = repo
        .find_commit(commit_id)
        .context("failed to load checkpoint commit")?;
    let commit = commit.decode()?;
    let headers = Headers::try_from_commit_headers(|| commit.extra_headers());
    Ok(headers.and_then(|headers| headers.change_id.map(|change_id| change_id.to_string())))
}

fn checkpoint_from_commit_id(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Option<HowCheckpoint>> {
    let commit = repo
        .find_commit(commit_id)
        .context("failed to load checkpoint commit")?;
    let message = commit
        .message()
        .context("failed to read checkpoint commit message")?;
    let title = message.title.to_string();
    if !title.starts_with(CHECKPOINT_PREFIX) {
        return Ok(None);
    }
    let decoded = commit.decode()?;
    let change_id = Headers::try_from_commit_headers(|| decoded.extra_headers())
        .and_then(|headers| headers.change_id.map(|change_id| change_id.to_string()));
    let created_at = commit.time()?.seconds.saturating_mul(1000);
    Ok(Some(HowCheckpoint {
        id: commit_id.to_string(),
        change_id,
        title,
        created_at,
    }))
}

fn visible_checkpoint_commit_id_by_change_id(
    repo: &gix::Repository,
    change_id: &str,
) -> anyhow::Result<Option<gix::ObjectId>> {
    let Ok(head) = repo.head_id() else {
        return Ok(None);
    };
    for info in head
        .ancestors()
        .first_parent_only()
        .all()
        .context("failed to walk checkpoint history")?
    {
        let info = info.context("failed to read commit from checkpoint history")?;
        let Some(checkpoint) = checkpoint_from_commit_id(repo, info.id)? else {
            continue;
        };
        if checkpoint.change_id.as_deref() == Some(change_id) {
            return Ok(Some(info.id));
        }
    }
    Ok(None)
}

fn commit_is_reachable_from_upstream(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> anyhow::Result<bool> {
    let Some(head_name) = repo.head_name()? else {
        return Ok(false);
    };
    let Ok(upstream_ref_name) =
        but_workspace::resolve_tracking_branch_ref_name(head_name.as_ref(), repo)
    else {
        return Ok(false);
    };
    let Some(mut upstream_ref) = repo.try_find_reference(upstream_ref_name.as_ref())? else {
        return Ok(false);
    };
    let upstream_tip = upstream_ref
        .peel_to_id()
        .context("failed to resolve upstream branch")?
        .detach();
    Ok(repo
        .merge_base(commit_id, upstream_tip)
        .ok()
        .map(|id| id.detach())
        == Some(commit_id))
}

fn how_update_checkpoint_message_by_change_id_with_perm(
    ctx: &mut Context,
    change_id: String,
    message: String,
    perm: &mut RepoExclusive,
) -> anyhow::Result<HowUpdateCheckpointMessageResult> {
    let commit_id = {
        let repo = ctx.repo.get()?;
        let Some(commit_id) = visible_checkpoint_commit_id_by_change_id(&repo, &change_id)? else {
            return Ok(HowUpdateCheckpointMessageResult::Skipped {
                reason: HowUpdateCheckpointMessageSkippedReason::NotFound,
            });
        };
        if commit_is_reachable_from_upstream(&repo, commit_id)? {
            return Ok(HowUpdateCheckpointMessageResult::Skipped {
                reason: HowUpdateCheckpointMessageSkippedReason::Published,
            });
        }
        commit_id
    };

    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = Editor::create(&mut ws, &mut meta, &repo)?;
    let (rebase, edited_commit_selector) =
        but_workspace::commit::reword(editor, commit_id, message.as_bytes().as_bstr())?;
    let new_commit_id = rebase.lookup_pick(edited_commit_selector)?;
    rebase.materialize()?;
    let checkpoint = checkpoint_from_commit_id(&repo, new_commit_id)?
        .context("updated commit is no longer a checkpoint")?;
    Ok(HowUpdateCheckpointMessageResult::Updated { checkpoint })
}

fn discard_current_workspace_changes(
    repo: &gix::Repository,
    context_lines: u32,
) -> anyhow::Result<()> {
    let specs = worktree_changes(repo)?
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect::<Vec<_>>();
    let refused = but_workspace::discard_workspace_changes(repo, specs, context_lines)?;
    if !refused.is_empty() {
        bail!("failed to discard all workspace changes before restoring checkpoint");
    }
    Ok(())
}

fn tree_diff_payload(
    repo: &gix::Repository,
    head_tree_id: gix::ObjectId,
    tree_id: gix::ObjectId,
) -> anyhow::Result<String> {
    if head_tree_id == tree_id {
        return Ok(String::new());
    }
    let changes = but_core::diff::tree_changes_with_line_stats(repo, Some(head_tree_id), tree_id)?
        .0
        .into_iter()
        .filter_map(|change| change.unified_diff(repo, 3).transpose())
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut out = String::new();
    for patch in changes {
        out.push_str(&String::from_utf8_lossy(&patch));
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out)
}

fn write_index_from_tree(repo: &gix::Repository, tree_id: gix::ObjectId) -> anyhow::Result<()> {
    let mut index = repo
        .index_from_tree(&tree_id)
        .context("failed to create index from checkpoint tree")?;
    index
        .write(Default::default())
        .context("failed to write checkpoint index")?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookmarkMetadata {
    id: String,
    name: String,
    created_at: i64,
    updated_at: i64,
    kind: HowBookmarkKind,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookmarkStore {
    version: u32,
    bookmarks: Vec<BookmarkMetadata>,
}

fn bookmarks_path(repo: &gix::Repository) -> anyhow::Result<PathBuf> {
    Ok(repo.gitbutler_storage_path()?.join(BOOKMARKS_FILE_NAME))
}

fn read_bookmark_store(repo: &gix::Repository) -> anyhow::Result<BookmarkStore> {
    let path = bookmarks_path(repo)?;
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("failed to read How bookmarks from '{}'", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(BookmarkStore {
            version: 1,
            bookmarks: Vec::new(),
        }),
        Err(error) => Err(error)
            .with_context(|| format!("failed to read How bookmarks from '{}'", path.display())),
    }
}

fn write_bookmark_store(repo: &gix::Repository, store: &BookmarkStore) -> anyhow::Result<()> {
    let path = bookmarks_path(repo)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(store).context("failed to serialize How bookmarks")?;
    fs::write(&path, format!("{raw}\n"))
        .with_context(|| format!("failed to write How bookmarks to '{}'", path.display()))?;
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn bookmark_ref_name(id: &str) -> anyhow::Result<gix::refs::FullName> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        bail!("Invalid bookmark id");
    }
    gix::refs::FullName::try_from(format!("{BOOKMARK_REF_PREFIX}/{id}"))
        .context("failed to build bookmark ref name")
}

fn list_bookmarks(repo: &gix::Repository) -> anyhow::Result<Vec<HowBookmark>> {
    let store = read_bookmark_store(repo)?;
    let current_commit_id = repo.head_id().ok().map(|id| id.detach());
    let mut bookmarks = Vec::new();
    for metadata in store.bookmarks {
        let ref_name = bookmark_ref_name(&metadata.id)?;
        let Some(mut reference) = repo.try_find_reference(ref_name.as_ref())? else {
            continue;
        };
        let target_commit_id = reference
            .peel_to_id()
            .with_context(|| format!("failed to resolve bookmark '{}'", metadata.id))?
            .detach();
        bookmarks.push(HowBookmark {
            id: metadata.id,
            name: metadata.name,
            target_commit_id: target_commit_id.to_string(),
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
            kind: metadata.kind,
            is_current: current_commit_id == Some(target_commit_id),
        });
    }
    bookmarks.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(bookmarks)
}

fn create_bookmark_at_commit(
    repo: &gix::Repository,
    name: String,
    kind: HowBookmarkKind,
    target_commit_id: gix::ObjectId,
) -> anyhow::Result<HowBookmark> {
    repo.find_commit(target_commit_id)
        .context("How could not find the bookmark state.")?;
    let mut store = read_bookmark_store(repo)?;
    let id = uuid::Uuid::new_v4().to_string();
    let ref_name = bookmark_ref_name(&id)?;
    repo.reference(
        ref_name.as_ref(),
        target_commit_id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "How create bookmark",
    )
    .context("failed to create bookmark ref")?;
    let now = now_ms();
    store.bookmarks.push(BookmarkMetadata {
        id: id.clone(),
        name: name.trim().to_owned(),
        created_at: now,
        updated_at: now,
        kind,
    });
    write_bookmark_store(repo, &store)?;
    list_bookmarks(repo)?
        .into_iter()
        .find(|bookmark| bookmark.id == id)
        .context("created bookmark was not found")
}

fn bookmark_metadata_mut<'a>(
    store: &'a mut BookmarkStore,
    bookmark_id: &str,
) -> anyhow::Result<&'a mut BookmarkMetadata> {
    store
        .bookmarks
        .iter_mut()
        .find(|bookmark| bookmark.id == bookmark_id)
        .with_context(|| format!("How could not find bookmark '{bookmark_id}'."))
}

fn bookmark_target_commit_id(
    repo: &gix::Repository,
    bookmark_id: &str,
) -> anyhow::Result<gix::ObjectId> {
    let mut reference = repo
        .find_reference(bookmark_ref_name(bookmark_id)?.as_ref())
        .with_context(|| format!("How could not find bookmark '{bookmark_id}'."))?;
    Ok(reference
        .peel_to_id()
        .context("failed to resolve bookmark ref")?
        .detach())
}

fn update_bookmark_target(
    repo: &gix::Repository,
    bookmark_id: &str,
    target_commit_id: gix::ObjectId,
) -> anyhow::Result<HowBookmark> {
    let mut store = read_bookmark_store(repo)?;
    {
        let metadata = bookmark_metadata_mut(&mut store, bookmark_id)?;
        metadata.updated_at = now_ms();
        metadata.kind = HowBookmarkKind::User;
    }
    let ref_name = bookmark_ref_name(bookmark_id)?;
    let previous = bookmark_target_commit_id(repo, bookmark_id)?;
    repo.reference(
        ref_name.as_ref(),
        target_commit_id,
        gix::refs::transaction::PreviousValue::MustExistAndMatch(previous.into()),
        "How update bookmark",
    )
    .context("failed to update bookmark ref")?;
    write_bookmark_store(repo, &store)?;
    list_bookmarks(repo)?
        .into_iter()
        .find(|bookmark| bookmark.id == bookmark_id)
        .context("updated bookmark was not found")
}

fn rename_bookmark(
    repo: &gix::Repository,
    bookmark_id: &str,
    name: String,
) -> anyhow::Result<HowBookmark> {
    let mut store = read_bookmark_store(repo)?;
    {
        let metadata = bookmark_metadata_mut(&mut store, bookmark_id)?;
        metadata.name = name.trim().to_owned();
        metadata.updated_at = now_ms();
        metadata.kind = HowBookmarkKind::User;
    }
    write_bookmark_store(repo, &store)?;
    list_bookmarks(repo)?
        .into_iter()
        .find(|bookmark| bookmark.id == bookmark_id)
        .context("renamed bookmark was not found")
}

fn delete_bookmark(repo: &gix::Repository, bookmark_id: &str) -> anyhow::Result<()> {
    let mut store = read_bookmark_store(repo)?;
    let ref_name = bookmark_ref_name(bookmark_id)?;
    if let Some(mut reference) = repo.try_find_reference(ref_name.as_ref())? {
        let target = reference.peel_to_id()?.detach();
        repo.edit_reference(gix::refs::transaction::RefEdit {
            change: gix::refs::transaction::Change::Delete {
                expected: gix::refs::transaction::PreviousValue::MustExistAndMatch(target.into()),
                log: gix::refs::transaction::RefLog::AndReference,
            },
            name: ref_name,
            deref: false,
        })
        .context("failed to delete bookmark ref")?;
    }
    store
        .bookmarks
        .retain(|bookmark| bookmark.id != bookmark_id);
    write_bookmark_store(repo, &store)
}

fn ensure_active_branch(repo: &gix::Repository) -> anyhow::Result<()> {
    let head_name = repo
        .head_name()
        .context("failed to read project state")?
        .context("How needs the project to be on its active line.")?;
    if head_name.as_bstr() != ACTIVE_BRANCH_REF.as_bytes() {
        bail!("How needs the project to be on its active line.");
    }
    Ok(())
}

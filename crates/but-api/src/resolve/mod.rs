//! Inspect and resolve the conflicts of a conflicted commit — without edit mode.
//!
//! GitButler keeps conflicts as first-class committed state: a conflicted
//! commit carries its merge inputs as trees. This module re-merges those trees
//! in memory to expose the conflicts as per-file hunks
//! ([`commit_conflicts()`]), and applies per-hunk resolutions — a side pick or
//! custom content, for some or all hunks — by narrowing the conflict and
//! rewriting the commit, rebasing its descendants
//! ([`resolve_commit_conflict_hunks()`]). A partial resolution leaves the
//! commit conflicted with fewer conflicts, so callers can work incrementally.
//!
//! [`resolve_commit_conflicts_ai()`] is a client of the same core: it sends
//! the hunks to the configured LLM as a single-shot, no-tools request and
//! applies the returned full-coverage resolutions. In every case the workspace
//! never enters edit mode, the exclusive worktree lock is held only for the
//! apply step, and an oplog snapshot records an undo point.

use std::collections::BTreeMap;

use anyhow::{Context as _, bail};
use but_api_macros::but_api;
use but_core::DryRun;
use but_core::sync::RepoExclusive;
use but_llm::{ChatMessage, LLMProvider};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use gix::objs::tree::EntryKind;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::WorkspaceState;

mod apply;
mod context;
mod prompt;

pub use context::{ConflictHunk, FileConflict, ResolutionRequest};
pub use prompt::{FileResolution, HunkContent, ResolutionResponse, SYSTEM_PROMPT};

/// The conflicts of a conflicted commit, as per-file hunks.
#[derive(Debug, Clone)]
pub struct CommitConflicts {
    /// The conflicted commit.
    pub commit_id: gix::ObjectId,
    /// The conflicted files, sorted by path.
    pub files: Vec<ConflictedFile>,
}

/// One conflicted file and its conflicts, in file order.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConflictedFile {
    /// The repo-relative path of the file.
    pub path: String,
    /// The file's change from the current base's version (*ours*) to the
    /// commit's own version (*theirs*). Regular commit diffs are computed
    /// against the auto-resolution, which keeps the base's version wherever
    /// there is a conflict — so this is the change to diff to actually see
    /// the conflicted content.
    pub change: but_core::ui::TreeChange,
    /// The conflicts of the file; hunks are addressed by their 1-based
    /// position in this list.
    pub hunks: Vec<ConflictHunk>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ConflictedFile);

/// Return the conflicts of the conflicted commit `commit_id` without entering
/// edit mode or touching the working tree.
///
/// Hunks are identified by `(path, 1-based index)`; the extraction is
/// deterministic, so the same commit id always yields the same hunks and
/// [`resolve_commit_conflict_hunks()`] can be called with indices from this
/// result. Fails for commits whose conflicts have no hunk representation
/// (deletions/renames, binaries, oversized files, marker-like content) — those
/// need manual resolution in edit mode.
#[but_api(try_from = crate::resolve::json::CommitConflicts)]
#[instrument(err(Debug))]
pub fn commit_conflicts(
    ctx: &but_ctx::Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<CommitConflicts> {
    let _guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    let request = context::build_request(&repo, commit_id)?;
    let ours_tree = repo.find_tree(request.ours_tree_id)?;
    let theirs_tree = repo.find_tree(request.theirs_tree_id)?;
    let files = request
        .files
        .into_iter()
        .map(|file| {
            let previous_state = conflict_side_state(&ours_tree, &file.rela_path)?;
            let state = conflict_side_state(&theirs_tree, &file.rela_path)?;
            let flags = match (previous_state.kind, state.kind) {
                (EntryKind::Blob, EntryKind::BlobExecutable) => {
                    Some(but_core::ui::ModeFlags::ExecutableBitAdded)
                }
                (EntryKind::BlobExecutable, EntryKind::Blob) => {
                    Some(but_core::ui::ModeFlags::ExecutableBitRemoved)
                }
                _ => None,
            };
            Ok(ConflictedFile {
                path: file.path,
                change: but_core::ui::TreeChange {
                    path: file.rela_path.clone().into(),
                    path_bytes: file.rela_path,
                    status: but_core::ui::TreeStatus::Modification {
                        previous_state,
                        state,
                        flags,
                    },
                },
                hunks: file.hunks,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(CommitConflicts {
        commit_id: request.commit_id,
        files,
    })
}

/// The blob of a conflicted path on one side of the conflict. Both sides are
/// present for every file that [`context::build_request()`] returns — side
/// deletions bail to manual resolution there.
fn conflict_side_state(
    tree: &gix::Tree<'_>,
    rela_path: &bstr::BString,
) -> anyhow::Result<but_core::ui::ChangeState> {
    let entry = tree
        .lookup_entry(rela_path.split(|byte| *byte == b'/'))?
        .with_context(|| format!("Conflicted path {rela_path:?} is missing from a side tree"))?;
    Ok(but_core::ui::ChangeState {
        id: entry.object_id(),
        kind: entry.mode().kind(),
    })
}

/// How to resolve a single conflict hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum HunkResolution {
    /// Take the *ours* side, i.e. the new base the commit was rebased onto.
    Ours,
    /// Take the *theirs* side, i.e. the commit's own version.
    Theirs,
    /// Replace the conflict with this content (for mixed resolutions; an
    /// empty string deletes the conflicted region).
    Content(String),
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HunkResolution);

/// One conflict to resolve, addressed by path and 1-based hunk index as
/// returned by [`commit_conflicts()`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ResolutionSpec {
    /// The repo-relative path of the conflicted file.
    pub path: String,
    /// The 1-based index of the conflict within the file.
    pub hunk: usize,
    /// How to resolve it.
    pub resolution: HunkResolution,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ResolutionSpec);

/// Conflicts still unresolved in a file after an apply.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RemainingConflicts {
    /// The repo-relative path of the file.
    pub path: String,
    /// How many conflicts remain in it.
    pub hunks: usize,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RemainingConflicts);

/// The outcome of applying per-hunk resolutions to a conflicted commit.
#[derive(Debug, Clone)]
pub struct HunkResolutionResult {
    /// The conflicted commit the resolutions were applied to.
    pub commit_id: gix::ObjectId,
    /// The rewritten commit. Still conflicted when `remaining` is non-empty.
    pub new_commit: gix::ObjectId,
    /// How many conflicts were resolved.
    pub resolved: usize,
    /// The conflicts that remain, per file. Empty when the commit is fully
    /// resolved.
    pub remaining: Vec<RemainingConflicts>,
    /// Workspace state after the apply.
    pub workspace: WorkspaceState,
}

/// Apply `specs` to the conflicted commit `commit_id` and rebase descendants.
///
/// Resolving a subset of the conflicts rewrites the commit into a conflicted
/// commit with only the remaining conflicts; resolving all of them rewrites it
/// into a normal commit. Either way the commit id changes — address follow-up
/// resolutions to the returned `new_commit`. An oplog snapshot records an undo
/// point. Nothing is written if any spec fails validation.
#[but_api(try_from = crate::resolve::json::HunkResolutionResult)]
#[instrument(skip(specs), err(Debug))]
pub fn resolve_commit_conflict_hunks(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    specs: Vec<ResolutionSpec>,
    dry_run: DryRun,
) -> anyhow::Result<HunkResolutionResult> {
    if specs.is_empty() {
        bail!("No resolutions were provided");
    }
    let request = {
        let _guard = ctx.shared_worktree_access();
        let repo = ctx.repo.get()?;
        context::build_request(&repo, commit_id)?
    };
    let picks = apply::validate_specs(&request, &specs)?;
    // Every spec addressed a distinct hunk, or validation would have failed.
    let resolved = specs.len();

    let mut guard = ctx.exclusive_worktree_access();
    ctx.invalidate_workspace_cache()?;
    let applied = apply_with_snapshot(
        ctx,
        &request,
        &picks,
        OperationKind::ResolveConflicts,
        dry_run,
        guard.write_permission(),
    )?;

    Ok(HunkResolutionResult {
        commit_id: request.commit_id,
        new_commit: applied.new_commit,
        resolved,
        remaining: applied.remaining,
        workspace: applied.workspace,
    })
}

/// How one conflicted file was resolved, for display to the user.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFile {
    /// The repo-relative path of the file.
    pub path: String,
    /// The content that replaced each conflict block, in file order.
    pub hunks: Vec<String>,
    /// The model's explanation of its decision for this file.
    pub reasoning: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ResolvedFile);

/// The outcome of resolving a conflicted commit with AI.
#[derive(Debug, Clone)]
pub struct AiResolutionResult {
    /// The conflicted commit that was resolved.
    pub commit_id: gix::ObjectId,
    /// The rewritten, no-longer-conflicted commit.
    pub new_commit: gix::ObjectId,
    /// A short model-authored markdown summary of the conflict and resolution.
    pub summary: Option<String>,
    /// The per-file resolutions that were applied.
    pub files: Vec<ResolvedFile>,
    /// Workspace state after the resolution.
    pub workspace: WorkspaceState,
}

/// Resolve all conflicts of the conflicted commit `commit_id` using the LLM
/// configured in the user's git configuration, apply the result, and rebase
/// descendants.
///
/// This acquires shared worktree access while gathering the conflicts, holds
/// no lock during the model call, and acquires exclusive worktree access for
/// the apply step. An oplog snapshot records an undo point on success. When
/// `dry_run` is enabled, the returned workspace previews the resolution
/// without materializing the rebase and no oplog entry is persisted.
///
/// Fails without changing anything if no AI provider is configured, if the
/// commit is not conflicted, if a conflict cannot be resolved automatically
/// (deletions, binaries, oversized files), or if the model response does not
/// validate against the request after one retry.
#[but_api(try_from = crate::resolve::json::AiResolutionResult)]
#[instrument(err(Debug))]
pub fn resolve_commit_conflicts_ai(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    dry_run: DryRun,
) -> anyhow::Result<AiResolutionResult> {
    // The repository's resolved configuration, so repo-local AI settings
    // (`but config ai --local`) take precedence over global ones.
    let llm = {
        let repo = ctx.repo.get()?;
        let config = repo.config_snapshot();
        LLMProvider::from_git_config(config.plumbing()).context(
            "AI is not configured. Configure an AI provider in the GitButler settings first.",
        )?
    };
    resolve_commit_conflicts_with(ctx, commit_id, dry_run, |request| {
        let model = llm.model_or_default();
        llm.structured_output::<ResolutionResponse>(
            SYSTEM_PROMPT,
            vec![ChatMessage::User(prompt::render_user_message(request))],
            &model,
        )?
        .context("The AI model returned no response")
    })
}

/// Like [`resolve_commit_conflicts_ai()`], but with the model call injected as
/// `resolve`, so tests and other frontends can supply resolutions without
/// network access.
///
/// `resolve` is called once, and once more if it errors (truncated or
/// malformed model output) or if its response fails validation against the
/// request.
pub fn resolve_commit_conflicts_with(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    dry_run: DryRun,
    resolve: impl Fn(&ResolutionRequest) -> anyhow::Result<ResolutionResponse>,
) -> anyhow::Result<AiResolutionResult> {
    let request = {
        let _guard = ctx.shared_worktree_access();
        let repo = ctx.repo.get()?;
        context::build_request(&repo, commit_id)?
    };

    // The model call happens without any worktree lock; the request is plain
    // data and the apply step below re-reads the workspace under the exclusive
    // lock, so it fails closed if the commit changed in the meantime.
    let attempt = || -> anyhow::Result<_> {
        let response = resolve(&request)?;
        let validated = validate_ai_response(&request, &response)?;
        Ok((validated, response.summary))
    };
    let ((picks, files), summary) = match attempt() {
        Ok(outcome) => outcome,
        Err(first_failure) => {
            tracing::warn!(
                error = ?first_failure,
                "AI conflict-resolution attempt failed, retrying once"
            );
            attempt()?
        }
    };

    let mut guard = ctx.exclusive_worktree_access();
    // The workspace graph may have been cached before or during the model
    // call; the commit-presence check in apply must run against the state
    // under this exclusive lock, not a stale cache.
    ctx.invalidate_workspace_cache()?;
    let applied = apply_with_snapshot(
        ctx,
        &request,
        &picks,
        OperationKind::ResolveConflictsAi,
        dry_run,
        guard.write_permission(),
    )?;

    Ok(AiResolutionResult {
        commit_id: request.commit_id,
        new_commit: applied.new_commit,
        summary,
        files,
        workspace: applied.workspace,
    })
}

/// Check the model response against the request and translate it into
/// full-coverage picks plus the per-file display payload. Any mismatch fails
/// validation so the caller can retry the model once before giving up.
fn validate_ai_response(
    request: &ResolutionRequest,
    response: &ResolutionResponse,
) -> anyhow::Result<(apply::PicksPerFile, Vec<ResolvedFile>)> {
    let files_by_path = apply::index_files_by_path(request)?;
    let mut picks: apply::PicksPerFile = vec![BTreeMap::new(); request.files.len()];
    let mut resolved_files: Vec<Option<ResolvedFile>> = vec![None; request.files.len()];

    for resolution in &response.resolutions {
        let Some(&index) = files_by_path.get(&apply::normalize_path(&resolution.path)) else {
            bail!(
                "The model returned a resolution for \"{}\", which was not requested",
                resolution.path
            );
        };
        if resolved_files[index].is_some() {
            bail!(
                "The model returned more than one resolution for \"{}\"",
                resolution.path
            );
        }
        let file = &request.files[index];
        if resolution.hunks.len() != file.hunks.len() {
            bail!(
                "The model returned {} resolved hunks for \"{}\" but the file has {} conflicts",
                resolution.hunks.len(),
                file.path,
                file.hunks.len()
            );
        }
        if resolution.reasoning.trim().is_empty() {
            bail!("The model returned no reasoning for \"{}\"", file.path);
        }
        for (hunk_index, hunk) in resolution.hunks.iter().enumerate() {
            apply::ensure_no_markers(&hunk.resolved_content, &file.path)?;
            picks[index].insert(
                hunk_index,
                apply::HunkPick::Content(hunk.resolved_content.clone()),
            );
        }
        resolved_files[index] = Some(ResolvedFile {
            path: file.path.clone(),
            hunks: resolution
                .hunks
                .iter()
                .map(|hunk| hunk.resolved_content.clone())
                .collect(),
            reasoning: resolution.reasoning.clone(),
        });
    }

    if let Some(missing) = resolved_files.iter().position(Option::is_none) {
        bail!(
            "The model returned no resolution for the conflicted file \"{}\"",
            request.files[missing].path
        );
    }

    Ok((picks, resolved_files.into_iter().flatten().collect()))
}

/// Record an undo point, apply `picks`, and commit the snapshot on success.
fn apply_with_snapshot(
    ctx: &mut but_ctx::Context,
    request: &ResolutionRequest,
    picks: &apply::PicksPerFile,
    operation: OperationKind,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> anyhow::Result<apply::AppliedResolution> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(operation),
        perm.read_permission(),
        dry_run,
    );

    let res = apply::apply(ctx, request, picks, dry_run, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}

/// JSON transport types for this module.
pub mod json {
    use serde::Serialize;

    use crate::json::HexHash;

    /// JSON transport type for the conflicts of a conflicted commit.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct CommitConflicts {
        /// The conflicted commit.
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        pub commit_id: HexHash,
        /// The conflicted files, sorted by path.
        pub files: Vec<super::ConflictedFile>,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(CommitConflicts);

    impl TryFrom<super::CommitConflicts> for CommitConflicts {
        type Error = anyhow::Error;

        fn try_from(value: super::CommitConflicts) -> Result<Self, Self::Error> {
            let super::CommitConflicts { commit_id, files } = value;
            Ok(Self {
                commit_id: commit_id.into(),
                files,
            })
        }
    }

    /// JSON transport type for the outcome of applying per-hunk resolutions.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct HunkResolutionResult {
        /// The conflicted commit the resolutions were applied to.
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        pub commit_id: HexHash,
        /// The rewritten commit. Still conflicted when `remaining` is non-empty.
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        pub new_commit: HexHash,
        /// How many conflicts were resolved.
        pub resolved: usize,
        /// The conflicts that remain, per file.
        pub remaining: Vec<super::RemainingConflicts>,
        /// Workspace state after the apply.
        pub workspace: crate::json::WorkspaceState,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(HunkResolutionResult);

    impl TryFrom<super::HunkResolutionResult> for HunkResolutionResult {
        type Error = anyhow::Error;

        fn try_from(value: super::HunkResolutionResult) -> Result<Self, Self::Error> {
            let super::HunkResolutionResult {
                commit_id,
                new_commit,
                resolved,
                remaining,
                workspace,
            } = value;
            Ok(Self {
                commit_id: commit_id.into(),
                new_commit: new_commit.into(),
                resolved,
                remaining,
                workspace: workspace.try_into()?,
            })
        }
    }

    /// JSON transport type for the outcome of an AI conflict resolution.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct AiResolutionResult {
        /// The conflicted commit that was resolved.
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        pub commit_id: HexHash,
        /// The rewritten, no-longer-conflicted commit.
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        pub new_commit: HexHash,
        /// A short model-authored markdown summary of the conflict and resolution.
        pub summary: Option<String>,
        /// The per-file resolutions that were applied.
        pub files: Vec<super::ResolvedFile>,
        /// Workspace state after the resolution.
        pub workspace: crate::json::WorkspaceState,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(AiResolutionResult);

    impl TryFrom<super::AiResolutionResult> for AiResolutionResult {
        type Error = anyhow::Error;

        fn try_from(value: super::AiResolutionResult) -> Result<Self, Self::Error> {
            let super::AiResolutionResult {
                commit_id,
                new_commit,
                summary,
                files,
                workspace,
            } = value;

            Ok(Self {
                commit_id: commit_id.into(),
                new_commit: new_commit.into(),
                summary,
                files,
                workspace: workspace.try_into()?,
            })
        }
    }
}

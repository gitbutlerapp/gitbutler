//! Ephemeral comments anchored to lines in diffs, shared between the GUI (which creates them)
//! and the CLI (where agents read and archive them).
//!
//! A comment is anchored to a line in a diff: either the uncommitted worktree diff of a file, or
//! the first-parent diff of a commit identified by its change-id (which survives amends and
//! rebases). Anchors are not durable — edits shift line numbers and hunks reshape — so every
//! anchor also snapshots the anchored line's content (and its neighbours, to disambiguate
//! identical lines). [`list_comments`] re-locates each comment in the current diff by content
//! before returning it, persists the refreshed position, and archives comments whose anchor no
//! longer exists (file committed, line gone). Consumers therefore only ever see comments that
//! point at real lines in the current diffs; everything else is best-effort auto-archived,
//! befitting their ephemeral nature.
//!
//! Comments are stored in a plain JSON file in the project data directory (see
//! [`CommentStore`]), not the project database — deliberately the lightest possible storage
//! while the feature proves itself.
#![deny(missing_docs)]

mod anchor;
mod store;

use anchor::FileDiffLines;
use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use gix::prelude::ObjectIdExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use store::{CommentStore, StoredComment};

/// How long archived comments are kept around before being purged on the next list call.
const PURGE_ARCHIVED_AFTER_MS: i64 = 14 * 24 * 60 * 60 * 1000;

/// The side of a diff a comment line lives on: `old` line numbers count in the pre-image,
/// `new` line numbers in the post-image. Context lines exist on both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum DiffSide {
    /// The pre-image side, i.e. removed lines and context lines.
    Old,
    /// The post-image side, i.e. added lines and context lines.
    New,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DiffSide);

impl DiffSide {
    fn as_str(&self) -> &'static str {
        match self {
            DiffSide::Old => "old",
            DiffSide::New => "new",
        }
    }
}

/// A comment anchored to a line in a diff, as returned to every consumer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DiffComment {
    /// The unique identifier of the comment.
    pub id: String,
    /// The worktree-relative path of the file the comment is anchored to.
    pub path: String,
    /// `None` when the comment is anchored to the uncommitted worktree diff, or the change-id of
    /// the commit whose first-parent diff the comment is anchored to.
    pub commit_change_id: Option<String>,
    /// The side of the diff the anchored line lives on.
    pub side: DiffSide,
    /// The 1-based line number of the anchored line, in `side`'s coordinates.
    pub line_number: u32,
    /// The content of the anchored line (without the leading `+`/`-`/space diff marker).
    pub line_content: String,
    /// The comment text itself.
    pub payload: String,
    /// When the comment was created, in milliseconds since the Unix epoch (UTC).
    pub created_at_ms: i64,
    /// When the comment payload was last updated, in milliseconds since the Unix epoch (UTC).
    pub updated_at_ms: i64,
    /// A unified-diff-formatted excerpt of the current diff around the anchored line, so consumers
    /// can understand what the comment is about without recomputing the diff.
    /// Only present on comments returned from [`list_comments`].
    pub context: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DiffComment);

/// Everything needed to create a new comment. See [`DiffComment`] for the field semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NewComment {
    /// The worktree-relative path of the file to anchor the comment to.
    pub path: String,
    /// `None` to anchor to the uncommitted worktree diff, or the change-id of a workspace commit
    /// to anchor to that commit's first-parent diff.
    pub commit_change_id: Option<String>,
    /// The side of the diff the anchored line lives on.
    pub side: DiffSide,
    /// The 1-based line number of the line to anchor to, in `side`'s coordinates.
    pub line_number: u32,
    /// The comment text.
    pub payload: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(NewComment);

/// Create a new comment anchored to a line in a diff.
///
/// The anchor must point at a line that exists in the current diff — the uncommitted worktree
/// diff of `path`, or the first-parent diff of the workspace commit with `commit_change_id` —
/// otherwise an error is returned. The anchored line's content (and its neighbours) is
/// snapshotted so the comment can be re-located when the diff drifts.
pub fn create_comment(
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    store: &CommentStore,
    comment: NewComment,
    context_lines: u32,
    now_ms: i64,
) -> anyhow::Result<DiffComment> {
    let scope = anchor_scope_display(&comment.commit_change_id, &comment.path);
    let mut diffs = ScopeDiffs::new(repo, workspace, context_lines);
    let Some(anchor) = diffs.file(comment.commit_change_id.as_deref(), &comment.path)? else {
        bail!(
            "Commit {} is not in the applied workspace",
            comment.commit_change_id.as_deref().unwrap_or_default()
        );
    };
    let file = match anchor {
        FileAnchor::Lines(lines) => lines,
        FileAnchor::Gone => bail!("Nothing to anchor a comment to in {scope}"),
        FileAnchor::Unanchorable => {
            bail!("Cannot comment on {scope}: the diff is binary or too large")
        }
    };
    let line = file
        .line_at(comment.side, comment.line_number)
        .with_context(|| {
            format!(
                "No line {} on the {} side in {scope}",
                comment.line_number,
                comment.side.as_str(),
            )
        })?;

    // Snapshot the same-side neighbouring lines too: they disambiguate between identical
    // lines when the comment is re-located later.
    let line_before = (comment.line_number > 1)
        .then(|| file.line_at(comment.side, comment.line_number - 1))
        .flatten()
        .map(|line| line.content.clone());
    let line_after = file
        .line_at(comment.side, comment.line_number + 1)
        .map(|line| line.content.clone());

    let stored = StoredComment {
        id: uuid::Uuid::new_v4().to_string(),
        path: comment.path,
        commit_change_id: comment.commit_change_id,
        side: comment.side,
        line_number: comment.line_number,
        line_content: line.content.clone(),
        line_before,
        line_after,
        payload: comment.payload,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
        archived_at_ms: None,
    };
    let result = stored.to_comment(stored.line_number, file.context_excerpt(line));
    store.update(|comments| {
        comments.push(stored);
        Ok(())
    })?;
    Ok(result)
}

impl StoredComment {
    /// The consumer-facing view of this comment, re-anchored at `line_number` and carrying the
    /// diff excerpt around it.
    fn to_comment(&self, line_number: u32, context: String) -> DiffComment {
        DiffComment {
            id: self.id.clone(),
            path: self.path.clone(),
            commit_change_id: self.commit_change_id.clone(),
            side: self.side,
            line_number,
            line_content: self.line_content.clone(),
            payload: self.payload.clone(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
            context: Some(context),
        }
    }
}

/// List all unarchived comments, re-anchored against the current diffs.
///
/// Each returned comment is guaranteed to point at a line that exists in the current diff of its
/// file, with `line_number` refreshed (and persisted) if the line drifted, and `context` filled
/// with an excerpt of the surrounding diff. Comments whose anchor cannot be found anymore — the
/// line's content is gone from the diff, or the file has no uncommitted changes anymore — are
/// archived and not returned. Comments whose anchor scope cannot even be resolved right now
/// (the anchored commit's branch is not applied) are neither listed nor archived: they come back
/// when the branch does.
pub fn list_comments(
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    store: &CommentStore,
    context_lines: u32,
    now_ms: i64,
) -> anyhow::Result<Listing> {
    let all = store.read();
    let purge_cutoff_ms = now_ms - PURGE_ARCHIVED_AFTER_MS;
    let purge_needed = all.iter().any(|comment| {
        comment
            .archived_at_ms
            .is_some_and(|at| at < purge_cutoff_ms)
    });

    enum Outcome {
        Keep { comment: DiffComment, drifted: bool },
        Archive { id: String },
    }
    let mut diffs = ScopeDiffs::new(repo, workspace, context_lines);
    let mut outcomes = Vec::new();
    for row in all.into_iter().filter(|c| c.archived_at_ms.is_none()) {
        // An unresolvable scope (e.g. the commit's branch is unapplied) leaves the comment
        // completely untouched: it comes back when the scope does.
        let Some(anchor) = diffs.file(row.commit_change_id.as_deref(), &row.path)? else {
            continue;
        };
        let lines = match anchor {
            FileAnchor::Lines(lines) => lines,
            // The anchor is genuinely gone from this scope.
            FileAnchor::Gone => {
                outcomes.push(Outcome::Archive { id: row.id });
                continue;
            }
            // Binary or too large: we cannot know whether the anchor survived, so the comment
            // is hidden rather than destroyed — it comes back if the file becomes diffable.
            FileAnchor::Unanchorable => continue,
        };
        let located = lines.locate(
            row.side,
            row.line_number,
            &row.line_content,
            row.line_before.as_deref(),
            row.line_after.as_deref(),
        );
        match located {
            Some(line) => {
                let drifted = line.line_number != row.line_number;
                let comment = row.to_comment(line.line_number, lines.context_excerpt(line));
                outcomes.push(Outcome::Keep { comment, drifted });
            }
            None => outcomes.push(Outcome::Archive { id: row.id }),
        }
    }

    let needs_write = purge_needed
        || outcomes.iter().any(|outcome| match outcome {
            Outcome::Keep { drifted, .. } => *drifted,
            Outcome::Archive { .. } => true,
        });
    let mut persisted_changes = false;
    let mut result = Vec::new();
    if needs_write {
        // Mutations are applied to the freshly re-read state inside the lock, field by field,
        // so payload edits or archiving that happened while we were re-anchoring survive.
        // Best-effort: listing is conceptually a read, so a failing persist step (e.g. a held
        // lock) must not fail it — the same drift/archiving is recomputed on the next call.
        let persisted = store.update(|comments| {
            for outcome in &outcomes {
                match outcome {
                    Outcome::Keep {
                        comment,
                        drifted: true,
                    } => {
                        if let Some(stored) = comments
                            .iter_mut()
                            .find(|c| c.id == comment.id && c.archived_at_ms.is_none())
                        {
                            stored.line_number = comment.line_number;
                        }
                    }
                    Outcome::Keep { .. } => {}
                    Outcome::Archive { id } => {
                        if let Some(stored) = comments
                            .iter_mut()
                            .find(|c| &c.id == id && c.archived_at_ms.is_none())
                        {
                            stored.archived_at_ms = Some(now_ms);
                        }
                    }
                }
            }
            comments.retain(|c| c.archived_at_ms.is_none_or(|at| at >= purge_cutoff_ms));
            Ok(())
        });
        match persisted {
            Ok(()) => persisted_changes = true,
            Err(err) => {
                tracing::warn!(%err, "could not persist re-anchored comments; returning them anyway");
            }
        }
    }
    for outcome in outcomes {
        if let Outcome::Keep { comment, .. } = outcome {
            result.push(comment);
        }
    }
    Ok(Listing {
        comments: result,
        persisted_changes,
    })
}

/// The result of [`list_comments`].
pub struct Listing {
    /// The re-anchored, unarchived comments.
    pub comments: Vec<DiffComment>,
    /// Whether the listing wrote to the store (persisted drift, auto-archived comments, or
    /// purged old archived rows). Callers that bridge processes can use this to notify other
    /// consumers of the store.
    pub persisted_changes: bool,
}

/// Replace the payload of the unarchived comment with the given `id`.
pub fn update_payload(
    store: &CommentStore,
    id: &str,
    payload: String,
    now_ms: i64,
) -> anyhow::Result<()> {
    store.update(|comments| {
        let Some(comment) = comments.iter_mut().find(|c| c.id == id) else {
            bail!("No comment with id {id}");
        };
        if comment.archived_at_ms.is_some() {
            bail!("Comment {id} is archived and cannot be updated");
        }
        comment.payload = payload;
        comment.updated_at_ms = now_ms;
        Ok(())
    })
}

/// Archive the comment with the given `id`, hiding it from all future listings.
/// Returns `false` if the comment does not exist or was already archived.
pub fn archive_comment(store: &CommentStore, id: &str, now_ms: i64) -> anyhow::Result<bool> {
    store.update(|comments| {
        Ok(
            match comments
                .iter_mut()
                .find(|c| c.id == id && c.archived_at_ms.is_none())
            {
                Some(comment) => {
                    comment.archived_at_ms = Some(now_ms);
                    true
                }
                None => false,
            },
        )
    })
}

fn anchor_scope_display(commit_change_id: &Option<String>, path: &str) -> String {
    match commit_change_id {
        None => format!("the uncommitted changes of {path}"),
        Some(change_id) => format!("the diff of {path} in commit {change_id}"),
    }
}

/// The anchorable diff lines of one path within a scope, or why they could not be produced.
enum FileAnchor {
    /// The file has a diff with anchorable lines.
    Lines(FileDiffLines),
    /// The file has no diff in this scope: anchors in it are genuinely gone.
    Gone,
    /// The file's diff is binary or too large, so anchor survival cannot be judged.
    Unanchorable,
}

/// The lazily-computed diff state of one operation: the worktree status, the change-id index,
/// and each commit's and file's diff are computed at most once, no matter how many comments an
/// operation touches.
struct ScopeDiffs<'a> {
    repo: &'a gix::Repository,
    workspace: &'a but_graph::Workspace,
    context_lines: u32,
    change_ids: Option<HashMap<String, gix::ObjectId>>,
    /// Tree changes per anchor scope; `None` when the scope could not be resolved.
    changes: HashMap<Option<String>, Option<Vec<but_core::TreeChange>>>,
    files: HashMap<(Option<String>, String), FileAnchor>,
}

impl<'a> ScopeDiffs<'a> {
    fn new(
        repo: &'a gix::Repository,
        workspace: &'a but_graph::Workspace,
        context_lines: u32,
    ) -> Self {
        ScopeDiffs {
            repo,
            workspace,
            context_lines,
            change_ids: None,
            changes: HashMap::new(),
            files: HashMap::new(),
        }
    }

    /// The anchor state of `path` within a scope (`None` = the uncommitted worktree diff,
    /// `Some` = the first-parent diff of the commit with that change-id).
    ///
    /// Returns `None` when the scope itself cannot be resolved right now — the commit is not
    /// reachable in the applied workspace (for example because its branch is unapplied) — which
    /// callers must treat as "don't know", never as "gone".
    fn file(&mut self, scope: Option<&str>, path: &str) -> anyhow::Result<Option<&FileAnchor>> {
        let scope_key = scope.map(str::to_owned);
        if !self.changes.contains_key(&scope_key) {
            let changes = self.changes_for_scope(scope)?;
            self.changes.insert(scope_key.clone(), changes);
        }
        let Some(changes) = &self.changes[&scope_key] else {
            return Ok(None);
        };
        let file_key = (scope_key, path.to_owned());
        if !self.files.contains_key(&file_key) {
            let anchor = file_diff_lines(self.repo, changes, path, self.context_lines)?;
            self.files.insert(file_key.clone(), anchor);
        }
        Ok(Some(&self.files[&file_key]))
    }

    fn changes_for_scope(
        &mut self,
        scope: Option<&str>,
    ) -> anyhow::Result<Option<Vec<but_core::TreeChange>>> {
        match scope {
            None => Ok(Some(but_core::diff::worktree_changes(self.repo)?.changes)),
            Some(change_id) => {
                let change_ids = match &mut self.change_ids {
                    Some(index) => index,
                    slot => slot.insert(change_id_index(self.repo, self.workspace)?),
                };
                let Some(&commit_id) = change_ids.get(change_id) else {
                    return Ok(None);
                };
                Ok(Some(
                    but_core::diff::CommitDetails::from_commit_id(
                        commit_id.attach(self.repo),
                        false,
                    )?
                    .diff_with_first_parent,
                ))
            }
        }
    }
}

/// Compute the anchorable diff lines of `path` from a scope's `changes`.
fn file_diff_lines(
    repo: &gix::Repository,
    changes: &[but_core::TreeChange],
    path: &str,
    context_lines: u32,
) -> anyhow::Result<FileAnchor> {
    // Compare lossily-decoded forms: every consumer (GUI transport, CLI arguments) carries
    // `String` paths, so a non-UTF-8 repository path can only ever match its lossy decoding.
    let Some(change) = changes
        .iter()
        .find(|change| change.path.to_str_lossy() == path)
    else {
        return Ok(FileAnchor::Gone);
    };
    match change.unified_patch(repo, context_lines)? {
        Some(but_core::UnifiedPatch::Patch { hunks, .. }) => {
            Ok(FileAnchor::Lines(FileDiffLines::from_hunks(&hunks)))
        }
        Some(_) => Ok(FileAnchor::Unanchorable),
        None => Ok(FileAnchor::Gone),
    }
}

/// Index every commit of every applied stack by its change-id, in one scan.
fn change_id_index(
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
) -> anyhow::Result<HashMap<String, gix::ObjectId>> {
    let mut index = HashMap::new();
    for stack in &workspace.stacks {
        for segment in &stack.segments {
            for commit in &segment.commits {
                let commit = but_core::Commit::from_id(commit.id.attach(repo))?;
                // The child-most commit wins when a change-id occurs twice.
                index
                    .entry(commit.change_id().to_string())
                    .or_insert_with(|| commit.id.detach());
            }
        }
    }
    Ok(index)
}

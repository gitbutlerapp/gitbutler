//! Ephemeral comments anchored to lines in diffs, shared between the GUI (which creates them)
//! and the CLI (where agents read, reply to, and archive them).
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
use std::collections::{HashMap, HashSet};

pub use store::{
    CommentStore, StoredComment, StoredCommentClient, StoredCommentReceipt, StoredMessage,
};

/// How long archived comments are kept around before being purged on the next list call.
const PURGE_ARCHIVED_AFTER_MS: i64 = 14 * 24 * 60 * 60 * 1000;

/// A polling agent is shown as active for this long after its most recent lease renewal.
/// Allow several missed two-second polling heartbeats and the brief handoff after delivery
/// without leaving disconnected workstreams visible for minutes.
pub const COMMENT_CLIENT_LEASE_MS: i64 = 15_000;

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

/// The kind of author identity asserted by a comment client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum CommentAuthorKind {
    /// A person using a comment client.
    Human,
    /// An automated agent using a comment client.
    Agent,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommentAuthorKind);

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
    /// The authored messages in this thread, in insertion order.
    pub messages: Vec<CommentMessage>,
    /// Agent workstreams that have been invited into this thread.
    pub agent_participant_ids: Vec<String>,
    /// Friendly identities and current listening state for invited agent workstreams.
    pub agent_participants: Vec<CommentParticipant>,
    /// A unified-diff-formatted excerpt of the current diff around the anchored line, so consumers
    /// can understand what the comment is about without recomputing the diff.
    /// Only present on comments returned from [`list_comments`].
    pub context: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(DiffComment);

/// An agent workstream participating in a comment thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommentParticipant {
    /// Stable agent workstream identity.
    pub id: String,
    /// Human-facing agent name.
    pub author: String,
    /// Optional human-facing workstream title.
    pub title: Option<String>,
    /// Whether the workstream has renewed its project polling lease recently.
    pub active: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommentParticipant);

/// One authored message in a diff comment thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommentMessage {
    /// The unique identifier of the message.
    pub id: String,
    /// The display name asserted by the client that created the message.
    pub author: String,
    /// Whether the client identifies the author as a human or an agent.
    pub author_kind: CommentAuthorKind,
    /// Stable workstream identity for an agent-authored message.
    pub author_client_id: Option<String>,
    /// Friendly workstream title captured when an agent authored this message.
    pub author_title: Option<String>,
    /// Agent workstreams invited into the thread by this message.
    pub mentioned_client_ids: Vec<String>,
    /// Agent workstreams that have explicitly acknowledged this message.
    pub acknowledgements: Vec<CommentAcknowledgement>,
    /// Number of agent participants expected to acknowledge this message.
    ///
    /// This snapshots the participants present when the message was authored, so inviting an
    /// agent later does not change an older message's read state.
    pub expected_acknowledgement_count: usize,
    /// The message text.
    pub payload: String,
    /// When the message was created, in milliseconds since the Unix epoch (UTC).
    pub created_at_ms: i64,
    /// When the message was last updated, in milliseconds since the Unix epoch (UTC).
    pub updated_at_ms: i64,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommentMessage);

/// Friendly identity for an agent that acknowledged a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommentAcknowledgement {
    /// Stable agent workstream identity.
    pub client_id: String,
    /// Human-facing agent name.
    pub author: String,
    /// Optional human-facing workstream title.
    pub title: Option<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommentAcknowledgement);

/// Client-supplied content and authorship for a new thread message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NewCommentMessage {
    /// An optional client-supplied ID. An ID will be generated if this is absent.
    pub id: Option<String>,
    /// The display name asserted by the client.
    pub author: String,
    /// Whether the client identifies the author as a human or an agent.
    pub author_kind: CommentAuthorKind,
    /// Stable workstream identity when the author is an agent.
    #[serde(default)]
    pub author_client_id: Option<String>,
    /// Agent workstreams to invite into the thread with this message.
    #[serde(default)]
    pub mentioned_client_ids: Vec<String>,
    /// The message text.
    pub payload: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(NewCommentMessage);

/// An agent workstream currently polling this project's comments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommentClient {
    /// Stable identity supplied by the agent harness.
    pub id: String,
    /// Human-facing agent name, such as `Codex`.
    pub author: String,
    /// Optional human-facing workstream title.
    pub title: Option<String>,
    /// Last lease renewal, in milliseconds since the Unix epoch (UTC).
    pub last_seen_at_ms: i64,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommentClient);

/// Everything needed to create a new comment. See [`DiffComment`] for the field semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NewComment {
    /// An optional client-supplied ID. An ID will be generated if this is absent.
    pub id: Option<String>,
    /// The worktree-relative path of the file to anchor the comment to.
    pub path: String,
    /// `None` to anchor to the uncommitted worktree diff, or the change-id of a workspace commit
    /// to anchor to that commit's first-parent diff.
    pub commit_change_id: Option<String>,
    /// The side of the diff the anchored line lives on.
    pub side: DiffSide,
    /// The 1-based line number of the line to anchor to, in `side`'s coordinates.
    pub line_number: u32,
    /// The first authored message in the thread.
    pub message: NewCommentMessage,
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
    validate_new_message(&comment.message, false)?;
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

    let author_title = comment
        .message
        .author_client_id
        .as_deref()
        .and_then(|client_id| {
            store
                .read_clients()
                .into_iter()
                .find(|client| client.id == client_id)
                .and_then(|client| client.title)
        });
    let mut stored = StoredComment {
        id: comment
            .id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        path: comment.path,
        commit_change_id: comment.commit_change_id,
        side: comment.side,
        line_number: comment.line_number,
        line_content: line.content.clone(),
        line_before,
        line_after,
        messages: vec![stored_message(comment.message, author_title, now_ms)],
        agent_participant_ids: Vec::new(),
        archived_at_ms: None,
    };
    add_message_participants(&mut stored);
    let result = stored.to_comment(
        stored.line_number,
        file.context_excerpt(line),
        &[],
        &store.read_clients(),
        now_ms,
    );
    store.update_file(|file| {
        acknowledge_authored_message(&mut file.receipts, &stored);
        file.comments.push(stored);
        Ok(())
    })?;
    Ok(result)
}

impl StoredComment {
    /// The consumer-facing view of this comment, re-anchored at `line_number` and carrying the
    /// diff excerpt around it.
    fn to_comment(
        &self,
        line_number: u32,
        context: String,
        receipts: &[StoredCommentReceipt],
        clients: &[StoredCommentClient],
        now_ms: i64,
    ) -> DiffComment {
        let receipt_positions = receipts
            .iter()
            .filter(|receipt| receipt.comment_id == self.id)
            .filter_map(|receipt| {
                self.messages
                    .iter()
                    .position(|message| message.id == receipt.message_id)
                    .map(|position| (receipt.client_id.as_str(), position))
            })
            .collect::<HashMap<_, _>>();
        let clients = clients
            .iter()
            .map(|client| (client.id.as_str(), client))
            .collect::<HashMap<_, _>>();
        let mut participants = HashSet::new();
        let active_after = now_ms.saturating_sub(COMMENT_CLIENT_LEASE_MS);
        DiffComment {
            id: self.id.clone(),
            path: self.path.clone(),
            commit_change_id: self.commit_change_id.clone(),
            side: self.side,
            line_number,
            line_content: self.line_content.clone(),
            messages: self
                .messages
                .iter()
                .enumerate()
                .map(|(position, message)| {
                    participants.extend(message.author_client_id.iter().cloned());
                    participants.extend(message.mentioned_client_ids.iter().cloned());
                    let expected_acknowledgement_count =
                        if message.author_kind == CommentAuthorKind::Human {
                            participants.len()
                        } else {
                            0
                        };
                    let mut acknowledgements = if message.author_kind == CommentAuthorKind::Human {
                        participants
                            .iter()
                            .filter(|client_id| {
                                receipt_positions
                                    .get(client_id.as_str())
                                    .is_some_and(|acknowledged| *acknowledged >= position)
                            })
                            .map(|client_id| {
                                let client = clients.get(client_id.as_str());
                                CommentAcknowledgement {
                                    client_id: client_id.clone(),
                                    author: client
                                        .map(|client| client.author.clone())
                                        .unwrap_or_else(|| client_id.clone()),
                                    title: client.and_then(|client| client.title.clone()),
                                }
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    acknowledgements.sort_by(|a, b| {
                        (&a.author, &a.title, &a.client_id).cmp(&(
                            &b.author,
                            &b.title,
                            &b.client_id,
                        ))
                    });
                    CommentMessage {
                        id: message.id.clone(),
                        author: message.author.clone(),
                        author_kind: message.author_kind,
                        author_client_id: message.author_client_id.clone(),
                        author_title: message.author_title.clone(),
                        mentioned_client_ids: message.mentioned_client_ids.clone(),
                        acknowledgements,
                        expected_acknowledgement_count,
                        payload: message.payload.clone(),
                        created_at_ms: message.created_at_ms,
                        updated_at_ms: message.updated_at_ms,
                    }
                })
                .collect(),
            agent_participant_ids: self.agent_participant_ids.clone(),
            agent_participants: self
                .agent_participant_ids
                .iter()
                .map(|id| {
                    let client = clients.get(id.as_str());
                    CommentParticipant {
                        id: id.clone(),
                        author: client
                            .map(|client| client.author.clone())
                            .unwrap_or_else(|| id.clone()),
                        title: client.and_then(|client| client.title.clone()),
                        active: client.is_some_and(|client| client.last_seen_at_ms >= active_after),
                    }
                })
                .collect(),
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
    let receipts = store.read_receipts();
    let clients = store.read_clients();
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
                let comment = row.to_comment(
                    line.line_number,
                    lines.context_excerpt(line),
                    &receipts,
                    &clients,
                    now_ms,
                );
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

/// Publish the payload of a blank draft message in an unarchived comment thread.
pub fn publish_draft_message(
    store: &CommentStore,
    comment_id: &str,
    message_id: &str,
    payload: String,
    mentioned_client_ids: Vec<String>,
    now_ms: i64,
) -> anyhow::Result<()> {
    if payload.trim().is_empty() {
        bail!("A published comment message cannot be blank");
    }
    store.update(|comments| {
        let Some(comment) = comments.iter_mut().find(|c| c.id == comment_id) else {
            bail!("No comment with id {comment_id}");
        };
        if comment.archived_at_ms.is_some() {
            bail!("Comment {comment_id} is archived and cannot be updated");
        }
        let Some(message) = comment.messages.iter_mut().find(|m| m.id == message_id) else {
            bail!("No message with id {message_id} in comment {comment_id}");
        };
        if !message.payload.trim().is_empty() {
            bail!("Message {message_id} is already published and cannot be edited");
        }
        message.payload = payload;
        message.mentioned_client_ids = mentioned_client_ids;
        message.updated_at_ms = now_ms.max(message.updated_at_ms.saturating_add(1));
        add_message_participants(comment);
        Ok(())
    })
}

/// Append an authored message to the unarchived comment with the given `id`.
pub fn reply_to_comment(
    store: &CommentStore,
    id: &str,
    message: NewCommentMessage,
    acknowledge_through: Option<&str>,
    now_ms: i64,
) -> anyhow::Result<CommentMessage> {
    validate_new_message(&message, true)?;
    store.update_file(|file| {
        let Some(comment) = file.comments.iter_mut().find(|c| c.id == id) else {
            bail!("No comment with id {id}");
        };
        if comment.archived_at_ms.is_some() {
            bail!("Comment {id} is archived and cannot be replied to");
        }
        let author_title = message.author_client_id.as_deref().and_then(|client_id| {
            file.clients
                .iter()
                .find(|client| client.id == client_id)
                .and_then(|client| client.title.clone())
        });
        let message = stored_message(message, author_title, now_ms);
        comment.messages.push(message.clone());
        add_message_participants(comment);
        if let (Some(client_id), Some(message_id)) =
            (message.author_client_id.as_deref(), acknowledge_through)
        {
            advance_receipt(&mut file.receipts, comment, message_id, client_id)?;
        }
        Ok(CommentMessage {
            id: message.id,
            author: message.author,
            author_kind: message.author_kind,
            author_client_id: message.author_client_id,
            author_title: message.author_title,
            mentioned_client_ids: message.mentioned_client_ids,
            acknowledgements: Vec::new(),
            expected_acknowledgement_count: 0,
            payload: message.payload,
            created_at_ms: message.created_at_ms,
            updated_at_ms: message.updated_at_ms,
        })
    })
}

fn validate_new_message(message: &NewCommentMessage, require_payload: bool) -> anyhow::Result<()> {
    if message.author.trim().is_empty() {
        bail!("A comment message author cannot be blank");
    }
    if require_payload && message.payload.trim().is_empty() {
        bail!("A comment reply cannot be blank");
    }
    Ok(())
}

fn stored_message(
    message: NewCommentMessage,
    author_title: Option<String>,
    now_ms: i64,
) -> StoredMessage {
    StoredMessage {
        id: message
            .id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        author: message.author,
        author_kind: message.author_kind,
        author_client_id: message.author_client_id,
        author_title,
        mentioned_client_ids: message.mentioned_client_ids,
        payload: message.payload,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
    }
}

fn add_message_participants(comment: &mut StoredComment) {
    let Some(message) = comment.messages.last() else {
        return;
    };
    let participants = message
        .author_client_id
        .iter()
        .chain(message.mentioned_client_ids.iter());
    for client_id in participants {
        if !comment.agent_participant_ids.contains(client_id) {
            comment.agent_participant_ids.push(client_id.clone());
        }
    }
}

fn acknowledge_authored_message(receipts: &mut Vec<StoredCommentReceipt>, comment: &StoredComment) {
    let Some(message) = comment.messages.last() else {
        return;
    };
    let Some(client_id) = message.author_client_id.as_ref() else {
        return;
    };
    set_receipt(receipts, client_id, &comment.id, &message.id);
}

fn set_receipt(
    receipts: &mut Vec<StoredCommentReceipt>,
    client_id: &str,
    comment_id: &str,
    message_id: &str,
) {
    if let Some(receipt) = receipts
        .iter_mut()
        .find(|receipt| receipt.client_id == client_id && receipt.comment_id == comment_id)
    {
        receipt.message_id = message_id.to_string();
    } else {
        receipts.push(StoredCommentReceipt {
            client_id: client_id.to_string(),
            comment_id: comment_id.to_string(),
            message_id: message_id.to_string(),
        });
    }
}

/// Record or renew an agent workstream's project-local polling lease.
pub fn register_comment_client(
    store: &CommentStore,
    id: &str,
    author: &str,
    title: Option<&str>,
    now_ms: i64,
) -> anyhow::Result<CommentClient> {
    if id.trim().is_empty() {
        bail!("A comment client id cannot be blank");
    }
    if author.trim().is_empty() {
        bail!("A comment client author cannot be blank");
    }
    let title = title
        .filter(|title| !title.trim().is_empty())
        .map(str::to_string);
    store.update_file(|file| {
        let client = StoredCommentClient {
            id: id.to_string(),
            author: author.to_string(),
            title,
            last_seen_at_ms: now_ms,
        };
        if let Some(existing) = file.clients.iter_mut().find(|client| client.id == id) {
            *existing = client.clone();
        } else {
            file.clients.push(client.clone());
        }
        Ok(client.into())
    })
}

/// Active agent workstreams, ordered by display name and title.
pub fn active_comment_clients(store: &CommentStore, now_ms: i64) -> Vec<CommentClient> {
    let oldest = now_ms.saturating_sub(COMMENT_CLIENT_LEASE_MS);
    let mut clients = store
        .read_clients()
        .into_iter()
        .filter(|client| client.last_seen_at_ms >= oldest)
        .map(CommentClient::from)
        .collect::<Vec<_>>();
    clients.sort_by(|a, b| (&a.author, &a.title, &a.id).cmp(&(&b.author, &b.title, &b.id)));
    clients
}

impl From<StoredCommentClient> for CommentClient {
    fn from(client: StoredCommentClient) -> Self {
        CommentClient {
            id: client.id,
            author: client.author,
            title: client.title,
            last_seen_at_ms: client.last_seen_at_ms,
        }
    }
}

/// Explicitly acknowledge every message through `message_id` for one thread and client.
pub fn acknowledge_comment(
    store: &CommentStore,
    comment_id: &str,
    message_id: &str,
    client_id: &str,
) -> anyhow::Result<()> {
    store.update_file(|file| {
        let Some(comment) = file
            .comments
            .iter()
            .find(|comment| comment.id == comment_id)
        else {
            bail!("No comment with id {comment_id}");
        };
        if !comment
            .agent_participant_ids
            .iter()
            .any(|id| id == client_id)
        {
            bail!("Client {client_id} is not a participant in comment {comment_id}");
        }
        advance_receipt(&mut file.receipts, comment, message_id, client_id)
    })
}

fn advance_receipt(
    receipts: &mut Vec<StoredCommentReceipt>,
    comment: &StoredComment,
    message_id: &str,
    client_id: &str,
) -> anyhow::Result<()> {
    let Some(next_index) = comment
        .messages
        .iter()
        .position(|message| message.id == message_id)
    else {
        bail!("No message with id {message_id} in comment {}", comment.id);
    };
    let current_index = receipts
        .iter()
        .find(|receipt| receipt.client_id == client_id && receipt.comment_id == comment.id)
        .and_then(|receipt| {
            comment
                .messages
                .iter()
                .position(|message| message.id == receipt.message_id)
        });
    if current_index.is_none_or(|current_index| next_index > current_index) {
        set_receipt(receipts, client_id, &comment.id, message_id);
    }
    Ok(())
}

/// Whether a participating client has an unacknowledged nonblank message authored by another
/// participant.
pub fn is_actionable_for(
    comment: &StoredComment,
    client_id: &str,
    receipts: &[StoredCommentReceipt],
) -> bool {
    if !comment
        .agent_participant_ids
        .iter()
        .any(|id| id == client_id)
    {
        return false;
    }
    let first_unread = receipts
        .iter()
        .find(|receipt| receipt.client_id == client_id && receipt.comment_id == comment.id)
        .and_then(|receipt| {
            comment
                .messages
                .iter()
                .position(|message| message.id == receipt.message_id)
        })
        .map_or(0, |index| index + 1);
    comment.messages[first_unread..].iter().any(|message| {
        !message.payload.trim().is_empty() && message.author_client_id.as_deref() != Some(client_id)
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

#[cfg(test)]
mod thread_tests {
    use super::*;

    fn stored_comment() -> StoredComment {
        StoredComment {
            id: "comment".to_string(),
            path: "src/a.rs".to_string(),
            commit_change_id: None,
            side: DiffSide::New,
            line_number: 1,
            line_content: "hello".to_string(),
            line_before: None,
            line_after: None,
            messages: vec![StoredMessage {
                id: "human-message".to_string(),
                author: "Sam".to_string(),
                author_kind: CommentAuthorKind::Human,
                author_client_id: None,
                author_title: None,
                mentioned_client_ids: vec!["codex-workstream".to_string()],
                payload: "please change this".to_string(),
                created_at_ms: 1000,
                updated_at_ms: 1000,
            }],
            agent_participant_ids: vec!["codex-workstream".to_string()],
            archived_at_ms: None,
        }
    }

    #[test]
    fn invitation_and_explicit_receipts_prevent_masked_human_messages() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = CommentStore::from_project_data_dir(dir.path());
        store.update(|comments| {
            comments.push(stored_comment());
            Ok(())
        })?;
        register_comment_client(
            &store,
            "codex-workstream",
            "Codex",
            Some("Implement foo"),
            1000,
        )?;

        assert!(
            is_actionable_for(&store.read()[0], "codex-workstream", &[]),
            "an invited client's first human message is actionable"
        );
        let first = reply_to_comment(
            &store,
            "comment",
            NewCommentMessage {
                id: Some("agent-message".to_string()),
                author: "Codex".to_string(),
                author_kind: CommentAuthorKind::Agent,
                author_client_id: Some("codex-workstream".to_string()),
                mentioned_client_ids: Vec::new(),
                payload: "done".to_string(),
            },
            Some("human-message"),
            1100,
        )?;
        assert_eq!(first.payload, "done");
        assert_eq!(first.author_title.as_deref(), Some("Implement foo"));
        let rendered = store.read()[0].to_comment(
            1,
            String::new(),
            &store.read_receipts(),
            &store.read_clients(),
            1100,
        );
        assert_eq!(
            rendered.agent_participants,
            [CommentParticipant {
                id: "codex-workstream".to_string(),
                author: "Codex".to_string(),
                title: Some("Implement foo".to_string()),
                active: true,
            }]
        );
        assert_eq!(rendered.messages[0].expected_acknowledgement_count, 1);
        assert_eq!(
            rendered.messages[0].acknowledgements,
            [CommentAcknowledgement {
                client_id: "codex-workstream".to_string(),
                author: "Codex".to_string(),
                title: Some("Implement foo".to_string()),
            }]
        );
        assert!(
            !is_actionable_for(&store.read()[0], "codex-workstream", &store.read_receipts()),
            "replying with an explicit cursor acknowledges the handled message"
        );

        reply_to_comment(
            &store,
            "comment",
            NewCommentMessage {
                id: Some("human-follow-up".to_string()),
                author: "Sam".to_string(),
                author_kind: CommentAuthorKind::Human,
                author_client_id: None,
                mentioned_client_ids: Vec::new(),
                payload: "one more thing".to_string(),
            },
            None,
            1200,
        )?;

        reply_to_comment(
            &store,
            "comment",
            NewCommentMessage {
                id: Some("agent-completion".to_string()),
                author: "Codex".to_string(),
                author_kind: CommentAuthorKind::Agent,
                author_client_id: Some("codex-workstream".to_string()),
                mentioned_client_ids: Vec::new(),
                payload: "all finished".to_string(),
            },
            None,
            1300,
        )?;

        let stored = store.read().remove(0);
        assert_eq!(stored.messages.len(), 4, "all messages remain in order");
        assert_eq!(stored.messages[0].author_kind, CommentAuthorKind::Human);
        assert_eq!(stored.messages[1].author_kind, CommentAuthorKind::Agent);
        assert_eq!(stored.messages[2].payload, "one more thing");
        assert!(
            is_actionable_for(&stored, "codex-workstream", &store.read_receipts()),
            "a later agent message cannot mask an unacknowledged human follow-up"
        );
        let rendered = stored.to_comment(
            1,
            String::new(),
            &store.read_receipts(),
            &store.read_clients(),
            1300,
        );
        assert_eq!(rendered.messages[2].expected_acknowledgement_count, 1);
        assert!(rendered.messages[2].acknowledgements.is_empty());

        acknowledge_comment(&store, "comment", "human-follow-up", "codex-workstream")?;
        assert!(
            !is_actionable_for(&store.read()[0], "codex-workstream", &store.read_receipts()),
            "explicit acknowledgement advances the client cursor"
        );
        let rendered = store.read()[0].to_comment(
            1,
            String::new(),
            &store.read_receipts(),
            &store.read_clients(),
            1400,
        );
        assert_eq!(rendered.messages[2].acknowledgements.len(), 1);
        Ok(())
    }

    #[test]
    fn polling_clients_have_leased_friendly_identity() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = CommentStore::from_project_data_dir(dir.path());
        register_comment_client(
            &store,
            "codex-workstream",
            "Codex",
            Some("Implement foo"),
            20_000,
        )?;

        assert_eq!(
            active_comment_clients(&store, 20_000),
            [CommentClient {
                id: "codex-workstream".to_string(),
                author: "Codex".to_string(),
                title: Some("Implement foo".to_string()),
                last_seen_at_ms: 20_000,
            }],
            "an active poller is available to mention"
        );
        assert!(
            active_comment_clients(&store, 20_000 + COMMENT_CLIENT_LEASE_MS + 1).is_empty(),
            "stale pollers disappear from mention autocomplete"
        );
        Ok(())
    }
}

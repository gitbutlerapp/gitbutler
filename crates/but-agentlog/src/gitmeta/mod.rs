use std::{borrow::Cow, collections::HashMap};

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use git_meta_lib::{ListEntry, MetaValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::environment::{EnvironmentSnapshot, ObservedTargets, SnapshotStatus};

const MAX_TOOL_RESULT_TEXT_BYTES: usize = 4 * 1024;
const TRUNCATION_MARKER: &str = "\n[TRUNCATED]\n";

// Index entries are rebuildable lookup data; readers verify hits against turn details.
const INDEX_NAMESPACE: &str = "gitbutler:agentlog-index:v1";

#[derive(Debug, Deserialize, Serialize)]
struct AcceptedRecord {
    record_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CaptureKind {
    Backfill,
    Incremental,
}

#[derive(Serialize)]
struct TurnSummary {
    turn_key: String,
    source_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_turn_key: Option<String>,
    capture_kind: CaptureKind,
    captured_at: String,
    environment_snapshot_status: SnapshotStatus,
}

#[derive(Deserialize)]
struct StoredTurnSummary {
    turn_key: String,
    previous_turn_key: Option<String>,
    capture_kind: String,
    captured_at: String,
    environment_snapshot_status: String,
}

#[derive(Serialize)]
struct TurnDetail<'a> {
    schema: &'static str,
    turn_key: &'a str,
    session_key: &'a str,
    source_key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_turn_key: Option<&'a str>,
    capture_kind: CaptureKind,
    captured_at: &'a str,
    records: &'a [AcceptedRecord],
    observed_targets: &'a ObservedTargets,
    environment: &'a EnvironmentSnapshot,
}

#[derive(Deserialize, Serialize)]
struct IndexHit {
    session_key: String,
    turn_key: String,
}

#[derive(Deserialize)]
struct StoredTurnDetail {
    records: Vec<AcceptedRecord>,
    #[serde(default)]
    observed_targets: StoredObservedTargets,
}

#[derive(Default, Deserialize)]
struct StoredObservedTargets {
    #[serde(default)]
    branches: Vec<StoredObservedTarget>,
    #[serde(default)]
    reviews: Vec<StoredObservedTarget>,
    #[serde(default)]
    changes: Vec<StoredObservedTarget>,
}

#[derive(Deserialize)]
struct StoredObservedTarget {
    key: String,
}

struct SessionListEntry {
    session_key: String,
    updated_at: String,
    sort_updated_at: DateTime<Utc>,
}

struct LatestStoredTurn {
    turn_key: String,
    timestamp: i64,
}

struct StoredTurnSummaryEntry {
    timestamp: i64,
    summary: StoredTurnSummary,
}

mod outline_support;
mod read;
mod read_support;
mod records_outline;
mod session_outline;
mod timeline_outline;
mod write;

pub(crate) use read::RelatedTarget;
pub(crate) use read::find_related_sessions_limited;
pub(crate) use records_outline::{SessionRecords, get_session_records};
pub(crate) use session_outline::RelatedSession;
pub(crate) use timeline_outline::{SessionTimeline, get_session_timeline_outline};
pub(crate) use write::{sync_metadata, write_transcript_batch};

fn index_key(kind: &str, target_key: &str) -> String {
    format!(
        "{INDEX_NAMESPACE}:{kind}:{}",
        hashed_index_target_key(target_key)
    )
}

fn hashed_index_target_key(target_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(target_key.as_bytes());
    format!("sha256-{}", hex::encode(hasher.finalize()))
}

fn latest_stored_turn(
    value: Option<&MetaValue>,
    turns_key: &str,
) -> Result<Option<LatestStoredTurn>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let MetaValue::List(entries) = value else {
        bail!("existing GitMeta key '{turns_key}' is not a list");
    };
    let Some(entry) = stored_turn_summary_entries(entries.to_vec(), turns_key)?.pop() else {
        return Ok(None);
    };
    Ok(Some(LatestStoredTurn {
        turn_key: entry.summary.turn_key,
        timestamp: entry.timestamp,
    }))
}

fn sorted_turn_entries(mut entries: Vec<ListEntry>) -> Vec<ListEntry> {
    entries.sort_by_key(|entry| entry.timestamp);
    entries
}

fn stored_turn_summary_entries(
    entries: Vec<ListEntry>,
    turns_key: &str,
) -> Result<Vec<StoredTurnSummaryEntry>> {
    let mut summaries = Vec::new();
    let mut positions_by_turn_key = HashMap::new();
    for entry in sorted_turn_entries(entries) {
        let summary: StoredTurnSummary = serde_json::from_str(&entry.value).with_context(|| {
            format!("existing GitMeta key '{turns_key}' has invalid turn summary")
        })?;
        if let Some(position) = positions_by_turn_key.get(&summary.turn_key).copied() {
            summaries[position] = StoredTurnSummaryEntry {
                timestamp: entry.timestamp,
                summary,
            };
        } else {
            positions_by_turn_key.insert(summary.turn_key.clone(), summaries.len());
            summaries.push(StoredTurnSummaryEntry {
                timestamp: entry.timestamp,
                summary,
            });
        }
    }
    Ok(summaries)
}

fn capture_turn_key(session_key: &str, source_key: &str, records: &[AcceptedRecord]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_key.as_bytes());
    hasher.update(b"\0");
    hasher.update(source_key.as_bytes());
    for record in records {
        hasher.update(b"\0");
        hasher.update(record.record_hash.as_bytes());
    }
    let digest = hasher.finalize();
    format!("sha256-{}", hex::encode(&digest[..16]))
}

fn cap_tool_result_text(text: &str) -> Cow<'_, str> {
    if text.len() <= MAX_TOOL_RESULT_TEXT_BYTES {
        return Cow::Borrowed(text);
    }

    let body_bytes = MAX_TOOL_RESULT_TEXT_BYTES - TRUNCATION_MARKER.len();
    let head_end = floor_char_boundary(text, body_bytes / 2);
    let tail_start = ceil_char_boundary(text, text.len() - (body_bytes - head_end));
    Cow::Owned(format!(
        "{}{}{}",
        &text[..head_end],
        TRUNCATION_MARKER,
        &text[tail_start..]
    ))
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests;

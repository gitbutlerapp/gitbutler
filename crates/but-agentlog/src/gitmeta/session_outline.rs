use std::collections::HashSet;

use anyhow::{Context as _, Result, bail};
use git_meta_lib::{MetaValue, SessionTargetHandle};
use serde::Serialize;
use serde_json::Value;

use super::{
    SessionListEntry,
    outline_support::{RecordPreview, compact_preview},
    read_support::{read_turn_detail, read_turn_summaries},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RelatedSession {
    pub(crate) session_key: String,
    pub(crate) updated_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) turn_count: usize,
    pub(crate) record_count: usize,
    pub(crate) latest_captured_at: Option<String>,
    pub(crate) related_turn_keys: Vec<String>,
    pub(crate) latest_user_preview: Option<RecordPreview>,
    pub(crate) latest_assistant_preview: Option<RecordPreview>,
}

pub(super) fn related_session_outline(
    handle: &SessionTargetHandle<'_>,
    entry: SessionListEntry,
    related_turn_keys: Vec<String>,
) -> Result<RelatedSession> {
    let session_prefix = format!("gitbutler:agent-session:{}", entry.session_key);
    let summaries = read_turn_summaries(handle, &format!("{session_prefix}:turns"))?;
    let related_turn_key_set = related_turn_keys
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut record_count = 0;
    let mut preview_hashes = HashSet::new();
    for summary in &summaries {
        let detail_key = format!("{session_prefix}:turn:{}", summary.turn_key);
        let detail = read_turn_detail(handle, &detail_key)?;
        record_count += detail.records.len();
        if related_turn_key_set.contains(summary.turn_key.as_str()) {
            preview_hashes.extend(detail.records.into_iter().map(|record| record.record_hash));
        }
    }

    let previews = session_previews(handle, &session_prefix, &preview_hashes)?;
    let latest = summaries.last();
    Ok(RelatedSession {
        session_key: entry.session_key,
        updated_at: entry.updated_at,
        started_at: summaries.first().map(|summary| summary.captured_at.clone()),
        turn_count: summaries.len(),
        record_count,
        latest_captured_at: latest.map(|summary| summary.captured_at.clone()),
        related_turn_keys,
        latest_user_preview: previews.latest_user,
        latest_assistant_preview: previews.latest_assistant,
    })
}

#[derive(Default)]
struct SessionPreviews {
    latest_user: Option<RecordPreview>,
    latest_assistant: Option<RecordPreview>,
}

fn session_previews(
    handle: &SessionTargetHandle<'_>,
    session_prefix: &str,
    preview_hashes: &HashSet<String>,
) -> Result<SessionPreviews> {
    if preview_hashes.is_empty() {
        return Ok(SessionPreviews::default());
    }

    let transcript_key = format!("{session_prefix}:transcript");
    let Some(value) = handle
        .get_value(&transcript_key)
        .with_context(|| format!("failed to read GitMeta key '{transcript_key}'"))?
    else {
        return Ok(SessionPreviews::default());
    };
    let MetaValue::List(entries) = value else {
        bail!("existing GitMeta key '{transcript_key}' is not a list");
    };

    let mut previews = SessionPreviews::default();
    let mut entries = entries;
    entries.sort_by_key(|entry| entry.timestamp);
    for entry in entries.into_iter().rev() {
        if previews.latest_user.is_some() && previews.latest_assistant.is_some() {
            break;
        }

        let Ok(record) = serde_json::from_str::<Value>(&entry.value) else {
            continue;
        };
        let Some(record_hash) = record["record_hash"].as_str() else {
            continue;
        };
        if !preview_hashes.contains(record_hash) {
            continue;
        }
        if record["kind"].as_str() != Some("message") {
            continue;
        }
        let Some(text) = record["text"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
        else {
            continue;
        };
        let preview = RecordPreview {
            timestamp: record["timestamp"].as_str().map(ToOwned::to_owned),
            source_event_kind: record["source_event_kind"].as_str().map(ToOwned::to_owned),
            text: compact_preview(text),
        };
        match record["role"].as_str() {
            Some("user") if previews.latest_user.is_none() => previews.latest_user = Some(preview),
            Some("assistant") if previews.latest_assistant.is_none() => {
                previews.latest_assistant = Some(preview);
            }
            _ => {}
        }
    }

    Ok(previews)
}

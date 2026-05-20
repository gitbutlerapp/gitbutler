use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use git_meta_lib::{ListEntry, MetaValue, Session, SessionTargetHandle, Target};
use serde::Serialize;
use serde_json::Value;

use super::{
    AcceptedRecord, IndexHit, SessionListEntry, StoredTurnDetail, StoredTurnSummary, index_key,
    stored_turn_summary_entries,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionSummary {
    pub session_key: String,
    pub updated_at: String,
    pub source_keys: Vec<String>,
    pub turn_count: usize,
    pub latest_turn_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimelineTurn {
    pub turn_key: String,
    pub source_key: String,
    pub previous_turn_key: Option<String>,
    pub capture_kind: String,
    pub captured_at: String,
    pub environment_snapshot_status: String,
    pub records: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelatedTurnWindow {
    pub context_before: usize,
    pub context_after: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelatedTarget<'a> {
    Branch(&'a str),
    Review(&'a str),
    Change(&'a str),
}

impl<'a> RelatedTarget<'a> {
    fn index_kind(self) -> &'static str {
        match self {
            RelatedTarget::Branch(_) => "branch",
            RelatedTarget::Review(_) => "review",
            RelatedTarget::Change(_) => "change",
        }
    }

    fn key(self) -> &'a str {
        match self {
            RelatedTarget::Branch(key)
            | RelatedTarget::Review(key)
            | RelatedTarget::Change(key) => key,
        }
    }

    fn observed_targets_field(self) -> &'static str {
        match self {
            RelatedTarget::Branch(_) => "branches",
            RelatedTarget::Review(_) => "reviews",
            RelatedTarget::Change(_) => "changes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatedSession {
    pub session_key: String,
    pub turn_keys: Vec<String>,
}

fn session_list_entry(
    handle: &SessionTargetHandle<'_>,
    session_key: String,
) -> Result<SessionListEntry> {
    let session_prefix = format!("gitbutler:agent-session:{session_key}");
    let updated_at_key = format!("{session_prefix}:updated-at");
    let updated_at = match handle
        .get_value(&updated_at_key)
        .with_context(|| format!("failed to read GitMeta key '{updated_at_key}'"))?
    {
        None => bail!("existing session '{session_key}' is missing updated-at"),
        Some(MetaValue::String(updated_at)) => updated_at,
        Some(_) => bail!("existing GitMeta key '{updated_at_key}' is not a string"),
    };
    let sort_updated_at = DateTime::parse_from_rfc3339(&updated_at)
        .with_context(|| format!("existing GitMeta key '{updated_at_key}' has invalid timestamp"))?
        .with_timezone(&Utc);
    Ok(SessionListEntry {
        session_key,
        updated_at,
        sort_updated_at,
    })
}

fn session_summary(
    handle: &SessionTargetHandle<'_>,
    entry: SessionListEntry,
) -> Result<SessionSummary> {
    let session_prefix = format!("gitbutler:agent-session:{}", entry.session_key);
    let sources_key = format!("{session_prefix}:sources");
    let turns_key = format!("{session_prefix}:turns");
    let mut source_keys: Vec<String> = match handle
        .get_value(&sources_key)
        .with_context(|| format!("failed to read GitMeta key '{sources_key}'"))?
    {
        None => bail!(
            "existing session '{}' is missing sources",
            entry.session_key
        ),
        Some(MetaValue::Set(source_keys)) => source_keys.into_iter().collect(),
        Some(_) => bail!("existing GitMeta key '{sources_key}' is not a set"),
    };
    source_keys.sort();
    let Some(turns_value) = handle
        .get_value(&turns_key)
        .with_context(|| format!("failed to read GitMeta key '{turns_key}'"))?
    else {
        bail!("existing session '{}' is missing turns", entry.session_key);
    };
    let MetaValue::List(turn_entries) = turns_value else {
        bail!("existing GitMeta key '{turns_key}' is not a list");
    };
    let summaries = turn_summaries_from_entries(turn_entries, &turns_key)?;
    let latest_turn_key = summaries.last().map(|summary| summary.turn_key.clone());
    Ok(SessionSummary {
        session_key: entry.session_key,
        updated_at: entry.updated_at,
        source_keys,
        turn_count: summaries.len(),
        latest_turn_key,
    })
}

pub fn find_related_sessions(
    repo_path: &Path,
    target: RelatedTarget<'_>,
) -> Result<Vec<RelatedSession>> {
    with_project_target(repo_path, |handle| {
        let sessions = verified_related_turns_by_session(handle, target)?
            .into_iter()
            .map(|(session_key, turn_keys)| RelatedSession {
                session_key,
                turn_keys,
            })
            .collect::<Vec<_>>();
        Ok(sessions)
    })
}

fn with_project_target<T>(
    repo_path: &Path,
    read: impl FnOnce(&SessionTargetHandle<'_>) -> Result<T>,
) -> Result<T> {
    let gitmeta = Session::open(repo_path).context("failed to open GitMeta session")?;
    let project = Target::project();
    let handle = gitmeta.target(&project);
    read(&handle)
}

fn verified_related_turns_by_session(
    handle: &SessionTargetHandle<'_>,
    target: RelatedTarget<'_>,
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut indexed_turns = BTreeMap::<String, HashSet<String>>::new();
    for hit in target_index_hits(handle, target)? {
        let Ok(hit) = serde_json::from_str::<IndexHit>(&hit) else {
            continue;
        };
        if turn_detail_observes_target(handle, &hit, target)? {
            indexed_turns
                .entry(hit.session_key)
                .or_default()
                .insert(hit.turn_key);
        }
    }

    let mut by_session = BTreeMap::new();
    for (session_key, turn_keys) in indexed_turns {
        let ordered_turn_keys = ordered_existing_turn_keys(handle, &session_key, &turn_keys)?;
        if !ordered_turn_keys.is_empty() {
            by_session.insert(session_key, ordered_turn_keys);
        }
    }
    Ok(by_session)
}

fn ordered_existing_turn_keys(
    handle: &SessionTargetHandle<'_>,
    session_key: &str,
    turn_keys: &HashSet<String>,
) -> Result<Vec<String>> {
    let turns_key = format!("gitbutler:agent-session:{session_key}:turns");
    Ok(read_turn_summaries(handle, &turns_key)?
        .into_iter()
        .filter_map(|summary| {
            if turn_keys.contains(&summary.turn_key) {
                Some(summary.turn_key)
            } else {
                None
            }
        })
        .collect())
}

fn target_index_hits(
    handle: &SessionTargetHandle<'_>,
    target: RelatedTarget<'_>,
) -> Result<BTreeSet<String>> {
    let index_key = index_key(target.index_kind(), target.key());
    let Some(value) = handle
        .get_value(&index_key)
        .with_context(|| format!("failed to read GitMeta key '{index_key}'"))?
    else {
        return Ok(BTreeSet::new());
    };
    let MetaValue::Set(index_hits) = value else {
        bail!("existing GitMeta key '{index_key}' is not a set");
    };
    Ok(index_hits)
}

fn read_turn_summaries(
    handle: &SessionTargetHandle<'_>,
    turns_key: &str,
    max_turns: Option<usize>,
) -> Result<Vec<StoredTurnSummary>> {
    let Some(turns_value) = handle
        .get_value(turns_key)
        .with_context(|| format!("failed to read GitMeta key '{turns_key}'"))?
    else {
        return Ok(Vec::new());
    };
    let MetaValue::List(turn_entries) = turns_value else {
        bail!("existing GitMeta key '{turns_key}' is not a list");
    };

    let mut summaries = turn_summaries_from_entries(turn_entries, turns_key)?;
    let summaries = if let Some(start) =
        max_turns.and_then(|max_turns| summaries.len().checked_sub(max_turns))
    {
        summaries.split_off(start)
    } else {
        summaries
    };

    Ok(summaries)
}

fn turn_summaries_from_entries(
    entries: Vec<ListEntry>,
    turns_key: &str,
) -> Result<Vec<StoredTurnSummary>> {
    Ok(stored_turn_summary_entries(entries, turns_key)?
        .into_iter()
        .map(|entry| entry.summary)
        .collect())
}

fn hydrate_timeline_turns(
    handle: &SessionTargetHandle<'_>,
    session_prefix: &str,
    transcript_key: &str,
    summaries: Vec<StoredTurnSummary>,
) -> Result<Vec<TimelineTurn>> {
    if summaries.is_empty() {
        return Ok(Vec::new());
    }

    let mut turn_parts = Vec::with_capacity(summaries.len());
    let mut needed_hashes = HashSet::new();
    for summary in summaries {
        let detail_key = format!("{session_prefix}:turn:{}", summary.turn_key);
        let accepted_records = turn_record_memberships(handle, &detail_key)?;
        needed_hashes.extend(
            accepted_records
                .iter()
                .map(|record| record.record_hash.clone()),
        );
        turn_parts.push((summary, accepted_records));
    }

    let Some(transcript_value) = handle
        .get_value(transcript_key)
        .with_context(|| format!("failed to read GitMeta key '{transcript_key}'"))?
    else {
        bail!("existing GitMeta key '{transcript_key}' is missing");
    };
    let MetaValue::List(transcript_entries) = transcript_value else {
        bail!("existing GitMeta key '{transcript_key}' is not a list");
    };
    let transcript_records = transcript_records_by_hash(transcript_entries, &needed_hashes);

    let mut turns = Vec::with_capacity(turn_parts.len());
    for (summary, accepted_records) in turn_parts {
        let records = timeline_records(accepted_records, &transcript_records)?;
        turns.push(TimelineTurn {
            turn_key: summary.turn_key,
            source_key: summary.source_key,
            previous_turn_key: summary.previous_turn_key,
            capture_kind: summary.capture_kind,
            captured_at: summary.captured_at,
            environment_snapshot_status: summary.environment_snapshot_status,
            records,
        });
    }

    Ok(turns)
}

fn transcript_records_by_hash(
    entries: Vec<ListEntry>,
    needed_hashes: &HashSet<String>,
) -> HashMap<String, Value> {
    let mut records = HashMap::with_capacity(needed_hashes.len());
    for entry in entries {
        let Ok(record) = serde_json::from_str::<Value>(&entry.value) else {
            continue;
        };
        let Some(record_hash) = record["record_hash"].as_str() else {
            continue;
        };
        if !needed_hashes.contains(record_hash) {
            continue;
        }
        records.insert(record_hash.to_owned(), record);
        if records.len() == needed_hashes.len() {
            break;
        }
    }
    records
}

fn selected_related_turn_summaries(
    summaries: Vec<StoredTurnSummary>,
    highlighted_turn_keys: &HashSet<&str>,
    turn_window: RelatedTurnWindow,
) -> Result<Vec<StoredTurnSummary>> {
    let mut selected_turns = vec![false; summaries.len()];
    {
        let mut found_turn_keys = HashSet::new();
        for (index, summary) in summaries.iter().enumerate() {
            if !highlighted_turn_keys.contains(summary.turn_key.as_str()) {
                continue;
            }

            found_turn_keys.insert(summary.turn_key.as_str());
            let start = index.saturating_sub(turn_window.context_before);
            let end = index
                .saturating_add(turn_window.context_after)
                .min(summaries.len().saturating_sub(1));
            for selected in &mut selected_turns[start..=end] {
                *selected = true;
            }
        }

        if let Some(missing_turn_key) = highlighted_turn_keys
            .iter()
            .copied()
            .find(|turn_key| !found_turn_keys.contains(turn_key))
        {
            bail!("related session references missing turn '{missing_turn_key}'");
        }
    }

    Ok(summaries
        .into_iter()
        .enumerate()
        .filter_map(|(index, summary)| {
            if selected_turns[index] {
                Some(summary)
            } else {
                None
            }
        })
        .collect())
}

fn turn_record_memberships(
    handle: &SessionTargetHandle<'_>,
    detail_key: &str,
) -> Result<Vec<AcceptedRecord>> {
    let Some(value) = handle
        .get_value(detail_key)
        .with_context(|| format!("failed to read GitMeta key '{detail_key}'"))?
    else {
        bail!("existing GitMeta key '{detail_key}' is missing");
    };
    let MetaValue::String(detail) = value else {
        bail!("existing GitMeta key '{detail_key}' is not a string");
    };
    let detail: StoredTurnDetail = serde_json::from_str(&detail)
        .with_context(|| format!("existing GitMeta key '{detail_key}' has invalid JSON"))?;
    Ok(detail.records)
}

fn timeline_records(
    accepted_records: Vec<AcceptedRecord>,
    transcript_records: &HashMap<String, Value>,
) -> Result<Vec<Value>> {
    accepted_records
        .into_iter()
        .map(|record| {
            transcript_records
                .get(&record.record_hash)
                .cloned()
                .with_context(|| {
                    format!(
                        "turn detail references missing transcript record '{}'",
                        record.record_hash
                    )
                })
        })
        .collect()
}

fn turn_detail_observes_target(
    handle: &SessionTargetHandle<'_>,
    hit: &IndexHit,
    target: RelatedTarget<'_>,
) -> Result<bool> {
    let detail_key = format!(
        "gitbutler:agent-session:{}:turn:{}",
        hit.session_key, hit.turn_key
    );
    let Some(value) = handle
        .get_value(&detail_key)
        .with_context(|| format!("failed to read GitMeta key '{detail_key}'"))?
    else {
        return Ok(false);
    };
    let MetaValue::String(detail) = value else {
        bail!("existing GitMeta key '{detail_key}' is not a string");
    };
    let detail: Value = serde_json::from_str(&detail)
        .with_context(|| format!("existing GitMeta key '{detail_key}' has invalid JSON"))?;
    Ok(detail["observed_targets"][target.observed_targets_field()]
        .as_array()
        .is_some_and(|targets| {
            targets
                .iter()
                .any(|observed| observed["key"].as_str() == Some(target.key()))
        }))
}

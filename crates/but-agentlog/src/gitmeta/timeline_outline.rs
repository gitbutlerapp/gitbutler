use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result};
use git_meta_lib::SessionTargetHandle;
use serde::{Deserialize, Serialize};

use super::{
    AcceptedRecord, StoredObservedTargets, StoredTurnSummary,
    outline_support::{ObservedTargetKeys, RecordPreview, compact_preview, push_unique},
    read_support::{
        read_transcript_entries, read_turn_detail, read_turn_summaries, transcript_records_by_hash,
        with_project_target,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SessionTimeline {
    pub(crate) session_key: String,
    pub(crate) coverage: TimelineCoverage,
    pub(crate) turns: Vec<TimelineTurnOutline>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TimelineCoverage {
    pub(crate) showing_turns: usize,
    pub(crate) total_turns: usize,
    pub(crate) has_more_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct TimelineTurnOutline {
    pub(crate) turn_index: usize,
    pub(crate) turn_key: String,
    pub(crate) previous_turn_key: Option<String>,
    pub(crate) capture_kind: String,
    pub(crate) captured_at: String,
    pub(crate) record_count: usize,
    pub(crate) source_record_index_range: SourceRecordIndexRange,
    pub(crate) environment_snapshot_status: String,
    pub(crate) observed_targets: ObservedTargetKeys,
    pub(crate) first_record_timestamp: Option<String>,
    pub(crate) last_record_timestamp: Option<String>,
    pub(crate) latest_user_preview: Option<RecordPreview>,
    pub(crate) latest_assistant_preview: Option<RecordPreview>,
    pub(crate) tool_counts: ToolCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SourceRecordIndexRange {
    pub(crate) start: Option<usize>,
    pub(crate) end: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct ToolCounts {
    pub(crate) tool_call_count: usize,
    pub(crate) tool_result_count: usize,
    pub(crate) tool_names: Vec<String>,
}

pub(crate) fn get_session_timeline_outline(
    repo_path: &std::path::Path,
    session_key: &str,
    max_turns: Option<usize>,
) -> Result<SessionTimeline> {
    with_project_target(repo_path, |handle| {
        let session_prefix = format!("gitbutler:agent-session:{session_key}");
        let turns_key = format!("{session_prefix}:turns");
        let summaries = read_turn_summaries(handle, &turns_key)?;
        let total_turns = summaries.len();
        let start = max_turns
            .and_then(|max_turns| total_turns.checked_sub(max_turns))
            .unwrap_or(0);
        let coverage = TimelineCoverage {
            showing_turns: total_turns - start,
            total_turns,
            has_more_before: start > 0,
        };
        let selected = summaries.into_iter().enumerate().skip(start);
        let turns = timeline_turns(handle, &session_prefix, selected)?;

        Ok(SessionTimeline {
            session_key: session_key.to_owned(),
            coverage,
            turns,
        })
    })
}

fn timeline_turns(
    handle: &SessionTargetHandle<'_>,
    session_prefix: &str,
    summaries: impl IntoIterator<Item = (usize, StoredTurnSummary)>,
) -> Result<Vec<TimelineTurnOutline>> {
    let mut parts = Vec::new();
    let mut needed_hashes = HashSet::new();
    for (turn_index, summary) in summaries {
        let detail_key = format!("{session_prefix}:turn:{}", summary.turn_key);
        let detail = read_turn_detail(handle, &detail_key)?;
        needed_hashes.extend(
            detail
                .records
                .iter()
                .map(|record| record.record_hash.clone()),
        );
        parts.push((turn_index, summary, detail.records, detail.observed_targets));
    }

    let transcript_key = format!("{session_prefix}:transcript");
    let records = compact_records_by_hash(handle, &transcript_key, &needed_hashes)?;
    parts
        .into_iter()
        .map(
            |(turn_index, summary, accepted_records, observed_targets)| {
                let turn_records = accepted_records
                    .iter()
                    .map(|record| {
                        records.get(&record.record_hash).with_context(|| {
                            format!(
                                "turn '{}' references a missing transcript record",
                                summary.turn_key
                            )
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(turn_outline(
                    turn_index,
                    summary,
                    &accepted_records,
                    &turn_records,
                    observed_targets,
                ))
            },
        )
        .collect()
}

fn turn_outline(
    turn_index: usize,
    summary: StoredTurnSummary,
    accepted_records: &[AcceptedRecord],
    records: &[&CompactRecord],
    observed_targets: StoredObservedTargets,
) -> TimelineTurnOutline {
    TimelineTurnOutline {
        turn_index,
        turn_key: summary.turn_key,
        previous_turn_key: summary.previous_turn_key,
        capture_kind: summary.capture_kind,
        captured_at: summary.captured_at,
        record_count: accepted_records.len(),
        source_record_index_range: source_record_index_range(records),
        environment_snapshot_status: summary.environment_snapshot_status,
        observed_targets: observed_target_keys(observed_targets),
        first_record_timestamp: records.first().and_then(|record| record.timestamp.clone()),
        last_record_timestamp: records.last().and_then(|record| record.timestamp.clone()),
        latest_user_preview: latest_role_preview(records, "user"),
        latest_assistant_preview: latest_role_preview(records, "assistant"),
        tool_counts: tool_counts(records),
    }
}

struct CompactRecord {
    record_index: Option<usize>,
    timestamp: Option<String>,
    kind: Option<String>,
    role: Option<String>,
    preview_text: Option<String>,
    source_event_kind: Option<String>,
    tool_name: Option<String>,
}

#[derive(Deserialize)]
struct StoredTranscriptRecord {
    record_hash: String,
    record_index: Option<usize>,
    timestamp: Option<String>,
    kind: Option<String>,
    role: Option<String>,
    text: Option<String>,
    source_event_kind: Option<String>,
    tool_name: Option<String>,
}

fn compact_records_by_hash(
    handle: &SessionTargetHandle<'_>,
    transcript_key: &str,
    needed_hashes: &HashSet<String>,
) -> Result<HashMap<String, CompactRecord>> {
    let entries = read_transcript_entries(handle, transcript_key)?;
    Ok(transcript_records_by_hash(
        entries,
        needed_hashes,
        parse_compact_record,
    ))
}

fn parse_compact_record(raw: &str) -> Option<(String, CompactRecord)> {
    let record = serde_json::from_str::<StoredTranscriptRecord>(raw).ok()?;
    let record_hash = record.record_hash;
    let preview_text = match (record.kind.as_deref(), record.role.as_deref()) {
        (Some("message"), Some("user" | "assistant")) => record
            .text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(compact_preview),
        _ => None,
    };
    Some((
        record_hash,
        CompactRecord {
            record_index: record.record_index,
            timestamp: record.timestamp,
            kind: record.kind,
            role: record.role,
            preview_text,
            source_event_kind: record.source_event_kind,
            tool_name: record.tool_name,
        },
    ))
}

fn source_record_index_range(records: &[&CompactRecord]) -> SourceRecordIndexRange {
    let mut indexes = records.iter().filter_map(|record| record.record_index);
    let Some(first) = indexes.next() else {
        return SourceRecordIndexRange {
            start: None,
            end: None,
        };
    };
    let (start, end) = indexes.fold((first, first), |(start, end), index| {
        (start.min(index), end.max(index))
    });
    SourceRecordIndexRange {
        start: Some(start),
        end: Some(end),
    }
}

fn observed_target_keys(targets: StoredObservedTargets) -> ObservedTargetKeys {
    ObservedTargetKeys {
        branches: targets
            .branches
            .into_iter()
            .map(|target| target.key)
            .collect(),
        reviews: targets
            .reviews
            .into_iter()
            .map(|target| target.key)
            .collect(),
        changes: targets
            .changes
            .into_iter()
            .map(|target| target.key)
            .collect(),
    }
}

fn latest_role_preview(records: &[&CompactRecord], role: &str) -> Option<RecordPreview> {
    records
        .iter()
        .rev()
        .find(|record| record.role.as_deref() == Some(role) && record.preview_text.is_some())
        .and_then(|record| {
            Some(RecordPreview {
                timestamp: record.timestamp.clone(),
                source_event_kind: record.source_event_kind.clone(),
                text: record.preview_text.clone()?,
            })
        })
}

fn tool_counts(records: &[&CompactRecord]) -> ToolCounts {
    let mut counts = ToolCounts::default();
    for record in records {
        match record.kind.as_deref() {
            Some("tool_call") => counts.tool_call_count += 1,
            Some("tool_result") => counts.tool_result_count += 1,
            _ => {}
        }
        push_unique(&mut counts.tool_names, record.tool_name.as_deref());
    }
    counts
}

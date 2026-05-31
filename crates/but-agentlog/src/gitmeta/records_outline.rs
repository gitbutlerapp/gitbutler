use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result};
use git_meta_lib::SessionTargetHandle;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AcceptedRecord, PublicationStatus,
    read_support::{
        read_transcript_entries, read_turn_detail, transcript_records_by_hash, with_project_target,
    },
    session_storage_prefix,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct SessionRecords {
    pub(crate) session_key: String,
    pub(crate) status: PublicationStatus,
    pub(crate) turn_key: String,
    pub(crate) coverage: RecordCoverage,
    pub(crate) records: Vec<RecordDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RecordCoverage {
    pub(crate) showing_records: usize,
    pub(crate) total_records: usize,
    pub(crate) has_more_before: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct RecordDetail {
    #[serde(skip)]
    pub(crate) record_hash: String,
    pub(crate) turn_record_index: usize,
    pub(crate) source_record_index: Option<usize>,
    pub(crate) timestamp: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) source_event_kind: Option<String>,
    pub(crate) role: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) prompt_source: Option<String>,
    pub(crate) tool_name: Option<String>,
    pub(crate) tool_kind: Option<String>,
    pub(crate) tool_input: Option<Value>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) outcome: Option<String>,
    pub(crate) source_record: Value,
}

pub(crate) fn get_session_records(
    repo_path: &std::path::Path,
    session_key: &str,
    status: PublicationStatus,
    turn_key: &str,
    limit: usize,
) -> Result<SessionRecords> {
    with_project_target(repo_path, |handle| {
        let session_prefix = session_storage_prefix(status, session_key);
        let detail_key = format!("{session_prefix}:turn:{turn_key}");
        let detail = read_turn_detail(handle, &detail_key)?;
        let total_records = detail.records.len();
        let start = total_records.saturating_sub(limit);
        let selected = &detail.records[start..];
        let coverage = RecordCoverage {
            showing_records: selected.len(),
            total_records,
            has_more_before: start > 0,
        };
        let records = if selected.is_empty() {
            Vec::new()
        } else {
            let transcript_key = format!("{session_prefix}:transcript");
            let mut record_map = records_by_hash(handle, &transcript_key, selected)?;
            selected
                .iter()
                .enumerate()
                .map(|(selected_index, record)| {
                    let turn_record_index = start + selected_index;
                    let stored = record_map.remove(&record.record_hash).with_context(|| {
                        format!("turn '{turn_key}' references a missing transcript record")
                    })?;
                    Ok(record_detail(turn_record_index, stored))
                })
                .collect::<Result<Vec<_>>>()?
        };

        Ok(SessionRecords {
            session_key: session_key.to_owned(),
            status,
            turn_key: turn_key.to_owned(),
            coverage,
            records,
        })
    })
}

#[derive(Deserialize)]
struct StoredRecord {
    record_hash: String,
    record_index: Option<usize>,
    timestamp: Option<String>,
    kind: Option<String>,
    source_event_kind: Option<String>,
    role: Option<String>,
    text: Option<String>,
    #[serde(default)]
    prompt_source: Option<String>,
    tool_name: Option<String>,
    #[serde(default)]
    tool_kind: Option<String>,
    tool_input: Option<Value>,
    #[serde(default)]
    exit_code: Option<i32>,
    #[serde(default)]
    outcome: Option<String>,
    source_record: Value,
}

fn records_by_hash(
    handle: &SessionTargetHandle<'_>,
    transcript_key: &str,
    accepted_records: &[AcceptedRecord],
) -> Result<HashMap<String, StoredRecord>> {
    let needed_hashes = accepted_records
        .iter()
        .map(|record| record.record_hash.clone())
        .collect::<HashSet<_>>();
    let entries = read_transcript_entries(handle, transcript_key)?;
    Ok(transcript_records_by_hash(
        entries,
        &needed_hashes,
        parse_record,
    ))
}

fn parse_record(raw: &str) -> Option<(String, StoredRecord)> {
    let record = serde_json::from_str::<StoredRecord>(raw).ok()?;
    Some((record.record_hash.clone(), record))
}

fn record_detail(turn_record_index: usize, record: StoredRecord) -> RecordDetail {
    RecordDetail {
        record_hash: record.record_hash,
        turn_record_index,
        source_record_index: record.record_index,
        timestamp: record.timestamp,
        kind: record.kind,
        source_event_kind: record.source_event_kind,
        role: record.role,
        text: record.text,
        prompt_source: record.prompt_source,
        tool_name: record.tool_name,
        tool_kind: record.tool_kind,
        tool_input: record.tool_input,
        exit_code: record.exit_code,
        outcome: record.outcome,
        source_record: record.source_record,
    }
}

use std::{collections::HashMap, path::Path};

use anyhow::{Context as _, Result, bail};
use git_meta_lib::{ListEntry, MetaValue, Session, SessionTargetHandle, Target};

use super::{StoredTurnDetail, StoredTurnSummary, stored_turn_summary_entries};

pub(super) fn with_project_target<T>(
    repo_path: &Path,
    read: impl FnOnce(&SessionTargetHandle<'_>) -> Result<T>,
) -> Result<T> {
    let gitmeta = Session::open(repo_path).context("failed to open GitMeta session")?;
    let project = Target::project();
    let handle = gitmeta.target(&project);
    read(&handle)
}

pub(super) fn read_turn_summaries(
    handle: &SessionTargetHandle<'_>,
    turns_key: &str,
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

    let summaries = stored_turn_summary_entries(turn_entries, turns_key)?
        .into_iter()
        .map(|entry| entry.summary)
        .collect::<Vec<_>>();

    Ok(summaries)
}

pub(super) fn read_turn_detail(
    handle: &SessionTargetHandle<'_>,
    detail_key: &str,
) -> Result<StoredTurnDetail> {
    let Some(detail) = read_optional_turn_detail(handle, detail_key)? else {
        bail!("existing GitMeta key '{detail_key}' is missing");
    };
    Ok(detail)
}

pub(super) fn read_optional_turn_detail(
    handle: &SessionTargetHandle<'_>,
    detail_key: &str,
) -> Result<Option<StoredTurnDetail>> {
    let Some(value) = handle
        .get_value(detail_key)
        .with_context(|| format!("failed to read GitMeta key '{detail_key}'"))?
    else {
        return Ok(None);
    };
    let MetaValue::String(detail) = value else {
        bail!("existing GitMeta key '{detail_key}' is not a string");
    };
    serde_json::from_str(&detail)
        .with_context(|| format!("existing GitMeta key '{detail_key}' has invalid JSON"))
        .map(Some)
}

pub(super) fn read_transcript_entries(
    handle: &SessionTargetHandle<'_>,
    transcript_key: &str,
) -> Result<Vec<ListEntry>> {
    let Some(transcript_value) = handle
        .get_value(transcript_key)
        .with_context(|| format!("failed to read GitMeta key '{transcript_key}'"))?
    else {
        bail!("existing GitMeta key '{transcript_key}' is missing");
    };
    let MetaValue::List(transcript_entries) = transcript_value else {
        bail!("existing GitMeta key '{transcript_key}' is not a list");
    };
    Ok(transcript_entries)
}

pub(super) fn transcript_records_by_hash<T>(
    entries: Vec<ListEntry>,
    needed_hashes: &std::collections::HashSet<String>,
    mut parse_record: impl FnMut(&str) -> Option<(String, T)>,
) -> HashMap<String, T> {
    let mut records = HashMap::with_capacity(needed_hashes.len());
    for entry in entries.into_iter().rev() {
        let Some((record_hash, record)) = parse_record(&entry.value) else {
            continue;
        };
        if !needed_hashes.contains(&record_hash) {
            continue;
        }
        records.insert(record_hash, record);
        if records.len() == needed_hashes.len() {
            break;
        }
    }
    records
}

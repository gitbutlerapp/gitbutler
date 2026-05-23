use std::{collections::HashSet, path::Path};

use anyhow::{Context as _, Result, bail};
use chrono::{SecondsFormat, Utc};
use git_meta_lib::{ListEntry, MetaEdit, MetaValue, Session, SessionTargetHandle, Target};
use serde::Serialize;
use serde_json::Value;

use crate::{
    agent::Agent,
    environment::{EnvironmentObservation, ObservedTargets, SnapshotStatus},
    redaction::{redact_text, redact_value},
    transcript::{RecordKind, TranscriptBatch},
};

use super::{
    AcceptedRecord, CaptureKind, IndexHit, TurnDetail, TurnSummary, cap_tool_result_text,
    capture_turn_key, index_key, latest_stored_turn,
};

#[derive(Debug)]
pub(crate) struct CaptureWriteOutcome {
    pub(crate) records_written: usize,
    pub(crate) metadata_changed: bool,
}

pub(crate) fn write_transcript_batch(
    repo_path: &Path,
    agent: Agent,
    session_key: &str,
    source_key: &str,
    batch: TranscriptBatch,
    capture_environment: impl FnOnce() -> EnvironmentObservation,
) -> Result<CaptureWriteOutcome> {
    let TranscriptBatch {
        provider,
        model,
        tool_version,
        mut records,
        ..
    } = batch;

    if records.is_empty() {
        return Ok(CaptureWriteOutcome {
            records_written: 0,
            metadata_changed: false,
        });
    }

    let session_prefix = format!("gitbutler:agent-session:{session_key}");
    let sources_key = format!("{session_prefix}:sources");
    let source_prefix = format!("{session_prefix}:source:{source_key}");
    let transcript_key = format!("{session_prefix}:transcript");
    let record_hashes_key = format!("{session_prefix}:record-hashes");
    let turns_key = format!("{session_prefix}:turns");

    let gitmeta = Session::open(repo_path).context("failed to open GitMeta session")?;
    let target = Target::project();
    let handle = gitmeta.target(&target);
    let turns_value = handle
        .get_value(&turns_key)
        .with_context(|| format!("failed to read GitMeta key '{turns_key}'"))?;
    let previous_turn = latest_stored_turn(turns_value.as_ref(), &turns_key)?;
    let previous_turn_key = previous_turn.as_ref().map(|turn| turn.turn_key.to_owned());
    let incoming_record_hashes = records
        .iter()
        .map(|record| record.source_record_hash.clone())
        .collect::<Vec<_>>();
    let mut seen_hashes = match handle
        .get_value(&record_hashes_key)
        .with_context(|| format!("failed to read GitMeta key '{record_hashes_key}'"))?
    {
        None => HashSet::new(),
        Some(MetaValue::Set(hashes)) => hashes.into_iter().collect(),
        Some(_) => bail!("existing GitMeta key '{record_hashes_key}' is not a set"),
    };
    records.retain(|record| {
        if seen_hashes.contains(&record.source_record_hash) {
            false
        } else {
            seen_hashes.insert(record.source_record_hash.clone());
            true
        }
    });

    if records.is_empty() {
        let metadata_changed = enrich_incomplete_turn(
            &handle,
            turns_value,
            &session_prefix,
            session_key,
            source_key,
            &incoming_record_hashes,
            capture_environment,
        )?;
        return Ok(CaptureWriteOutcome {
            records_written: 0,
            metadata_changed,
        });
    }

    let records_captured = records.len();
    let capture_kind = if previous_turn_key.is_some() {
        CaptureKind::Incremental
    } else {
        CaptureKind::Backfill
    };
    let mut record_hashes = Vec::with_capacity(records_captured);
    let mut transcript_entries = Vec::with_capacity(records_captured);
    let mut accepted_records = Vec::with_capacity(records_captured);
    let now = Utc::now().timestamp_millis();
    let entry_timestamp = previous_turn
        .as_ref()
        .map_or(now, |turn| now.max(turn.timestamp + 1));
    for (entry_timestamp, record) in (entry_timestamp..).zip(records) {
        let record_hash = record.source_record_hash;
        let transcript_record = TranscriptRecord {
            source_key,
            record_index: record.index,
            record_hash: &record_hash,
            timestamp: record.source_timestamp.as_deref().map(redact_text),
            kind: record.kind,
            source_event_kind: redact_text(&record.source_event_kind),
            role: record.role.as_deref().map(redact_text),
            text: stored_text(record.kind, record.text.as_deref()),
            tool_name: record.tool_name.as_deref().map(redact_text),
            tool_input: record.tool_input.map(redact_value),
            source_record: redact_value(record.source_record),
        };
        let transcript_record = serde_json::to_string(&transcript_record)
            .context("failed to serialize transcript record")?;
        accepted_records.push(AcceptedRecord {
            record_hash: record_hash.clone(),
        });
        record_hashes.push(record_hash);
        transcript_entries.push(ListEntry {
            value: transcript_record,
            timestamp: entry_timestamp,
        });
    }

    let turn_key = capture_turn_key(session_key, source_key, &accepted_records);
    let turn_detail_key = format!("{session_prefix}:turn:{turn_key}");
    let environment_observation = capture_environment();

    let updated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let turn_summary = TurnSummary {
        turn_key: turn_key.clone(),
        source_key: source_key.to_owned(),
        previous_turn_key: previous_turn_key.clone(),
        capture_kind,
        captured_at: updated_at.clone(),
        environment_snapshot_status: environment_observation.snapshot_status(),
    };
    let turn_detail = TurnDetail {
        schema: "gitbutler.agent-session-turn.v1",
        turn_key: &turn_key,
        session_key,
        source_key,
        previous_turn_key: previous_turn_key.as_deref(),
        capture_kind,
        captured_at: &updated_at,
        records: &accepted_records,
        observed_targets: environment_observation.observed_targets(),
        environment: environment_observation.environment(),
    };
    let turn_summary_entries = [ListEntry {
        value: serde_json::to_string(&turn_summary).context("failed to serialize turn summary")?,
        timestamp: entry_timestamp + records_captured as i64,
    }];

    let session_keys = [session_key.to_owned()];
    let source_keys = [source_key.to_owned()];
    let session_schema_key = format!("{session_prefix}:schema");
    let updated_at_key = format!("{session_prefix}:updated-at");
    let session_schema_value = MetaValue::String("gitbutler.agent-session.v1".to_owned());
    let updated_at_value = MetaValue::String(updated_at.clone());
    let source_fields =
        source_metadata_fields(&source_prefix, agent, provider, model, tool_version);
    let turn_detail_value = MetaValue::String(
        serde_json::to_string(&turn_detail).context("failed to serialize turn detail")?,
    );
    let index_hit = serde_json::to_string(&IndexHit {
        session_key: session_key.to_owned(),
        turn_key,
    })
    .context("failed to serialize agentlog index hit")?;
    let index_hit_members = [index_hit];
    let index_keys = observed_target_index_keys(environment_observation.observed_targets());

    let mut edits = vec![
        MetaEdit::set_add("gitbutler:agent-sessions", &session_keys),
        MetaEdit::set_value(&session_schema_key, &session_schema_value),
        MetaEdit::set_value(&updated_at_key, &updated_at_value),
        MetaEdit::set_add(&sources_key, &source_keys),
    ];
    edits.extend(
        source_fields
            .iter()
            .map(|(key, value)| MetaEdit::set_value(key, value)),
    );
    edits.extend([
        MetaEdit::list_append(&transcript_key, &transcript_entries),
        MetaEdit::set_add(&record_hashes_key, &record_hashes),
        MetaEdit::list_append(&turns_key, &turn_summary_entries),
        MetaEdit::set_value(&turn_detail_key, &turn_detail_value),
    ]);
    edits.extend(
        index_keys
            .iter()
            .map(|key| MetaEdit::set_add(key, &index_hit_members)),
    );

    handle
        .apply_edits(edits)
        .context("failed to write agent session metadata")?;

    Ok(CaptureWriteOutcome {
        records_written: records_captured,
        metadata_changed: true,
    })
}

pub(crate) fn sync_metadata(repo_path: &Path) -> Result<()> {
    const MAX_RETRIES: usize = 5;

    let gitmeta = Session::open(repo_path).context("failed to open GitMeta session")?;
    match gitmeta.pull(None) {
        Ok(_) => {}
        // An empty metadata remote has no ref yet; the push below initializes it.
        Err(git_meta_lib::Error::GitCommand(message))
            if message.contains("couldn't find remote ref") => {}
        Err(err) => return Err(err).context("failed to pull GitMeta metadata"),
    }

    let mut attempts = 0;
    loop {
        attempts += 1;
        let output = gitmeta
            .push_once(None)
            .context("failed to push GitMeta metadata")?;
        if output.success {
            return Ok(());
        }
        if !output.non_fast_forward {
            bail!("push failed");
        }
        if attempts >= MAX_RETRIES {
            bail!("push failed after {MAX_RETRIES} attempts");
        }
        gitmeta
            .resolve_push_conflict(None)
            .context("failed to resolve GitMeta push conflict")?;
    }
}

fn stored_text(kind: RecordKind, text: Option<&str>) -> Option<String> {
    let text = text?;
    Some(match kind {
        RecordKind::ToolResult => redact_text(cap_tool_result_text(text).as_ref()),
        _ => redact_text(text),
    })
}

fn source_metadata_fields(
    prefix: &str,
    agent: Agent,
    provider: Option<String>,
    model: Option<String>,
    tool_version: Option<String>,
) -> Vec<(String, MetaValue)> {
    let mut fields = vec![(
        format!("{prefix}:agent"),
        MetaValue::String(agent.as_str().to_owned()),
    )];
    if let Some(provider) = provider {
        fields.push((
            format!("{prefix}:provider"),
            MetaValue::String(redact_text(&provider)),
        ));
    }
    if let Some(model) = model {
        fields.push((
            format!("{prefix}:model"),
            MetaValue::String(redact_text(&model)),
        ));
    }
    if let Some(tool_version) = tool_version {
        fields.push((
            format!("{prefix}:tool-version"),
            MetaValue::String(redact_text(&tool_version)),
        ));
    }
    fields
}

fn observed_target_index_keys(observed_targets: &ObservedTargets) -> Vec<String> {
    let mut keys = Vec::new();
    keys.extend(
        observed_targets
            .branch_keys()
            .map(|key| index_key("branch", key)),
    );
    keys.extend(
        observed_targets
            .review_keys()
            .map(|key| index_key("review", key)),
    );
    keys.extend(
        observed_targets
            .change_keys()
            .map(|key| index_key("change", key)),
    );
    keys
}

fn enrich_incomplete_turn(
    handle: &SessionTargetHandle<'_>,
    turns_value: Option<MetaValue>,
    session_prefix: &str,
    session_key: &str,
    source_key: &str,
    incoming_record_hashes: &[String],
    capture_environment: impl FnOnce() -> EnvironmentObservation,
) -> Result<bool> {
    let Some(MetaValue::List(mut turn_entries)) = turns_value else {
        return Ok(false);
    };

    let turns_key = format!("{session_prefix}:turns");
    let Some((turn_index, mut summary, turn_key, mut detail)) = incomplete_turn_matching_records(
        handle,
        &turn_entries,
        &turns_key,
        session_prefix,
        source_key,
        incoming_record_hashes,
    )?
    else {
        return Ok(false);
    };
    let current_status = summary["environment_snapshot_status"]
        .as_str()
        .unwrap_or_default();

    let environment_observation = capture_environment();
    let new_status = environment_observation.snapshot_status();
    let improves_status = matches!(
        (current_status, new_status),
        ("failed", SnapshotStatus::Partial | SnapshotStatus::Complete)
            | ("partial", SnapshotStatus::Complete)
    );
    if !improves_status {
        return Ok(false);
    }

    let turn_detail_key = format!("{session_prefix}:turn:{turn_key}");
    let updated_at_key = format!("{session_prefix}:updated-at");

    summary["environment_snapshot_status"] = serde_json::to_value(new_status)
        .context("failed to serialize environment snapshot status")?;
    turn_entries[turn_index].value =
        serde_json::to_string(&summary).context("failed to serialize enriched turn summary")?;

    detail["observed_targets"] = serde_json::to_value(environment_observation.observed_targets())
        .context("failed to serialize enriched observed targets")?;
    detail["environment"] = serde_json::to_value(environment_observation.environment())
        .context("failed to serialize enriched environment snapshot")?;

    let turns_value = MetaValue::List(turn_entries);
    let turn_detail_value = MetaValue::String(
        serde_json::to_string(&detail).context("failed to serialize enriched turn detail")?,
    );
    let updated_at_value =
        MetaValue::String(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true));
    let index_hit = serde_json::to_string(&IndexHit {
        session_key: session_key.to_owned(),
        turn_key: turn_key.to_owned(),
    })
    .context("failed to serialize agentlog index hit")?;
    let index_hit_members = [index_hit];
    let index_keys = observed_target_index_keys(environment_observation.observed_targets());

    let mut edits = vec![
        MetaEdit::set_value(&updated_at_key, &updated_at_value),
        MetaEdit::set_value(&turns_key, &turns_value),
        MetaEdit::set_value(&turn_detail_key, &turn_detail_value),
    ];
    edits.extend(
        index_keys
            .iter()
            .map(|key| MetaEdit::set_add(key, &index_hit_members)),
    );

    handle
        .apply_edits(edits)
        .context("failed to enrich agent session turn")?;
    Ok(true)
}

fn incomplete_turn_matching_records(
    handle: &SessionTargetHandle<'_>,
    entries: &[ListEntry],
    turns_key: &str,
    session_prefix: &str,
    source_key: &str,
    incoming_record_hashes: &[String],
) -> Result<Option<(usize, Value, String, Value)>> {
    let incoming_record_hashes = incoming_record_hashes
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut best = None;
    for (index, entry) in entries.iter().enumerate() {
        let summary: Value = serde_json::from_str(&entry.value).with_context(|| {
            format!("existing GitMeta key '{turns_key}' has invalid turn summary")
        })?;
        if !matches!(
            summary["environment_snapshot_status"].as_str(),
            Some("failed" | "partial")
        ) || summary["source_key"].as_str() != Some(source_key)
        {
            continue;
        }
        let turn_key = summary["turn_key"]
            .as_str()
            .context("existing turn summary is missing turn_key")?
            .to_owned();
        let detail_key = format!("{session_prefix}:turn:{turn_key}");
        let Some(MetaValue::String(detail)) = handle
            .get_value(&detail_key)
            .with_context(|| format!("failed to read GitMeta key '{detail_key}'"))?
        else {
            bail!("existing GitMeta key '{detail_key}' is not a string");
        };
        let detail: Value = serde_json::from_str(&detail)
            .with_context(|| format!("existing GitMeta key '{detail_key}' has invalid JSON"))?;
        if !detail_records_are_incoming(&detail, &incoming_record_hashes) {
            continue;
        }
        let is_new_best = best
            .as_ref()
            .is_none_or(|(timestamp, best_index, _, _, _)| {
                entry
                    .timestamp
                    .cmp(timestamp)
                    .then_with(|| index.cmp(best_index))
                    .is_gt()
            });
        if is_new_best {
            best = Some((entry.timestamp, index, summary, turn_key, detail));
        }
    }
    Ok(best.map(|(_, index, summary, turn_key, detail)| (index, summary, turn_key, detail)))
}

fn detail_records_are_incoming(detail: &Value, incoming_record_hashes: &HashSet<&str>) -> bool {
    detail["records"].as_array().is_some_and(|records| {
        !records.is_empty()
            && records.iter().all(|record| {
                record["record_hash"]
                    .as_str()
                    .is_some_and(|hash| incoming_record_hashes.contains(hash))
            })
    })
}

#[derive(Serialize)]
struct TranscriptRecord<'a> {
    source_key: &'a str,
    record_index: usize,
    record_hash: &'a str,
    timestamp: Option<String>,
    kind: RecordKind,
    source_event_kind: String,
    role: Option<String>,
    text: Option<String>,
    tool_name: Option<String>,
    tool_input: Option<Value>,
    source_record: Value,
}

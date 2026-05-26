use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use git_meta_lib::{MetaValue, SessionTargetHandle};
use serde::Deserialize;

use super::{
    AcceptedRecord, IndexHit, SessionListEntry, StoredObservedTargets, index_key,
    read_support::{
        read_optional_turn_detail, read_transcript_entries, read_turn_detail, read_turn_summaries,
        transcript_records_by_hash, with_project_target,
    },
    session_outline::{RelatedSession, related_session_outline},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelatedTarget<'a> {
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

pub(crate) fn find_related_sessions_limited(
    repo_path: &Path,
    target: RelatedTarget<'_>,
    max_sessions: Option<usize>,
) -> Result<Vec<RelatedSession>> {
    with_project_target(repo_path, |handle| {
        let mut matches = Vec::new();
        for (session_key, related_turn_keys) in verified_related_turns_by_session(handle, target)? {
            let entry = session_list_entry(handle, session_key)?;
            matches.push((entry, related_turn_keys));
        }
        matches.sort_by(|(lhs, _), (rhs, _)| {
            rhs.sort_updated_at
                .cmp(&lhs.sort_updated_at)
                .then_with(|| lhs.session_key.cmp(&rhs.session_key))
        });
        if let Some(max_sessions) = max_sessions {
            matches.truncate(max_sessions);
        }
        matches
            .into_iter()
            .map(|(entry, related_turn_keys)| {
                related_session_outline(handle, entry, related_turn_keys)
            })
            .collect()
    })
}

fn verified_related_turns_by_session(
    handle: &SessionTargetHandle<'_>,
    target: RelatedTarget<'_>,
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut indexed_turns = BTreeMap::<String, HashSet<String>>::new();
    let mut session_activity_matches = BTreeMap::<String, bool>::new();
    for hit in target_index_hits(handle, target)? {
        let Ok(hit) = serde_json::from_str::<IndexHit>(&hit) else {
            continue;
        };
        if turn_detail_observes_target(handle, &hit, target, &mut session_activity_matches)? {
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

fn turn_detail_observes_target(
    handle: &SessionTargetHandle<'_>,
    hit: &IndexHit,
    target: RelatedTarget<'_>,
    session_activity_matches: &mut BTreeMap<String, bool>,
) -> Result<bool> {
    let detail_key = format!(
        "gitbutler:agent-session:{}:turn:{}",
        hit.session_key, hit.turn_key
    );
    let Some(detail) = read_optional_turn_detail(handle, &detail_key)? else {
        return Ok(false);
    };
    if !observed_targets_observe(&detail.observed_targets, target)
        && !session_associations_observe_target(handle, &hit.session_key, target)?
    {
        return Ok(false);
    }
    if let Some(matches) = session_activity_matches.get(&hit.session_key) {
        return Ok(*matches);
    }
    let matches = session_has_target_activity(handle, &hit.session_key, target)?;
    session_activity_matches.insert(hit.session_key.clone(), matches);
    Ok(matches)
}

fn observed_targets_observe(targets: &StoredObservedTargets, target: RelatedTarget<'_>) -> bool {
    match target {
        RelatedTarget::Branch(key) => targets.branches.iter().any(|target| target.key == key),
        RelatedTarget::Review(key) => targets.reviews.iter().any(|target| target.key == key),
        RelatedTarget::Change(key) => targets.changes.iter().any(|target| target.key == key),
    }
}

fn session_associations_observe_target(
    handle: &SessionTargetHandle<'_>,
    session_key: &str,
    target: RelatedTarget<'_>,
) -> Result<bool> {
    let associated_targets_key =
        format!("gitbutler:agent-session:{session_key}:associated-targets");
    let Some(value) = handle
        .get_value(&associated_targets_key)
        .with_context(|| format!("failed to read GitMeta key '{associated_targets_key}'"))?
    else {
        return Ok(false);
    };
    let MetaValue::String(value) = value else {
        bail!("existing GitMeta key '{associated_targets_key}' is not a string");
    };
    let targets: StoredSessionAssociations = serde_json::from_str(&value).with_context(|| {
        format!("existing GitMeta key '{associated_targets_key}' has invalid JSON")
    })?;
    Ok(targets.observes(target))
}

#[derive(Deserialize)]
struct StoredSessionAssociations {
    #[serde(default)]
    branches: BTreeSet<String>,
    #[serde(default)]
    reviews: BTreeSet<String>,
    #[serde(default)]
    changes: BTreeSet<String>,
}

impl StoredSessionAssociations {
    fn observes(&self, target: RelatedTarget<'_>) -> bool {
        match target {
            RelatedTarget::Branch(key) => self.branches.contains(key),
            RelatedTarget::Review(key) => self.reviews.contains(key),
            RelatedTarget::Change(key) => self.changes.contains(key),
        }
    }
}

fn session_has_target_activity(
    handle: &SessionTargetHandle<'_>,
    session_key: &str,
    target: RelatedTarget<'_>,
) -> Result<bool> {
    let session_prefix = format!("gitbutler:agent-session:{session_key}");
    let summaries = read_turn_summaries(handle, &format!("{session_prefix}:turns"))?;
    let mut saw_turn_before_target = false;
    for summary in summaries {
        let detail_key = format!("{session_prefix}:turn:{}", summary.turn_key);
        let detail = read_turn_detail(handle, &detail_key)?;
        if observed_targets_observe(&detail.observed_targets, target) {
            if saw_turn_before_target
                || turn_has_agent_activity(handle, &session_prefix, &detail.records)?
            {
                return Ok(true);
            }
        } else {
            saw_turn_before_target = true;
        }
    }
    Ok(false)
}

#[derive(Deserialize)]
struct StoredActivityRecord {
    record_hash: String,
    #[serde(default)]
    tool_kind: Option<String>,
    tool_input: Option<serde_json::Value>,
}

fn turn_has_agent_activity(
    handle: &SessionTargetHandle<'_>,
    session_prefix: &str,
    accepted_records: &[AcceptedRecord],
) -> Result<bool> {
    if accepted_records.is_empty() {
        return Ok(false);
    }
    let transcript_key = format!("{session_prefix}:transcript");
    let records = activity_records_by_hash(handle, &transcript_key, accepted_records)?;
    Ok(records.values().any(activity_record_touches_target))
}

fn activity_records_by_hash(
    handle: &SessionTargetHandle<'_>,
    transcript_key: &str,
    accepted_records: &[AcceptedRecord],
) -> Result<BTreeMap<String, StoredActivityRecord>> {
    let needed_hashes = accepted_records
        .iter()
        .map(|record| record.record_hash.clone())
        .collect::<HashSet<_>>();
    let entries = read_transcript_entries(handle, transcript_key)?;
    Ok(
        transcript_records_by_hash(entries, &needed_hashes, parse_activity_record)
            .into_iter()
            .collect(),
    )
}

fn parse_activity_record(raw: &str) -> Option<(String, StoredActivityRecord)> {
    let record = serde_json::from_str::<StoredActivityRecord>(raw).ok()?;
    Some((record.record_hash.clone(), record))
}

fn activity_record_touches_target(record: &StoredActivityRecord) -> bool {
    match record.tool_kind.as_deref() {
        Some("exec") => record
            .tool_input
            .as_ref()
            .and_then(command_text)
            .is_some_and(is_mutating_repo_command),
        _ => false,
    }
}

fn command_text(input: &serde_json::Value) -> Option<&str> {
    if let Some(command) = input.as_str() {
        return Some(command);
    }
    ["cmd", "command", "input"]
        .into_iter()
        .find_map(|key| input.get(key).and_then(serde_json::Value::as_str))
}

fn is_mutating_repo_command(command: impl AsRef<str>) -> bool {
    command
        .as_ref()
        .lines()
        .flat_map(|line| line.split("&&"))
        .flat_map(|line| line.split(';'))
        .any(|segment| {
            let tokens = segment.split_whitespace().collect::<Vec<_>>();
            mutating_but_subcommand(&tokens).is_some_and(is_mutating_but_subcommand)
                || mutating_git_subcommand(&tokens).is_some_and(is_mutating_git_subcommand)
        })
}

fn mutating_but_subcommand<'a>(tokens: &'a [&str]) -> Option<&'a str> {
    let first = *tokens.first()?;
    if first == "but" {
        return first_non_option(&tokens[1..]);
    }
    if first == "cargo" && tokens.get(1).copied() == Some("run") {
        let after_separator = tokens
            .iter()
            .position(|token| *token == "--")
            .map(|index| &tokens[index + 1..])?;
        return first_non_option(after_separator);
    }
    None
}

fn mutating_git_subcommand<'a>(tokens: &'a [&str]) -> Option<&'a str> {
    if tokens.first().copied() == Some("git") {
        first_non_option(&tokens[1..])
    } else {
        None
    }
}

fn first_non_option<'a>(tokens: &'a [&str]) -> Option<&'a str> {
    let mut skip_next = false;
    for token in tokens.iter().copied() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if matches!(
            token,
            "-C" | "--current-dir" | "--git-dir" | "--work-tree" | "-c"
        ) {
            skip_next = true;
            continue;
        }
        if token.starts_with("-C")
            || token.starts_with("--current-dir=")
            || token.starts_with("--git-dir=")
            || token.starts_with("--work-tree=")
            || token.starts_with("-c")
        {
            continue;
        }
        if token.starts_with('-') || token.contains('=') {
            continue;
        }
        return Some(token);
    }
    None
}

fn is_mutating_but_subcommand(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "absorb"
            | "amend"
            | "apply"
            | "commit"
            | "discard"
            | "move"
            | "pick"
            | "pr"
            | "push"
            | "resolve"
            | "reword"
            | "rub"
            | "squash"
            | "stage"
            | "unapply"
            | "uncommit"
    )
}

fn is_mutating_git_subcommand(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "add"
            | "am"
            | "apply"
            | "checkout"
            | "cherry-pick"
            | "commit"
            | "merge"
            | "mv"
            | "push"
            | "rebase"
            | "reset"
            | "revert"
            | "rm"
            | "switch"
    )
}

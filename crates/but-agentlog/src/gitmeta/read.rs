use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use git_meta_lib::{MetaValue, SessionTargetHandle};

use super::{
    IndexHit, SessionListEntry, StoredObservedTargets, index_key,
    read_support::{read_optional_turn_detail, read_turn_summaries, with_project_target},
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

fn turn_detail_observes_target(
    handle: &SessionTargetHandle<'_>,
    hit: &IndexHit,
    target: RelatedTarget<'_>,
) -> Result<bool> {
    let detail_key = format!(
        "gitbutler:agent-session:{}:turn:{}",
        hit.session_key, hit.turn_key
    );
    let Some(detail) = read_optional_turn_detail(handle, &detail_key)? else {
        return Ok(false);
    };
    Ok(observed_targets_observe(&detail.observed_targets, target))
}

fn observed_targets_observe(targets: &StoredObservedTargets, target: RelatedTarget<'_>) -> bool {
    match target {
        RelatedTarget::Branch(key) => targets.branches.iter().any(|target| target.key == key),
        RelatedTarget::Review(key) => targets.reviews.iter().any(|target| target.key == key),
        RelatedTarget::Change(key) => targets.changes.iter().any(|target| target.key == key),
    }
}

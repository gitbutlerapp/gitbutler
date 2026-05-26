use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use git_meta_lib::{MetaValue, SessionTargetHandle};
use serde::Deserialize;

use crate::environment::path_fingerprint;

use super::{
    IndexHit, SessionListEntry, StoredBranchSnapshot, StoredCommitSnapshot,
    StoredEnvironmentSnapshot, StoredObservedTargets, index_key,
    read_support::{
        read_optional_turn_detail, read_turn_detail, read_turn_summaries, with_project_target,
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
    let mut previous_detail: Option<super::StoredTurnDetail> = None;
    for summary in summaries {
        let detail_key = format!("{session_prefix}:turn:{}", summary.turn_key);
        let detail = read_turn_detail(handle, &detail_key)?;
        if previous_detail.as_ref().is_some_and(|previous| {
            environment_promotes_worktree_to_target(
                &previous.environment,
                &detail.environment,
                target,
            )
        }) {
            return Ok(true);
        }
        if observed_targets_observe(&detail.observed_targets, target) {
            if saw_turn_before_target
                && target_appearance_is_specific(&detail.observed_targets, target)
            {
                return Ok(true);
            }
        } else {
            saw_turn_before_target = true;
        }
        previous_detail = Some(detail);
    }
    Ok(false)
}

fn target_appearance_is_specific(
    targets: &StoredObservedTargets,
    target: RelatedTarget<'_>,
) -> bool {
    match target {
        RelatedTarget::Branch(_) => targets.branches.len() == 1,
        RelatedTarget::Review(_) => targets.branches.len() <= 1,
        RelatedTarget::Change(_) => targets.changes.len() == 1,
    }
}

fn environment_promotes_worktree_to_target(
    previous: &StoredEnvironmentSnapshot,
    current: &StoredEnvironmentSnapshot,
    target: RelatedTarget<'_>,
) -> bool {
    // Strong association: files dirty in one turn show up in a new commit on the
    // target in the next turn. Ambient applied branches stay weak.
    if !environment_is_complete(previous) || !environment_is_complete(current) {
        return false;
    }
    let Some(previous_worktree) = previous.worktree.as_ref() else {
        return false;
    };
    let previous_worktree_files = previous_worktree
        .files
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if previous_worktree_files.is_empty() {
        return false;
    }
    let previous_worktree_file_hashes = previous_worktree_files
        .iter()
        .map(|file| path_fingerprint(file))
        .collect::<BTreeSet<_>>();

    environment_branches(current).any(|branch| {
        let previous_branch = match previous_branch_snapshot(previous, &branch.key) {
            PreviousBranchSnapshot::Absent => KnownPreviousBranch::default(),
            PreviousBranchSnapshot::Known(snapshot) => snapshot,
            PreviousBranchSnapshot::Unknown => return false,
        };
        branch_matches_target(branch, target)
            && branch.commits.iter().any(|commit| {
                commit_matches_target(commit, target)
                    && !previous_branch.commit_ids.contains(&commit.id)
                    && commit_contains_worktree_file(
                        commit,
                        &previous_worktree_files,
                        &previous_worktree_file_hashes,
                        &previous_branch,
                    )
            })
    })
}

fn environment_is_complete(environment: &StoredEnvironmentSnapshot) -> bool {
    environment.snapshot_status.as_deref() == Some("complete") && environment.error_kind.is_none()
}

enum PreviousBranchSnapshot {
    Absent,
    Known(KnownPreviousBranch),
    Unknown,
}

#[derive(Default)]
struct KnownPreviousBranch {
    commit_ids: BTreeSet<String>,
    file_hashes: BTreeSet<String>,
    files: BTreeSet<String>,
}

fn commit_contains_worktree_file(
    commit: &StoredCommitSnapshot,
    previous_worktree_files: &BTreeSet<&str>,
    previous_worktree_file_hashes: &BTreeSet<String>,
    previous_branch: &KnownPreviousBranch,
) -> bool {
    commit.file_hashes.iter().any(|hash| {
        previous_worktree_file_hashes.contains(hash) && !previous_branch.file_hashes.contains(hash)
    }) || commit.files.iter().any(|file| {
        previous_worktree_files.contains(file.as_str())
            && !previous_branch.files.contains(file)
            && !previous_branch
                .file_hashes
                .contains(&path_fingerprint(file))
    })
}

fn environment_branches(
    environment: &StoredEnvironmentSnapshot,
) -> impl Iterator<Item = &StoredBranchSnapshot> {
    environment
        .stacks
        .iter()
        .flat_map(|stack| stack.branches.iter())
}

fn previous_branch_snapshot(
    environment: &StoredEnvironmentSnapshot,
    branch_key: &str,
) -> PreviousBranchSnapshot {
    let mut branch_exists = false;
    let mut snapshot = KnownPreviousBranch::default();
    for branch in environment_branches(environment).filter(|branch| branch.key == branch_key) {
        branch_exists = true;
        for commit in &branch.commits {
            snapshot.commit_ids.insert(commit.id.clone());
            snapshot
                .file_hashes
                .extend(commit.file_hashes.iter().cloned());
            snapshot
                .file_hashes
                .extend(commit.files.iter().map(|file| path_fingerprint(file)));
            snapshot.files.extend(commit.files.iter().cloned());
        }
    }
    if !branch_exists {
        PreviousBranchSnapshot::Absent
    } else if snapshot.commit_ids.is_empty() {
        PreviousBranchSnapshot::Unknown
    } else {
        PreviousBranchSnapshot::Known(snapshot)
    }
}

fn branch_matches_target(branch: &StoredBranchSnapshot, target: RelatedTarget<'_>) -> bool {
    match target {
        RelatedTarget::Branch(key) => branch.key == key,
        RelatedTarget::Review(key) => branch_reviews(branch).any(|review_key| review_key == key),
        RelatedTarget::Change(_) => true,
    }
}

fn branch_reviews(branch: &StoredBranchSnapshot) -> impl Iterator<Item = &str> {
    branch
        .legacy_review
        .iter()
        .chain(branch.reviews.iter())
        .map(|review| review.key.as_str())
}

fn commit_matches_target(commit: &StoredCommitSnapshot, target: RelatedTarget<'_>) -> bool {
    match target {
        RelatedTarget::Branch(_) | RelatedTarget::Review(_) => true,
        RelatedTarget::Change(key) => commit
            .change
            .as_ref()
            .is_some_and(|change| change.key == key),
    }
}

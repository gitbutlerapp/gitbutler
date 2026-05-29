//! Public/read projection primitives for Agent Trail.
//!
//! This module is the allowlisted boundary between captured GitMeta agent logs
//! and public review-facing consumers. It exposes verified PR/head-branch
//! matches, evidence tiers, opaque handles, and bounded redacted records without
//! leaking raw storage keys or raw `source_record` payloads.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use git_meta_lib::{MetaValue, Session, Target};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::environment::is_public_repo_path;
use crate::gitmeta::{
    RelatedSession, RelatedTarget, SessionRecords, find_related_sessions_limited,
    get_session_records, get_session_timeline_outline,
};
use crate::redaction::redact_text;

const DEFAULT_MAX_TURNS: usize = 32;
const DEFAULT_MAX_RECORDS_PER_TURN: usize = 512;
const DEFAULT_MAX_TEXT_CHARS: usize = 1_200;
/// Build a public/read projection for an Agent Trail pull-request view.
///
/// The returned value contains only allowlisted, redacted, bounded fields. Raw
/// GitMeta storage keys, raw provider session ids, raw `source_record` JSON, and
/// raw tool payloads are intentionally not present.
pub fn project_pr(repo_path: &Path, request: &ProjectionRequest) -> Result<Projection> {
    let review_target = request.review_target_key();
    let branch_target = request.branch_target_key();
    let review_sessions = find_related_sessions_limited(
        repo_path,
        RelatedTarget::Review(review_target.as_str()),
        None,
    )
    .context("failed to find review-target agent sessions")?;
    let branch_sessions = find_related_sessions_limited(
        repo_path,
        RelatedTarget::Branch(branch_target.as_str()),
        None,
    )
    .context("failed to find branch-target agent sessions")?;

    let has_review_matches = !review_sessions.is_empty();
    let has_branch_matches = !branch_sessions.is_empty();
    let mut warnings = Vec::new();
    if !has_review_matches && has_branch_matches {
        warnings.push(ProjectionWarning {
            kind: ProjectionWarningKind::NoReviewMatch,
            message: "No review-target match; showing branch-only evidence.".to_owned(),
        });
        warnings.push(ProjectionWarning {
            kind: ProjectionWarningKind::WeakBranchOnly,
            message: "Branch-only matches are weaker and may be unrelated.".to_owned(),
        });
    }
    if !has_review_matches && !has_branch_matches {
        warnings.push(ProjectionWarning {
            kind: ProjectionWarningKind::NoMatches,
            message: "Metadata exists, but no related agent sessions matched this PR.".to_owned(),
        });
    }

    let mut candidates = BTreeMap::<String, CandidateSession>::new();
    add_candidate_sessions(
        &mut candidates,
        review_sessions,
        ProjectionMatchKind::ReviewTarget,
    );
    add_candidate_sessions(
        &mut candidates,
        branch_sessions,
        ProjectionMatchKind::BranchTarget,
    );

    let source_metadata = session_source_metadata(repo_path, candidates.keys())?;
    let mut turn_candidates = Vec::new();
    for (session_key, candidate) in &candidates {
        let timeline =
            get_session_timeline_outline(repo_path, session_key, None).with_context(|| {
                format!("failed to read projected timeline for session '{session_key}'")
            })?;
        for turn in &timeline.turns {
            let reasons = candidate.turn_match_reasons(&turn.turn_key);
            if reasons.is_empty() {
                continue;
            }
            turn_candidates.push(CandidateTurn {
                session_key: session_key.clone(),
                turn_key: turn.turn_key.clone(),
                captured_at: turn.captured_at.clone(),
                turn_index: turn.turn_index,
            });
        }
    }

    turn_candidates.sort_by(|lhs, rhs| {
        lhs.captured_at
            .cmp(&rhs.captured_at)
            .then_with(|| lhs.session_key.cmp(&rhs.session_key))
            .then_with(|| lhs.turn_index.cmp(&rhs.turn_index))
    });
    let truncated = turn_candidates.len() > request.limits.max_turns;
    if truncated {
        warnings.push(ProjectionWarning {
            kind: ProjectionWarningKind::ProjectionLimitReached,
            message: "Older matching turns were dropped to keep the projection bounded.".to_owned(),
        });
    }
    let selected_start = turn_candidates
        .len()
        .saturating_sub(request.limits.max_turns);
    let selected_turns = turn_candidates
        .into_iter()
        .skip(selected_start)
        .collect::<Vec<_>>();

    let mut turns_by_session = BTreeMap::<String, Vec<ProjectionTurn>>::new();
    let mut selected_handles = Vec::new();
    let mut saw_partial_turn = false;
    let mut saw_truncated_record_text = false;
    for selected in selected_turns {
        let candidate = candidates
            .get(&selected.session_key)
            .context("selected turn references an unknown candidate session")?;
        let records = get_session_records(
            repo_path,
            &selected.session_key,
            &selected.turn_key,
            request.limits.max_records_per_turn,
        )
        .with_context(|| {
            format!(
                "failed to read projected records for session '{}' turn '{}'",
                selected.session_key, selected.turn_key
            )
        })?;
        let records_has_more_before = records.coverage.has_more_before;
        saw_partial_turn |= records_has_more_before;
        let turn_handle = turn_handle(request, &selected.session_key, &selected.turn_key);
        let public_records = project_records(
            request,
            &selected.session_key,
            &selected.turn_key,
            records,
            &mut saw_truncated_record_text,
        );
        let latest_user_preview = latest_public_preview(&public_records, "user");
        let latest_assistant_preview = latest_public_preview(&public_records, "assistant");
        selected_handles.push(turn_handle.clone());
        selected_handles.extend(public_records.iter().map(|record| record.handle.clone()));

        turns_by_session
            .entry(selected.session_key.clone())
            .or_default()
            .push(ProjectionTurn {
                handle: turn_handle,
                captured_at: selected.captured_at,
                coverage: ProjectionRecordCoverage {
                    showing_records: public_records.len(),
                    has_more_before: records_has_more_before,
                },
                evidence_tier: candidate.turn_evidence_tier(&selected.turn_key),
                match_reasons: candidate.turn_match_reasons(&selected.turn_key),
                latest_user_preview,
                latest_assistant_preview,
                records: public_records,
            });
    }

    if saw_partial_turn {
        warnings.push(ProjectionWarning {
            kind: ProjectionWarningKind::PartialTurn,
            message: "At least one turn has more records than this bounded projection includes."
                .to_owned(),
        });
    }
    if saw_truncated_record_text {
        warnings.push(ProjectionWarning {
            kind: ProjectionWarningKind::TruncatedRecord,
            message: "At least one record text was truncated to keep the projection bounded."
                .to_owned(),
        });
    }

    let mut sessions = Vec::new();
    for (session_key, candidate) in candidates {
        let Some(turns) = turns_by_session.remove(&session_key) else {
            continue;
        };
        let evidence_tier = candidate.session_evidence_tier();
        let match_reasons = candidate.session_match_reasons();
        sessions.push(ProjectionSession {
            handle: session_handle(request, &session_key),
            agents: source_metadata
                .get(&session_key)
                .cloned()
                .unwrap_or_default(),
            started_at: candidate.outline.started_at,
            updated_at: candidate.outline.updated_at,
            latest_captured_at: candidate.outline.latest_captured_at,
            evidence_tier,
            match_reasons,
            turns,
        });
    }
    sessions.sort_by(|lhs, rhs| {
        evidence_rank(lhs.evidence_tier)
            .cmp(&evidence_rank(rhs.evidence_tier))
            .then_with(|| rhs.latest_captured_at.cmp(&lhs.latest_captured_at))
            .then_with(|| lhs.handle.cmp(&rhs.handle))
    });

    selected_handles.extend(sessions.iter().map(|session| session.handle.clone()));
    selected_handles.sort();
    let status = match (has_review_matches, has_branch_matches) {
        (true, _) => ProjectionStatus::Ready,
        (false, true) => ProjectionStatus::WeakMatches,
        (false, false) => ProjectionStatus::NoMatches,
    };
    Ok(Projection {
        pr: request.pr.clone(),
        snapshot: ProjectionSnapshot {
            metadata_oid: request.snapshot.metadata_oid.clone(),
            projection_version: request.snapshot.projection_version,
            generated_at_unix_seconds: request.snapshot.generated_at_unix_seconds,
            selected_evidence_digest: selected_evidence_digest(&selected_handles),
            truncated,
        },
        status,
        warnings,
        sessions,
    })
}

/// Public projection request. Agent Trail supplies GitHub PR facts and snapshot
/// facts; `but-agentlog` owns target construction and verified metadata lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionRequest {
    pub pr: ProjectionPrFacts,
    pub snapshot: ProjectionSnapshotInput,
    pub limits: ProjectionLimits,
}

impl ProjectionRequest {
    /// Construct a request with default projection limits.
    pub fn new(pr: ProjectionPrFacts, snapshot: ProjectionSnapshotInput) -> Self {
        Self {
            pr,
            snapshot,
            limits: ProjectionLimits::default(),
        }
    }

    fn review_target_key(&self) -> String {
        format!(
            "pull-request:{}#{}",
            normalize_branch_target_key(&self.pr.head_ref),
            self.pr.pull_request
        )
    }

    fn branch_target_key(&self) -> String {
        normalize_branch_target_key(&self.pr.head_ref)
    }
}

/// Pull-request facts used to construct review and fallback branch targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionPrFacts {
    pub agent_trail_host: String,
    pub github_repository_id: u64,
    pub owner: String,
    pub repo: String,
    pub pull_request: u64,
    pub base_ref: String,
    pub head_ref: String,
    pub head_sha: String,
}

/// Snapshot facts supplied by the caller and echoed in the projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionSnapshotInput {
    pub metadata_oid: String,
    pub projection_version: u32,
    pub generated_at_unix_seconds: u64,
}

/// Bounds for the public projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProjectionLimits {
    pub max_turns: usize,
    pub max_records_per_turn: usize,
    pub max_text_chars: usize,
}

impl Default for ProjectionLimits {
    fn default() -> Self {
        Self {
            max_turns: DEFAULT_MAX_TURNS,
            max_records_per_turn: DEFAULT_MAX_RECORDS_PER_TURN,
            max_text_chars: DEFAULT_MAX_TEXT_CHARS,
        }
    }
}

/// Public projection returned by [`project_pr`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Projection {
    pub pr: ProjectionPrFacts,
    pub snapshot: ProjectionSnapshot,
    pub status: ProjectionStatus,
    pub warnings: Vec<ProjectionWarning>,
    pub sessions: Vec<ProjectionSession>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionSnapshot {
    pub metadata_oid: String,
    pub projection_version: u32,
    pub generated_at_unix_seconds: u64,
    pub selected_evidence_digest: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionStatus {
    Ready,
    WeakMatches,
    NoMatches,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionWarning {
    pub kind: ProjectionWarningKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionWarningKind {
    NoReviewMatch,
    WeakBranchOnly,
    ProjectionLimitReached,
    PartialTurn,
    TruncatedRecord,
    NoMatches,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionSession {
    pub handle: String,
    pub agents: Vec<ProjectionAgentLabel>,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub latest_captured_at: Option<String>,
    pub evidence_tier: ProjectionEvidenceTier,
    pub match_reasons: Vec<ProjectionMatchReason>,
    pub turns: Vec<ProjectionTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionAgentLabel {
    pub agent: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tool_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionTurn {
    pub handle: String,
    pub captured_at: String,
    pub coverage: ProjectionRecordCoverage,
    pub evidence_tier: ProjectionEvidenceTier,
    pub match_reasons: Vec<ProjectionMatchReason>,
    pub latest_user_preview: Option<ProjectionPreview>,
    pub latest_assistant_preview: Option<ProjectionPreview>,
    pub records: Vec<ProjectionRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionEvidenceTier {
    Supporting,
    Direct,
    Possible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionMatchReason {
    pub kind: ProjectionMatchKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMatchKind {
    ReviewTarget,
    BranchTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionPreview {
    pub timestamp: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectionRecordCoverage {
    pub showing_records: usize,
    pub has_more_before: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionRecord {
    pub handle: String,
    pub timestamp: Option<String>,
    pub kind: Option<String>,
    pub role: Option<String>,
    pub prompt_source: Option<String>,
    pub text: Option<String>,
    pub text_truncated: bool,
    pub tool_name: Option<String>,
    pub tool_kind: Option<String>,
    pub file_paths: Vec<String>,
    pub exit_code: Option<i32>,
    pub outcome: Option<String>,
}

struct CandidateSession {
    outline: RelatedSession,
    has_review_match: bool,
    has_branch_match: bool,
    review_turn_keys: BTreeSet<String>,
    branch_turn_keys: BTreeSet<String>,
}

impl CandidateSession {
    fn new(outline: RelatedSession) -> Self {
        Self {
            outline,
            has_review_match: false,
            has_branch_match: false,
            review_turn_keys: BTreeSet::new(),
            branch_turn_keys: BTreeSet::new(),
        }
    }

    fn add_match(
        &mut self,
        kind: ProjectionMatchKind,
        turn_keys: impl IntoIterator<Item = String>,
    ) {
        match kind {
            ProjectionMatchKind::ReviewTarget => {
                self.has_review_match = true;
                self.review_turn_keys.extend(turn_keys);
            }
            ProjectionMatchKind::BranchTarget => {
                self.has_branch_match = true;
                self.branch_turn_keys.extend(turn_keys);
            }
        }
    }

    fn session_match_reasons(&self) -> Vec<ProjectionMatchReason> {
        match_reasons(self.has_review_match, self.has_branch_match)
    }

    fn turn_match_reasons(&self, turn_key: &str) -> Vec<ProjectionMatchReason> {
        match_reasons(
            self.review_turn_keys.contains(turn_key),
            self.branch_turn_keys.contains(turn_key),
        )
    }

    fn session_evidence_tier(&self) -> ProjectionEvidenceTier {
        evidence_tier(self.has_review_match, self.has_branch_match)
    }

    fn turn_evidence_tier(&self, turn_key: &str) -> ProjectionEvidenceTier {
        evidence_tier(
            self.review_turn_keys.contains(turn_key),
            self.branch_turn_keys.contains(turn_key),
        )
    }
}

struct CandidateTurn {
    session_key: String,
    turn_key: String,
    captured_at: String,
    turn_index: usize,
}

fn add_candidate_sessions(
    candidates: &mut BTreeMap<String, CandidateSession>,
    sessions: Vec<RelatedSession>,
    kind: ProjectionMatchKind,
) {
    for session in sessions {
        let turn_keys = session.related_turn_keys.clone();
        candidates
            .entry(session.session_key.clone())
            .or_insert_with(|| CandidateSession::new(session))
            .add_match(kind, turn_keys);
    }
}

fn match_reasons(has_review: bool, has_branch: bool) -> Vec<ProjectionMatchReason> {
    let mut reasons = Vec::new();
    if has_review {
        reasons.push(ProjectionMatchReason {
            kind: ProjectionMatchKind::ReviewTarget,
        });
    }
    if has_branch {
        reasons.push(ProjectionMatchReason {
            kind: ProjectionMatchKind::BranchTarget,
        });
    }
    reasons
}

fn evidence_tier(has_review: bool, has_branch: bool) -> ProjectionEvidenceTier {
    match (has_review, has_branch) {
        (true, true) => ProjectionEvidenceTier::Supporting,
        (true, false) => ProjectionEvidenceTier::Direct,
        (false, true) => ProjectionEvidenceTier::Possible,
        (false, false) => unreachable!("candidate turns always have review or branch evidence"),
    }
}

fn evidence_rank(tier: ProjectionEvidenceTier) -> usize {
    match tier {
        ProjectionEvidenceTier::Supporting => 0,
        ProjectionEvidenceTier::Direct => 1,
        ProjectionEvidenceTier::Possible => 2,
    }
}

fn project_records(
    request: &ProjectionRequest,
    session_key: &str,
    turn_key: &str,
    records: SessionRecords,
    saw_truncated_text: &mut bool,
) -> Vec<ProjectionRecord> {
    records
        .records
        .into_iter()
        .filter(|record| {
            record.prompt_source.as_deref() != Some("system_injected")
                && !matches!(record.role.as_deref(), Some("system" | "developer"))
        })
        .map(|record| {
            let (text, text_truncated) = public_text(&record.kind, record.text.as_deref(), request);
            *saw_truncated_text |= text_truncated;
            ProjectionRecord {
                handle: record_handle(request, session_key, turn_key, &record.record_hash),
                timestamp: record.timestamp,
                kind: record.kind,
                role: record.role,
                prompt_source: record.prompt_source,
                text,
                text_truncated,
                file_paths: file_paths_of(record.tool_input.as_ref()),
                tool_name: record.tool_name,
                tool_kind: record.tool_kind,
                exit_code: record.exit_code,
                outcome: record.outcome,
            }
        })
        .collect()
}

fn latest_public_preview(records: &[ProjectionRecord], role: &str) -> Option<ProjectionPreview> {
    records.iter().rev().find_map(|record| {
        if record.role.as_deref() != Some(role) {
            return None;
        }
        let text = record.text.as_deref()?.trim();
        if text.is_empty() {
            return None;
        }
        Some(ProjectionPreview {
            timestamp: record.timestamp.clone(),
            text: text.to_owned(),
        })
    })
}

fn public_text(
    kind: &Option<String>,
    text: Option<&str>,
    request: &ProjectionRequest,
) -> (Option<String>, bool) {
    let Some(text) = text else {
        return (None, false);
    };
    let cleaned = if kind.as_deref() == Some("tool_result") {
        Cow::Owned(
            text.lines()
                .filter(|line| !is_plumbing_line(line))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        Cow::Borrowed(text)
    };
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return (None, false);
    }
    let redacted = redact_text(trimmed);
    bounded_text(redacted.trim(), request.limits.max_text_chars)
}

fn file_paths_of(input: Option<&Value>) -> Vec<String> {
    let Some(input) = input else {
        return Vec::new();
    };
    let patch = input
        .get("patch")
        .or_else(|| input.get("input"))
        .or_else(|| input.get("content"))
        .and_then(Value::as_str)
        .or_else(|| input.as_str())
        .unwrap_or_default();
    let mut paths = Vec::new();
    for line in patch.lines() {
        let line = line.trim();
        for marker in [
            "*** Update File: ",
            "*** Add File: ",
            "*** Delete File: ",
            "*** Move to: ",
        ] {
            let Some(path) = line.strip_prefix(marker).map(str::trim) else {
                continue;
            };
            if is_public_repo_path(path) && !paths.iter().any(|existing| existing == path) {
                paths.push(path.to_owned());
            }
        }
    }
    paths
}

fn bounded_text(text: &str, max_chars: usize) -> (Option<String>, bool) {
    if max_chars == 0 {
        return (None, !text.is_empty());
    }
    let mut chars = text.chars();
    let out = chars.by_ref().take(max_chars).collect::<String>();
    let truncated = chars.next().is_some();
    (Some(out), truncated)
}

fn is_plumbing_line(line: &str) -> bool {
    let text = line.trim();
    if text.is_empty()
        || matches!(text, "Output:" | "Input:" | "Result:")
        || text.starts_with("Process exited")
        || text.starts_with("Process running")
    {
        return true;
    }
    [
        "Chunk ID",
        "Wall time",
        "Original token count",
        "yield_time_ms",
        "max_output_tokens",
        "Token count",
        "Tokens used",
        "Duration:",
    ]
    .iter()
    .any(|needle| text.starts_with(needle) || text.contains(needle))
}

fn session_source_metadata<'a>(
    repo_path: &Path,
    session_keys: impl IntoIterator<Item = &'a String>,
) -> Result<BTreeMap<String, Vec<ProjectionAgentLabel>>> {
    let gitmeta = Session::open(repo_path).context("failed to open GitMeta session")?;
    let target = Target::project();
    let handle = gitmeta.target(&target);
    let mut output = BTreeMap::new();
    for session_key in session_keys {
        let session_prefix = format!("gitbutler:agent-session:{session_key}");
        let sources_key = format!("{session_prefix}:sources");
        let sources = match handle
            .get_value(&sources_key)
            .with_context(|| format!("failed to read GitMeta key '{sources_key}'"))?
        {
            None => BTreeSet::new(),
            Some(MetaValue::Set(values)) => values.into_iter().collect(),
            Some(_) => bail!("existing GitMeta key '{sources_key}' is not a set"),
        };
        let mut labels = Vec::new();
        for source_key in sources {
            let source_prefix = format!("{session_prefix}:source:{source_key}");
            let label = ProjectionAgentLabel {
                agent: optional_string_value(&handle, &format!("{source_prefix}:agent"))?,
                provider: optional_string_value(&handle, &format!("{source_prefix}:provider"))?,
                model: optional_string_value(&handle, &format!("{source_prefix}:model"))?,
                tool_version: optional_string_value(
                    &handle,
                    &format!("{source_prefix}:tool-version"),
                )?,
            };
            if label.agent.is_some() || label.provider.is_some() || label.model.is_some() {
                labels.push(label);
            }
        }
        labels.sort_by(|lhs, rhs| {
            lhs.agent
                .cmp(&rhs.agent)
                .then_with(|| lhs.provider.cmp(&rhs.provider))
                .then_with(|| lhs.model.cmp(&rhs.model))
        });
        labels.dedup();
        output.insert(session_key.clone(), labels);
    }
    Ok(output)
}

fn optional_string_value(
    handle: &git_meta_lib::SessionTargetHandle<'_>,
    key: &str,
) -> Result<Option<String>> {
    match handle
        .get_value(key)
        .with_context(|| format!("failed to read GitMeta key '{key}'"))?
    {
        None => Ok(None),
        Some(MetaValue::String(value)) => Ok(Some(value)),
        Some(_) => bail!("existing GitMeta key '{key}' is not a string"),
    }
}

fn session_handle(request: &ProjectionRequest, session_key: &str) -> String {
    opaque_handle(
        "ps",
        &[
            &request.pr.github_repository_id.to_string(),
            &request.pr.pull_request.to_string(),
            &request.snapshot.metadata_oid,
            session_key,
        ],
    )
}

fn turn_handle(request: &ProjectionRequest, session_key: &str, turn_key: &str) -> String {
    opaque_handle(
        "pt",
        &[
            &request.pr.github_repository_id.to_string(),
            &request.pr.pull_request.to_string(),
            &request.snapshot.metadata_oid,
            session_key,
            turn_key,
        ],
    )
}

fn record_handle(
    request: &ProjectionRequest,
    session_key: &str,
    turn_key: &str,
    record_hash: &str,
) -> String {
    opaque_handle(
        "pr",
        &[
            &request.pr.github_repository_id.to_string(),
            &request.pr.pull_request.to_string(),
            &request.snapshot.metadata_oid,
            session_key,
            turn_key,
            record_hash,
        ],
    )
}

fn opaque_handle(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"\0");
    }
    let digest = hasher.finalize();
    format!("{prefix}_{}", hex::encode(&digest[..16]))
}

fn selected_evidence_digest(handles: &[String]) -> String {
    let mut hasher = Sha256::new();
    for handle in handles {
        hasher.update(handle.as_bytes());
        hasher.update(b"\0");
    }
    format!("sha256-{}", hex::encode(hasher.finalize()))
}

fn normalize_branch_target_key(value: &str) -> String {
    let value = value.strip_prefix("branch:").unwrap_or(value);
    let value = value.strip_prefix("ref:").unwrap_or(value);
    let value = value.strip_prefix("refs/heads/").unwrap_or(value);
    format!("ref:refs/heads/{value}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection_request_with_text_limit(max_text_chars: usize) -> ProjectionRequest {
        let mut request = ProjectionRequest::new(
            ProjectionPrFacts {
                agent_trail_host: "https://agent-trail.test".to_owned(),
                github_repository_id: 42,
                owner: "gitbutler".to_owned(),
                repo: "gitbutler".to_owned(),
                pull_request: 1,
                base_ref: "main".to_owned(),
                head_ref: "main".to_owned(),
                head_sha: "0123456789abcdef".to_owned(),
            },
            ProjectionSnapshotInput {
                metadata_oid: "sha256-test-metadata".to_owned(),
                projection_version: 1,
                generated_at_unix_seconds: 1_779_999_999,
            },
        );
        request.limits.max_text_chars = max_text_chars;
        request
    }

    #[test]
    fn public_text_redacts_absolute_paths_before_emitting() {
        let request = projection_request_with_text_limit(200);
        let kind = Some("tool_result".to_owned());

        let (text, truncated) = public_text(
            &kind,
            Some(
                "Chunk ID: abc\nUpdated /Users/alice/src/project/src/lib.rs and /home/alice/.ssh/id_ed25519\nProcess exited with code 0",
            ),
            &request,
        );

        assert!(!truncated, "redacted text should fit within the test limit");
        let text = text.expect("public text");
        assert_eq!(text, "Updated [REDACTED:path] and [REDACTED:path]");
        assert!(
            !text.contains("/Users/") && !text.contains("/home/"),
            "public projection text must not expose home-prefixed absolute paths"
        );
    }
}

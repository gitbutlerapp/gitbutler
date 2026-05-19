//! Unity scene and prefab semantic review APIs.

use std::{path::PathBuf, process::Command};

use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_ctx::Context;
use but_unity_yaml::{
    UnityLineRange, UnitySemanticChange, UnitySemanticDiff, UnitySemanticNode,
    UnitySemanticSelectionRange,
};
use tracing::instrument;

const UNITY_SEMANTIC_MAX_CHANGED_LINES: u32 = 1_000;
const UNITY_SEMANTIC_MAX_FILE_BYTES: u64 = 1_000_000;

/// API types for Unity semantic review.
pub mod json {
    use but_unity_yaml::{
        UnityChangeKind, UnityFileKind, UnityNodeKind, UnitySemanticSummary, UnitySemanticWarning,
    };
    use serde::{Deserialize, Serialize};

    /// A 1-based line selected inside a diff hunk.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnityLineId {
        /// Old-file line number, when the selected diff row has one.
        pub old_line: Option<u32>,
        /// New-file line number, when the selected diff row has one.
        pub new_line: Option<u32>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnityLineId);

    /// A hunk that a semantic Unity row can select.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnitySelectableHunk {
        /// Previous file start line.
        pub old_start: u32,
        /// Previous file line count.
        pub old_lines: u32,
        /// Current file start line.
        pub new_start: u32,
        /// Current file line count.
        pub new_lines: u32,
        /// Exact selected lines when available. Empty means select the whole hunk.
        pub lines: Vec<UnityLineId>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySelectableHunk);

    /// How safely a semantic Unity row maps to GitButler's existing hunk selection model.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub enum UnitySelectionMode {
        /// Exact line-level mapping is available.
        Precise,
        /// Only whole-hunk mapping is available.
        Hunk,
        /// The whole file should be selected.
        File,
        /// The row is informational only.
        Unavailable,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySelectionMode);

    /// Selection metadata for a semantic Unity row.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnitySelection {
        /// Selection precision.
        pub mode: UnitySelectionMode,
        /// Hunk mappings for hunk/precise selection.
        pub hunks: Vec<UnitySelectableHunk>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySelection);

    /// A Unity semantic property change enriched with selection data.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnitySemanticChangeForFrontend {
        /// Human-readable change label.
        pub label: String,
        /// Unity serialized property path.
        pub property_path: String,
        /// Previous value, if any.
        pub old_value: Option<String>,
        /// Current value, if any.
        pub new_value: Option<String>,
        /// Coarse change kind.
        pub change_kind: UnityChangeKind,
        /// Selection metadata.
        pub selection: UnitySelection,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySemanticChangeForFrontend);

    /// A Unity semantic node enriched with selection data.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnitySemanticNodeForFrontend {
        /// Stable node id.
        pub id: String,
        /// Human-readable label.
        pub label: String,
        /// Node kind.
        pub kind: UnityNodeKind,
        /// Coarse change kind.
        pub change_kind: UnityChangeKind,
        /// Human-readable hierarchy path.
        pub path: String,
        /// Unity class name when known.
        pub class_name: Option<String>,
        /// Child nodes.
        pub children: Vec<UnitySemanticNodeForFrontend>,
        /// Property changes.
        pub changes: Vec<UnitySemanticChangeForFrontend>,
        /// Selection metadata.
        pub selection: UnitySelection,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySemanticNodeForFrontend);

    /// A Unity semantic diff enriched for frontend selection behavior.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnitySemanticDiffForFrontend {
        /// File kind.
        pub file_kind: UnityFileKind,
        /// Summary counts.
        pub summary: UnitySemanticSummary,
        /// Top-level semantic nodes.
        pub nodes: Vec<UnitySemanticNodeForFrontend>,
        /// Parser warnings.
        pub warnings: Vec<UnitySemanticWarning>,
        /// Whether a raw diff is available as fallback.
        pub raw_available: bool,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySemanticDiffForFrontend);

    /// Availability information for Unity Smart Merge.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnitySmartMergeStatus {
        /// Whether GitButler can attempt Unity Smart Merge.
        pub available: bool,
        /// Human-readable command/tool description.
        pub command: Option<String>,
        /// Message explaining availability or missing setup.
        pub message: String,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySmartMergeStatus);

    /// Result of a user-triggered Unity Smart Merge run.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct UnitySmartMergeOutcome {
        /// Whether the command exited successfully.
        pub success: bool,
        /// Whether conflict markers remain after running.
        pub unresolved_markers_remaining: bool,
        /// Whether the file content changed.
        pub file_changed: bool,
        /// Process stdout summary.
        pub stdout: String,
        /// Process stderr summary.
        pub stderr: String,
        /// Human-readable outcome message.
        pub message: String,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UnitySmartMergeOutcome);
}

/// Return a semantic Unity scene/prefab diff for supported files.
#[but_api(napi, json::UnitySemanticDiffForFrontend)]
#[instrument(err(Debug))]
pub fn unity_semantic_diff(
    ctx: &Context,
    change: but_core::ui::TreeChange,
) -> Result<Option<UnitySemanticDiff>> {
    let path = change.path.to_string();
    if !but_unity_yaml::is_supported_path(&path) {
        return Ok(None);
    }

    let repo = ctx.repo.get()?;
    let core_change: but_core::TreeChange = change.into();
    if let Some(diff) = too_large_unity_file(&repo, &path, &core_change)? {
        CURRENT_PATCH.with(|current| {
            *current.borrow_mut() = None;
        });
        return Ok(Some(diff));
    }

    let patch = core_change.unified_patch(&repo, ctx.settings.context_lines)?;
    let raw_available = patch.is_some();
    if let Some(diff) = too_large_unity_diff(&path, patch.as_ref()) {
        CURRENT_PATCH.with(|current| {
            *current.borrow_mut() = patch;
        });
        return Ok(Some(diff));
    }

    let previous = previous_content(&repo, &core_change)?;
    let current = current_content(&repo, &core_change)?;

    let diff = but_unity_yaml::semantic_diff(
        &path,
        previous.as_deref(),
        current.as_deref(),
        raw_available,
    );
    CURRENT_PATCH.with(|current| {
        *current.borrow_mut() = patch;
    });
    Ok(diff)
}

fn too_large_unity_file(
    repo: &gix::Repository,
    path: &str,
    change: &but_core::TreeChange,
) -> Result<Option<UnitySemanticDiff>> {
    let file_kind = but_unity_yaml::file_kind(path);
    let Some(file_kind) = file_kind else {
        return Ok(None);
    };
    let file_size = largest_change_size(repo, path, change)?;
    Ok((file_size > UNITY_SEMANTIC_MAX_FILE_BYTES).then(|| {
        too_large_diff(
            file_kind,
            format!(
                "This Unity file is too large to analyze safely ({file_size} bytes). Use Raw diff when you need to inspect it."
            ),
        )
    }))
}

fn too_large_unity_diff(
    path: &str,
    patch: Option<&but_core::UnifiedPatch>,
) -> Option<UnitySemanticDiff> {
    let file_kind = but_unity_yaml::file_kind(path)?;
    let changed_lines = match patch? {
        but_core::UnifiedPatch::Patch {
            lines_added,
            lines_removed,
            ..
        } => lines_added.saturating_add(*lines_removed),
        but_core::UnifiedPatch::TooLarge { .. } => UNITY_SEMANTIC_MAX_CHANGED_LINES + 1,
        but_core::UnifiedPatch::Binary => return None,
    };

    (changed_lines > UNITY_SEMANTIC_MAX_CHANGED_LINES).then(|| {
        too_large_diff(
            file_kind,
            format!(
                "This Unity diff is too long to render safely ({changed_lines} changed lines). Use Raw diff when you need to inspect it."
            ),
        )
    })
}

fn too_large_diff(file_kind: but_unity_yaml::UnityFileKind, message: String) -> UnitySemanticDiff {
    UnitySemanticDiff {
        file_kind,
        summary: but_unity_yaml::UnitySemanticSummary {
            warnings: 1,
            ..but_unity_yaml::UnitySemanticSummary::default()
        },
        nodes: Vec::new(),
        warnings: vec![but_unity_yaml::UnitySemanticWarning {
            message,
            line: None,
        }],
        raw_available: true,
    }
}

fn largest_change_size(
    repo: &gix::Repository,
    path: &str,
    change: &but_core::TreeChange,
) -> Result<u64> {
    let size = match &change.status {
        but_core::TreeStatus::Addition { state, .. } => change_state_size(repo, path, *state)?,
        but_core::TreeStatus::Deletion { previous_state } => {
            change_state_size(repo, path, *previous_state)?
        }
        but_core::TreeStatus::Modification {
            previous_state,
            state,
            ..
        }
        | but_core::TreeStatus::Rename {
            previous_state,
            state,
            ..
        } => change_state_size(repo, path, *previous_state)?
            .max(change_state_size(repo, path, *state)?),
    };
    Ok(size)
}

fn change_state_size(
    repo: &gix::Repository,
    path: &str,
    state: but_core::ChangeState,
) -> Result<u64> {
    if !state.id.is_null() {
        return Ok(repo.find_header(state.id)?.size());
    }

    let Some(workdir) = repo.workdir() else {
        return Ok(0);
    };
    let metadata = std::fs::metadata(workdir.join(path)).with_context(|| {
        format!(
            "Failed to read metadata for {}",
            workdir.join(path).display()
        )
    })?;
    Ok(metadata.len())
}

/// Return the current Unity Smart Merge availability for a project.
#[but_api(napi, json::UnitySmartMergeStatus)]
#[instrument(err(Debug))]
pub fn unity_smart_merge_preview(
    ctx: &Context,
    _path: PathBuf,
) -> Result<json::UnitySmartMergeStatus> {
    let repo = ctx.repo.get()?;
    Ok(smart_merge_status(&repo))
}

/// Run Unity Smart Merge for a conflicted Unity path.
#[but_api(napi, json::UnitySmartMergeOutcome)]
#[instrument(err(Debug))]
pub fn run_unity_smart_merge(ctx: &Context, path: PathBuf) -> Result<json::UnitySmartMergeOutcome> {
    let repo = ctx.repo.get()?;
    let workdir = repo
        .workdir()
        .context("Unity Smart Merge requires a worktree")?;
    let before = std::fs::read(workdir.join(&path)).unwrap_or_default();

    let output = Command::new("git")
        .arg("mergetool")
        .arg("--tool=unityyamlmerge")
        .arg("--no-prompt")
        .arg("--")
        .arg(&path)
        .current_dir(workdir)
        .output()
        .context("Failed to run git mergetool for Unity Smart Merge")?;

    let after = std::fs::read(workdir.join(&path)).unwrap_or_default();
    let unresolved_markers_remaining = String::from_utf8_lossy(&after).contains("<<<<<<<");
    let success = output.status.success() && !unresolved_markers_remaining;
    let stdout = truncate_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = truncate_output(&String::from_utf8_lossy(&output.stderr));

    Ok(json::UnitySmartMergeOutcome {
        success,
        unresolved_markers_remaining,
        file_changed: before != after,
        stdout,
        stderr,
        message: if success {
            "Unity Smart Merge completed.".to_owned()
        } else if unresolved_markers_remaining {
            "Unity Smart Merge ran, but conflict markers remain.".to_owned()
        } else {
            "Unity Smart Merge did not complete successfully.".to_owned()
        },
    })
}

fn previous_content(
    repo: &gix::Repository,
    change: &but_core::TreeChange,
) -> Result<Option<String>> {
    match &change.status {
        but_core::TreeStatus::Addition { .. } => Ok(None),
        but_core::TreeStatus::Deletion { previous_state }
        | but_core::TreeStatus::Modification { previous_state, .. }
        | but_core::TreeStatus::Rename { previous_state, .. } => {
            content_from_state(repo, *previous_state)
        }
    }
}

fn current_content(
    repo: &gix::Repository,
    change: &but_core::TreeChange,
) -> Result<Option<String>> {
    match &change.status {
        but_core::TreeStatus::Deletion { .. } => Ok(None),
        but_core::TreeStatus::Addition { state, .. }
        | but_core::TreeStatus::Modification { state, .. }
        | but_core::TreeStatus::Rename { state, .. } => content_from_state(repo, *state),
    }
}

fn content_from_state(
    repo: &gix::Repository,
    state: but_core::ChangeState,
) -> Result<Option<String>> {
    if !state.id.is_null() {
        let blob = repo.find_blob(state.id)?;
        return Ok(String::from_utf8(blob.detach().data).ok());
    }
    Ok(None)
}

fn smart_merge_status(repo: &gix::Repository) -> json::UnitySmartMergeStatus {
    let configured = repo
        .config_snapshot()
        .string("mergetool.unityyamlmerge.cmd")
        .map(|cmd| cmd.to_string());
    if let Some(command) = configured {
        return json::UnitySmartMergeStatus {
            available: true,
            command: Some(command),
            message: "Unity Smart Merge is configured as a Git mergetool.".to_owned(),
        };
    }

    if let Ok(path) = which::which("UnityYAMLMerge") {
        return json::UnitySmartMergeStatus {
            available: true,
            command: Some(path.display().to_string()),
            message: "UnityYAMLMerge is available on PATH.".to_owned(),
        };
    }

    json::UnitySmartMergeStatus {
        available: false,
        command: None,
        message: "UnityYAMLMerge is not configured on this machine.".to_owned(),
    }
}

fn truncate_output(output: &str) -> String {
    const LIMIT: usize = 4_000;
    if output.len() <= LIMIT {
        output.to_owned()
    } else {
        format!("{}...", &output[..LIMIT])
    }
}

impl From<UnitySemanticDiff> for json::UnitySemanticDiffForFrontend {
    fn from(diff: UnitySemanticDiff) -> Self {
        let patch = CURRENT_PATCH.with(|patch| patch.borrow().clone());
        json::UnitySemanticDiffForFrontend {
            file_kind: diff.file_kind,
            summary: diff.summary,
            nodes: diff
                .nodes
                .into_iter()
                .map(|node| convert_node(node, patch.as_ref()))
                .collect(),
            warnings: diff.warnings,
            raw_available: diff.raw_available,
        }
    }
}

thread_local! {
    static CURRENT_PATCH: std::cell::RefCell<Option<but_core::UnifiedPatch>> = const { std::cell::RefCell::new(None) };
}

fn convert_node(
    node: UnitySemanticNode,
    patch: Option<&but_core::UnifiedPatch>,
) -> json::UnitySemanticNodeForFrontend {
    json::UnitySemanticNodeForFrontend {
        id: node.id,
        label: node.label,
        kind: node.kind,
        change_kind: node.change_kind,
        path: node.path,
        class_name: node.class_name,
        selection: selection_from_ranges(Some(node.range), patch),
        changes: node
            .changes
            .into_iter()
            .map(|change| convert_change(change, patch))
            .collect(),
        children: node
            .children
            .into_iter()
            .map(|child| convert_node(child, patch))
            .collect(),
    }
}

fn convert_change(
    change: UnitySemanticChange,
    patch: Option<&but_core::UnifiedPatch>,
) -> json::UnitySemanticChangeForFrontend {
    json::UnitySemanticChangeForFrontend {
        label: change.label,
        property_path: change.property_path,
        old_value: change.old_value,
        new_value: change.new_value,
        change_kind: change.change_kind,
        selection: selection_from_ranges(Some(change.range), patch),
    }
}

fn selection_from_ranges(
    range: Option<UnitySemanticSelectionRange>,
    patch: Option<&but_core::UnifiedPatch>,
) -> json::UnitySelection {
    let Some(range) = range else {
        return unavailable_selection();
    };
    let Some(but_core::UnifiedPatch::Patch {
        hunks,
        is_result_of_binary_to_text_conversion: false,
        ..
    }) = patch
    else {
        return json::UnitySelection {
            mode: json::UnitySelectionMode::File,
            hunks: Vec::new(),
        };
    };

    let mut selectable = Vec::new();
    let mut precise = true;
    for hunk in hunks {
        if !range_overlaps_hunk(&range, hunk) {
            continue;
        }
        let lines = precise_lines(&range, hunk);
        if lines.is_empty() {
            precise = false;
        }
        selectable.push(json::UnitySelectableHunk {
            old_start: hunk.old_start,
            old_lines: hunk.old_lines,
            new_start: hunk.new_start,
            new_lines: hunk.new_lines,
            lines,
        });
    }

    if selectable.is_empty() {
        return unavailable_selection();
    }

    json::UnitySelection {
        mode: if precise {
            json::UnitySelectionMode::Precise
        } else {
            json::UnitySelectionMode::Hunk
        },
        hunks: selectable,
    }
}

fn unavailable_selection() -> json::UnitySelection {
    json::UnitySelection {
        mode: json::UnitySelectionMode::Unavailable,
        hunks: Vec::new(),
    }
}

fn range_overlaps_hunk(
    range: &UnitySemanticSelectionRange,
    hunk: &but_core::unified_diff::DiffHunk,
) -> bool {
    range.old.is_some_and(|line_range| {
        ranges_overlap(
            line_range.start,
            line_range.end,
            hunk.old_start,
            hunk.old_start
                .saturating_add(hunk.old_lines.saturating_sub(1)),
        )
    }) || range.new.is_some_and(|line_range| {
        ranges_overlap(
            line_range.start,
            line_range.end,
            hunk.new_start,
            hunk.new_start
                .saturating_add(hunk.new_lines.saturating_sub(1)),
        )
    })
}

fn precise_lines(
    range: &UnitySemanticSelectionRange,
    hunk: &but_core::unified_diff::DiffHunk,
) -> Vec<json::UnityLineId> {
    let old_lines = lines_in_hunk(range.old, hunk.old_start, hunk.old_lines);
    let new_lines = lines_in_hunk(range.new, hunk.new_start, hunk.new_lines);
    let max_len = old_lines.len().max(new_lines.len());
    if max_len == 0 || max_len > 3 {
        return Vec::new();
    }

    (0..max_len)
        .map(|index| json::UnityLineId {
            old_line: old_lines.get(index).copied(),
            new_line: new_lines.get(index).copied(),
        })
        .collect()
}

fn lines_in_hunk(range: Option<UnityLineRange>, start: u32, len: u32) -> Vec<u32> {
    let Some(range) = range else {
        return Vec::new();
    };
    let end = start.saturating_add(len.saturating_sub(1));
    (range.start.max(start)..=range.end.min(end)).collect()
}

fn ranges_overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start <= b_end && b_start <= a_end
}

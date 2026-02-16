use std::path::Path;

use bstr::BString;
use but_core::sync::RepoExclusiveGuard;
use but_hunk_assignment::{CommitMap, convert_assignments_to_diff_specs};
use but_workspace::commit_engine;
use gitbutler_project::ProjectId;
use serde::Serialize;

type AutoCommitEmitter = dyn Fn(&str, serde_json::Value) + Send + Sync + 'static;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "camelCase")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./action/autoCommit.ts"))]
enum AutoCommitEvent {
    /// Emitted when the auto-commit process has started.
    ///
    /// `steps_length`: The total number of steps in the auto-commit process.
    Started { steps_length: usize },
    /// Emitted when a commit message is being generated.
    ///
    /// `parent_commit_id`: The ID of the parent commit for which the message is being generated.
    /// `token`: A token representing the progress of the commit message generation.
    CommitGeneration {
        #[cfg_attr(feature = "export-ts", ts(type = "string"))]
        #[serde(with = "but_serde::object_id")]
        parent_commit_id: gix::ObjectId,
        token: String,
    },
    /// Emitted when a commit has been successfully created.
    ///
    /// `commit_id`: The ID of the newly created commit.
    CommitSuccess {
        #[cfg_attr(feature = "export-ts", ts(type = "string"))]
        #[serde(with = "but_serde::object_id")]
        commit_id: gix::ObjectId,
    },
    /// Emitted when an error occurs during the auto-commit process.
    ///
    /// `error_message`: A message describing the error.
    CommitError { error_message: String },
    /// Emitted when the auto-commit process has completed.
    Completed,
}

impl AutoCommitEvent {
    fn event_name(&self, project_id: ProjectId) -> String {
        format!("project://{project_id}/auto-commit")
    }

    fn emit_payload(&self) -> serde_json::Value {
        serde_json::to_value(self)
            .unwrap_or_else(|e| serde_json::json!({"error": format!("Failed to serialize event payload: {}", e)}))
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn auto_commit(
    project_id: ProjectId,
    repo: &gix::Repository,
    project_data_dir: &Path,
    context_lines: u32,
    llm: Option<&but_llm::LLMProvider>,
    emitter: impl Fn(&str, serde_json::Value) + Send + Sync + 'static,
    absorption_plan: Vec<but_hunk_assignment::CommitAbsorption>,
    guard: &mut RepoExclusiveGuard,
) -> anyhow::Result<usize> {
    let commit_map = CommitMap::default();

    let emitter: std::sync::Arc<AutoCommitEmitter> = std::sync::Arc::new(emitter);

    // Emit the started event
    let event = AutoCommitEvent::Started {
        steps_length: absorption_plan.len(),
    };
    let event_name = event.event_name(project_id);
    emitter(&event_name, event.emit_payload());

    match apply_commit_changes(
        Some(project_id),
        repo,
        project_data_dir,
        context_lines,
        llm,
        absorption_plan,
        guard,
        commit_map,
        Some(emitter.clone()),
    ) {
        Err(e) => {
            tracing::error!("Auto-commit failed: {}", e);
            let event = AutoCommitEvent::CommitError {
                error_message: e.to_string(),
            };
            let event_name = event.event_name(project_id);
            let emitter = emitter.clone();
            emitter(&event_name, event.emit_payload());
            Err(e)
        }
        Ok(number_of_rejections) => {
            let event = AutoCommitEvent::Completed;
            let event_name = event.event_name(project_id);
            let emitter = emitter.clone();
            emitter(&event_name, event.emit_payload());
            Ok(number_of_rejections)
        }
    }
}

pub(crate) fn auto_commit_simple(
    repo: &gix::Repository,
    project_data_dir: &Path,
    context_lines: u32,
    llm: Option<&but_llm::LLMProvider>,
    absorption_plan: Vec<but_hunk_assignment::CommitAbsorption>,
    guard: &mut RepoExclusiveGuard,
) -> anyhow::Result<usize> {
    let commit_map = CommitMap::default();

    apply_commit_changes(
        None,
        repo,
        project_data_dir,
        context_lines,
        llm,
        absorption_plan,
        guard,
        commit_map,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_commit_changes(
    project_id: Option<ProjectId>,
    repo: &gix::Repository,
    project_data_dir: &Path,
    context_lines: u32,
    llm: Option<&but_llm::LLMProvider>,
    absorption_plan: Vec<but_hunk_assignment::CommitAbsorption>,
    guard: &mut RepoExclusiveGuard,
    mut commit_map: CommitMap,
    emitter: Option<std::sync::Arc<AutoCommitEmitter>>,
) -> anyhow::Result<usize> {
    let mut total_rejected = 0;
    for absorption in absorption_plan {
        let diff_specs = convert_assignments_to_diff_specs(
            &absorption
                .files
                .iter()
                .map(|f| f.assignment.clone())
                .collect::<Vec<_>>(),
        )?;
        let diff_infos = absorption_files_to_diff_infos(&absorption.files);
        let commit_id = commit_map.find_mapped_id(absorption.commit_id);
        let stack_id = absorption.stack_id;
        let commit_message = commit_message_generation(project_id, commit_id, llm, emitter.as_ref(), &diff_infos)?;
        let outcome = but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
            repo,
            project_data_dir,
            Some(stack_id),
            commit_engine::Destination::NewCommit {
                message: commit_message,
                parent_commit_id: Some(commit_id),
                stack_segment: None,
            },
            diff_specs,
            context_lines,
            guard.write_permission(),
        )?;

        if let Some(new_commit_id) = outcome.new_commit
            && let Some(project_id) = project_id
            && let Some(emitter) = &emitter
        {
            let event = AutoCommitEvent::CommitSuccess {
                commit_id: new_commit_id,
            };
            let event_name = event.event_name(project_id);
            emitter(&event_name, event.emit_payload());
        }

        if let Some(rebase_output) = &outcome.rebase_output {
            for mapping in &rebase_output.commit_mapping {
                commit_map.add_mapping(mapping.1, mapping.2);
            }
        }

        total_rejected += outcome.rejected_specs.len();
    }

    Ok(total_rejected)
}

#[derive(Debug, Clone)]
struct DiffInfo {
    /// The file path of the diff.
    path: String,
    /// The diff content.
    diff: BString,
}

impl std::fmt::Display for DiffInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "file: {}\n{}", self.path, self.diff)
    }
}

fn absorption_files_to_diff_infos(absorption_files: &[but_hunk_assignment::FileAbsorption]) -> Vec<DiffInfo> {
    absorption_files
        .iter()
        .filter_map(|f| {
            f.assignment.diff.as_ref().map(|diff| DiffInfo {
                path: f.path.clone(),
                diff: diff.clone(),
            })
        })
        .collect()
}

/// Generate a commit message using the LLM provider, if available.
///
/// If no project and no emitter are provided, the function will not stream tokens.
fn commit_message_generation(
    project_id: Option<ProjectId>,
    parent_commit_id: gix::ObjectId,
    llm: Option<&but_llm::LLMProvider>,
    emitter: Option<&std::sync::Arc<AutoCommitEmitter>>,
    hunk_diffs: &[DiffInfo],
) -> anyhow::Result<String> {
    if let Some(llm) = llm
        && let Some(model) = llm.model()
    {
        let system_message = "
<tone>
    You are an expert git commit message generator.
    Your task is to create clear, concise, and descriptive commit messages (title and body) based on the provided diffs.
    The response is intended to be used directly as a git commit message.
</tone>

<instructions>
    - Summarize the changes made in the diff.
    - Use imperative mood (e.g., 'Fix bug', 'Add feature').
    - Generate a title and a body if necessary.
    - Keep the message title concise, ideally under 50 characters for the subject line.
    - The body should provide additional context, but not be overly verbose.
    - If multiple changes are present, provide a brief overview.
</instructions>

<format>
    - Return only the commit message text without any additional formatting or explanations.
    - Ensure the title and body are separated by a blank line.
</format>
    ";

        let changes = hunk_diffs.iter().map(|diff| diff.to_string()).collect::<Vec<_>>();

        let prompt = format!(
            "Please generate a concise and descriptive git commit message for the following changes:\n\n{}",
            changes.join("\n")
        );

        let commit_message = match (project_id, emitter) {
            (Some(project_id), Some(emitter)) => llm.stream_response(system_message, vec![prompt.into()], &model, {
                let emitter = std::sync::Arc::clone(emitter);
                move |token| {
                    let event = AutoCommitEvent::CommitGeneration {
                        parent_commit_id,
                        token: token.to_string(),
                    };
                    let event_name = event.event_name(project_id);
                    emitter(&event_name, event.emit_payload());
                }
            })?,
            _ => llm.response(system_message, vec![prompt.into()], &model)?,
        };

        if let Some(message) = commit_message {
            return Ok(message);
        }
    }

    Ok("[AUTO-COMMIT] Generated commit message".to_string())
}

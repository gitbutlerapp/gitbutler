use anyhow::Context as _;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use but_tools::{
    emit::Emitter,
    workspace::{FileChange, SimpleCommit, amend_toolset},
};
use gix::hashtable::hash_map::HashMap;

use crate::OpenAiProvider;

/// Absorb file changes into existing commits in the project.
///
/// This function will first try to get rid of the changes that are locked to one specific commit.
/// After that, the tool calling loop will be used.
///
/// TODO: This implementation has some clear disadvantage:
/// - It requires the agent to get the project status after every amendment.
///   Why? Because we need to know the current state of the project (specifically, the commit IDs)
///   to determine where to put the changes.
/// - File changes that are locked to multiple commits are not absorbed.
///
/// The next iteration should improve this by:
/// - Absorbing changes at a line level, rather than file level.
/// - Reading the commit outcome, that contains the mapping of old to new commit IDs.
///   Being able to read this, eliminates the need to get the project status after every amendment.
///
pub(crate) fn absorb(
    emitter: std::sync::Arc<Emitter>,
    ctx: &mut Context,
    openai: &OpenAiProvider,
    changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<()> {
    let paths = changes
        .iter()
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    let path_strings = paths.iter().map(|p| p.to_string()).collect::<Vec<String>>();
    let path_strings = path_strings.join("\n");

    let start = std::time::Instant::now();
    let project_status = but_tools::workspace::get_project_status(ctx, Some(paths.clone()))?;
    tracing::info!("get_project_status took {:?}", start.elapsed());

    // First, absorb changes that are already locked to a specific commit.
    absorb_locked_changes(emitter.clone(), ctx, &project_status)
        .context("Failed to absorb locked changes")?;

    // After absorbing locked changes, we need to get the project status again,
    // because the commit IDs might have changed.
    let project_status = but_tools::workspace::get_project_status(ctx, Some(paths))
        .context("Failed to get project status after absorbing locked changes")?;

    if project_status.file_changes.is_empty() {
        // If there are no file changes left, we are done.
        return Ok(());
    }

    let serialized_status = serde_json::to_string_pretty(&project_status)
        .context("Failed to serialize project status")?;

    let mut toolset = amend_toolset(ctx, emitter);

    let system_message ="
       You are an expert in finding where to put file changes in a project.
        When given the status of a project, you should be able to identify the commits in which the changes should be amended.
        ";

    let prompt = format!("
        Please, figure out how to absorb the file changes into the existing commits in the project.
        Follow these steps:
        1. Take a look at the existing commits and the file changes.
        2. Determine which files belong to which commit.
        3. Amend a file (or set of files) into their corresponding commit.
        4. Get the project status again and repeat the process until all changes are absorbed.

        ### Determining where to put changes:
        In order to determine where to put the changes do this in order:
        1. Take a look at the existing dependency locks. If there are any locks pointing to a commit ID, that is the commit where the changes should be absorbed.
        2. If there are no locks, look at the assignments. If there are any assignments pointing to a stack ID, that is the stack where the changes should be absorbed.
           Already knowing the stack ID, look at the commit messages inside the stack branches and try to find the commit that is related to the changes.
        3. If there are no assignments, look at the descriptions of the branches and commit messages. Try to find the branch and commit that most closely matches the changes.
        4. If there are no branch or commits that match the change, don't do anything. The changes will be left unabsorbed.

        <important_note>
            Only absorb changes specified by the user
        </important_note>

        Here are the file changes to absorb:
        <file_changes>
                {path_strings}
        </file_changes>

        Here is the project status:
        <project_status>
                {serialized_status}
        </project_status>
    ");

    // Now we trigger the tool calling loop to absorb the remaining changes.
    crate::openai::tool_calling_loop(
        openai,
        system_message,
        vec![prompt.into()],
        &mut toolset,
        None,
    )?;

    Ok(())
}

struct AbsorbGroup {
    commit_id: gix::ObjectId,
    stack_id: StackId,
    files: Vec<FileChange>,
}

/// Absorb the changes that are already locked to a specific commit.
///
/// In this iteration, we will only amend complete file changes that are locked to exactly one commit.
///
/// This doesn't account for:
/// - Changes that not locked to anything.
/// - Changes that are locked to multiple commits (i.e. Different hunks are are locked to different commits).
fn absorb_locked_changes(
    emitter: std::sync::Arc<Emitter>,
    ctx: &mut Context,
    project_status: &but_tools::workspace::ProjectStatus,
) -> anyhow::Result<()> {
    let mut absorb_groups: std::collections::HashMap<gix::ObjectId, AbsorbGroup> =
        std::collections::HashMap::new();

    for change in &project_status.file_changes {
        let mut hunk_lock: Option<but_hunk_dependency::ui::HunkLock> = None;
        for hunks in change.hunks.iter() {
            if hunks.dependency_locks.len() > 1 {
                // Ignore files that have hunks locked to multiple commits.
                // Later iterations should amend changes at a line level.
                break;
            }

            if hunks.dependency_locks.is_empty() {
                // If there are no locks in this hunk, we skip it.
                continue;
            }

            if let Some(lock) = hunks.dependency_locks.first() {
                if hunk_lock.is_none() {
                    // If this is the first lock we find, we set it.
                    hunk_lock = Some(*lock);
                    continue;
                }

                if hunk_lock != Some(*lock) {
                    // If there is another lock, we skip this file completely
                    break;
                }
            }
        }

        if let Some(hunk_lock) = hunk_lock {
            // File is locked to a single commit, add it to the absorb group.
            let group = absorb_groups
                .entry(hunk_lock.commit_id)
                .or_insert_with(|| AbsorbGroup {
                    commit_id: hunk_lock.commit_id,
                    stack_id: hunk_lock.stack_id,
                    files: Vec::new(),
                });

            group.files.push(change.clone());
        }
    }

    if absorb_groups.is_empty() {
        // No locked changes to absorb.
        return Ok(());
    }

    let mut commit_id_map = HashMap::new();

    for group in absorb_groups.values() {
        let AbsorbGroup {
            commit_id,
            stack_id,
            files,
        } = group;

        let commit_id = find_mapped_commit_id(commit_id, &commit_id_map);

        let stack = project_status
            .stacks
            .iter()
            .find(|s| s.id == *stack_id)
            .context("Stack not found in project status")?;

        let commit = stack
            .branches
            .iter()
            .flat_map(|b| b.commits.iter())
            .find(|c| c.id == commit_id)
            .context("Commit not found in project status")?;

        // Absorb the file changes into the commit.
        let outcome = absorb_file_changes_into_commit(
            emitter.clone(),
            ctx,
            stack_id,
            &commit_id,
            commit,
            files,
        )
        .context(format!(
            "Failed to absorb changes into commit {commit_id} in stack {stack_id}"
        ))?;

        if let Some(rebase_output) = outcome.rebase_output {
            rebase_output
                .commit_mapping
                .iter()
                .for_each(|(_, old_id, new_id)| {
                    commit_id_map.insert(old_id.to_owned(), new_id.to_owned());
                });
        }
    }

    Ok(())
}

fn find_mapped_commit_id(
    commit_id: &gix::ObjectId,
    commit_id_map: &HashMap<gix::ObjectId, gix::ObjectId>,
) -> gix::ObjectId {
    let mut current_id = commit_id.to_owned();
    while commit_id_map.contains_key(&current_id) {
        if let Some(new_id) = commit_id_map.get(&current_id) {
            current_id = new_id.to_owned();
        } else {
            break; // No mapping found, exit the loop
        }
    }
    current_id
}

/// Absorb file changes into an existing commit.
///
/// This function uses OpenAI to generate the commit message and amend the file changes into the commit.
/// TODO: Do we need to recompute the commit message?
fn absorb_file_changes_into_commit(
    emitter: std::sync::Arc<Emitter>,
    ctx: &mut Context,
    stack_id: &StackId,
    commit_id: &gix::ObjectId,
    commit: &SimpleCommit,
    files: &[FileChange],
) -> anyhow::Result<but_workspace::commit_engine::CreateCommitOutcome> {
    let outcome = but_tools::workspace::amend_commit_inner(
        ctx,
        emitter,
        but_tools::workspace::AmendParameters {
            commit_id: commit_id.to_owned().to_git2().to_string(),
            stack_id: stack_id.to_owned().to_string(),
            message_title: commit.message_title.clone(),
            message_body: commit.message_body.clone(),
            files: files.iter().map(|f| f.to_owned().path).collect(),
        },
        None,
    )?;

    Ok(outcome)
}

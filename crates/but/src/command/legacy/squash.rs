//! Implementation of the `but squash` command.
//!
//! This command squashes two commits together and uses AI to generate a combined
//! commit message from both original messages.

use anyhow::{Result, bail};
use but_action::OpenAiProvider;
use but_ctx::Context;
use but_oxidize::ObjectIdExt;
use colored::Colorize;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gix::ObjectId;

use crate::{CliId, IdMap, utils::OutputChannel};

/// Handle the squash command by squashing two commits and generating an AI commit message.
pub(crate) async fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit1_str: &str,
    commit2_str: &str,
) -> Result<()> {
    let mut id_map = IdMap::new_from_context(ctx)?;
    id_map.add_file_info_from_context(ctx)?;

    // Resolve both commit IDs
    let commit1_id = resolve_commit_id(&id_map, commit1_str)?;
    let commit2_id = resolve_commit_id(&id_map, commit2_str)?;

    // Verify that one is an ancestor of the other
    let (source, destination) = determine_parent_child(ctx, &commit1_id, &commit2_id)?;

    // Get the commit messages before squashing
    let source_message = get_commit_message(ctx, &source)?;
    let destination_message = get_commit_message(ctx, &destination)?;

    // Find which stack this commit belongs to
    let stack_id = stack_id_by_commit_id(ctx, &source)?;

    // Create a snapshot before the operation
    {
        let mut guard = ctx.exclusive_worktree_access();
        let _snapshot = ctx.create_snapshot(
            SnapshotDetails::new(OperationKind::SquashCommit),
            guard.write_permission(),
        )?;
    }

    // Perform the squash using the existing squash_commits function
    let new_commit_oid = gitbutler_branch_actions::squash_commits(
        ctx,
        stack_id,
        vec![source.to_git2()],
        destination.to_git2(),
    )?;

    // Now generate an AI commit message
    let new_message = generate_combined_message(&source_message, &destination_message).await;

    // If we got a new message from AI, update the commit message
    if let Ok(message) = new_message {
        gitbutler_branch_actions::update_commit_message(ctx, stack_id, new_commit_oid, &message)?;

        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "Squashed {} + {} → {}",
                source.to_string()[..7].blue(),
                destination.to_string()[..7].blue(),
                new_commit_oid.to_string()[..7].green()
            )?;
            writeln!(out)?;
            writeln!(out, "{}", "New commit message:".bold())?;
            writeln!(out, "{}", message)?;
        }
    } else {
        // Fall back to just reporting the squash without AI message
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "Squashed {} + {} → {}",
                source.to_string()[..7].blue(),
                destination.to_string()[..7].blue(),
                new_commit_oid.to_string()[..7].green()
            )?;
            if let Err(e) = new_message {
                writeln!(
                    out,
                    "{} {}",
                    "Note: Could not generate AI commit message:".yellow(),
                    e
                )?;
            }
        }
    }

    Ok(())
}

/// Resolve a string to a commit ObjectId.
fn resolve_commit_id(id_map: &IdMap, id_str: &str) -> Result<ObjectId> {
    let matches = id_map.resolve_entity_to_ids(id_str)?;

    if matches.is_empty() {
        bail!(
            "Could not find commit '{}'. Make sure it's a valid commit ID or CLI short ID.",
            id_str
        );
    }

    // Filter for only commit matches
    let commit_matches: Vec<_> = matches
        .iter()
        .filter_map(|id| {
            if let CliId::Commit(oid) = id {
                Some(*oid)
            } else {
                None
            }
        })
        .collect();

    if commit_matches.is_empty() {
        bail!(
            "'{}' does not resolve to a commit. It resolved to: {}",
            id_str,
            matches
                .iter()
                .map(|id| id.kind_for_humans())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if commit_matches.len() > 1 {
        bail!(
            "'{}' is ambiguous and matches {} commits. Please use a longer SHA.",
            id_str,
            commit_matches.len()
        );
    }

    Ok(commit_matches[0])
}

/// Determine which commit is the parent and which is the child.
/// Returns (source, destination) where source will be squashed into destination.
fn determine_parent_child(
    ctx: &mut Context,
    commit1: &ObjectId,
    commit2: &ObjectId,
) -> Result<(ObjectId, ObjectId)> {
    let git2_repo = ctx.git2_repo.get()?;

    // Check if commit1 is an ancestor of commit2 using merge_base
    let merge_base_1_2 = git2_repo.merge_base(commit1.to_git2(), commit2.to_git2());
    let commit1_is_ancestor = match merge_base_1_2 {
        Ok(base) => base == commit1.to_git2(),
        Err(_) => false,
    };

    // Check if commit2 is an ancestor of commit1
    let merge_base_2_1 = git2_repo.merge_base(commit2.to_git2(), commit1.to_git2());
    let commit2_is_ancestor = match merge_base_2_1 {
        Ok(base) => base == commit2.to_git2(),
        Err(_) => false,
    };

    match (commit1_is_ancestor, commit2_is_ancestor) {
        (true, false) => {
            // commit1 is a parent of commit2, so we squash commit2 into commit1
            // (child into parent)
            Ok((*commit2, *commit1))
        }
        (false, true) => {
            // commit2 is a parent of commit1, so we squash commit1 into commit2
            // (child into parent)
            Ok((*commit1, *commit2))
        }
        (false, false) => {
            bail!(
                "Neither commit is an ancestor of the other. \
                 {} and {} are not in a parent-child relationship.",
                &commit1.to_string()[..7],
                &commit2.to_string()[..7]
            );
        }
        (true, true) => {
            // This shouldn't happen unless they're the same commit
            bail!(
                "Cannot squash a commit with itself: {} and {} appear to be the same commit.",
                &commit1.to_string()[..7],
                &commit2.to_string()[..7]
            );
        }
    }
}

/// Get the commit message for a given commit ID using git2.
fn get_commit_message(ctx: &Context, commit_id: &ObjectId) -> Result<String> {
    let git2_repo = ctx.git2_repo.get()?;
    let commit = git2_repo.find_commit(commit_id.to_git2())?;
    let message = commit.message().unwrap_or("(no message)");
    Ok(message.to_string())
}

/// Find the stack ID for a given commit.
fn stack_id_by_commit_id(
    ctx: &Context,
    oid: &ObjectId,
) -> anyhow::Result<but_core::ref_metadata::StackId> {
    let stacks = crate::legacy::commits::stacks(ctx)?
        .iter()
        .filter_map(|s| {
            s.id.map(|id| crate::legacy::commits::stack_details(ctx, id).map(|d| (id, d)))
        })
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    if let Some((id, _)) = stacks.iter().find(|(_, stack)| {
        stack
            .branch_details
            .iter()
            .any(|branch| branch.commits.iter().any(|commit| commit.id == *oid))
    }) {
        return Ok(*id);
    }
    anyhow::bail!("No stack found for commit {}", oid)
}

/// Generate a combined commit message using AI.
async fn generate_combined_message(
    source_message: &str,
    destination_message: &str,
) -> Result<String> {
    let openai = OpenAiProvider::with(None)
        .ok_or_else(|| anyhow::anyhow!("No OpenAI provider available. Please configure OpenAI credentials or log in to GitButler."))?;

    let client = openai.client()?;

    let system_message =
        "You are a version control assistant that helps write git commit messages.".to_string();

    let user_message = format!(
        r#"Two commits have been squashed together. Generate a single, well-written commit message that combines the intent of both original messages.

{SQUASH_COMMIT_MESSAGE_INSTRUCTIONS}

## Original commit messages:

### Commit 1 (destination):
{destination_message}

### Commit 2 (source, being squashed into commit 1):
{source_message}

Generate a single commit message that captures the combined changes.
"#
    );

    use async_openai::types::chat::{
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    };

    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "camelCase")]
    struct SquashOutput {
        commit_message: String,
    }

    let schema = schemars::schema_for!(SquashOutput);
    let schema_json = serde_json::to_value(schema)?;
    let response_format = ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            description: None,
            name: "squash_commit_message".into(),
            schema: Some(schema_json),
            strict: Some(false),
        },
    };

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4o-mini")
        .messages([
            ChatCompletionRequestSystemMessage::from(system_message).into(),
            ChatCompletionRequestUserMessage::from(user_message).into(),
        ])
        .response_format(response_format)
        .build()?;

    let response = client.chat().create(request).await?;
    let response_string = response
        .choices
        .first()
        .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))?
        .message
        .content
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Empty response from OpenAI"))?;

    let output: SquashOutput = serde_json::from_str(response_string)
        .map_err(|e| anyhow::anyhow!("Failed to parse AI response: {}", e))?;

    Ok(output.commit_message)
}

const SQUASH_COMMIT_MESSAGE_INSTRUCTIONS: &str = r#"## Instructions for generating the commit message:

- Write a short summary line (50 characters or less) as the first line
- Follow with a blank line
- Then provide a paragraph explaining what was done and why
- If the original messages have good details, incorporate them
- Use the imperative mood (e.g., "Add feature" not "Added feature")
- Focus on the WHY and WHAT, not the HOW

## Example of a good squash commit message:

Add user authentication with session management

This combines the initial authentication scaffolding with the session
handling implementation. Users can now log in and their sessions are
properly managed with secure cookie storage.

Key changes:
- Implement login/logout endpoints
- Add session token generation and validation
- Store sessions in database with expiry"#;

//! Implementation of the `but squash` command.
//!
//! This command squashes multiple commits together and uses AI to generate a combined
//! commit message from all original messages.

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

/// Handle the squash command by squashing multiple commits and generating an AI commit message.
pub(crate) async fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit_strs: &[String],
) -> Result<()> {
    if commit_strs.len() < 2 {
        bail!("At least two commits are required for squashing.");
    }

    let mut id_map = IdMap::new_from_context(ctx)?;
    id_map.add_file_info_from_context(ctx)?;

    // Resolve all commit IDs
    let mut commit_ids: Vec<ObjectId> = Vec::new();
    for commit_str in commit_strs {
        let id = resolve_commit_id(&id_map, commit_str)?;
        commit_ids.push(id);
    }

    // Sort commits by their position in the commit history (oldest first)
    let sorted_commits = sort_commits_by_ancestry(ctx, &commit_ids)?;

    // The destination is the oldest commit (parent-most)
    let destination = sorted_commits.last().expect("at least 2 commits");
    // The sources are all the other commits (newer ones)
    let sources: Vec<ObjectId> = sorted_commits[..sorted_commits.len() - 1].to_vec();

    // Get all commit messages before squashing (in order from newest to oldest)
    let mut commit_messages: Vec<(ObjectId, String)> = Vec::new();
    for commit_id in &sorted_commits {
        let message = get_commit_message(ctx, commit_id)?;
        commit_messages.push((*commit_id, message));
    }

    // Find which stack this commit belongs to
    let stack_id = stack_id_by_commit_id(ctx, &sources[0])?;

    // Create a snapshot before the operation
    {
        let mut guard = ctx.exclusive_worktree_access();
        let _snapshot = ctx.create_snapshot(
            SnapshotDetails::new(OperationKind::SquashCommit),
            guard.write_permission(),
        )?;
    }

    // Format the list of squashed commits for display
    let commit_list: Vec<String> = sorted_commits
        .iter()
        .map(|id| id.to_string()[..7].to_string())
        .collect();

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Squashing {} commits: {}",
            sorted_commits.len(),
            commit_list
                .iter()
                .map(|s| s.blue().to_string())
                .collect::<Vec<_>>()
                .join(" + ")
        )?;
    }

    // Perform the squash using the existing squash_commits function
    let new_commit_oid = gitbutler_branch_actions::squash_commits(
        ctx,
        stack_id,
        sources.iter().map(|id| id.to_git2()).collect(),
        destination.to_git2(),
    )?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Squashed into {} → {}",
            destination.to_string()[..7].blue(),
            new_commit_oid.to_string()[..7].green()
        )?;
        writeln!(out)?;
        writeln!(out, "Asking AI to generate a combined commit message...")?;
    }

    // Now generate an AI commit message
    let new_message = generate_combined_message(&commit_messages).await;

    // If we got a new message from AI, update the commit message
    if let Ok(message) = new_message {
        gitbutler_branch_actions::update_commit_message(ctx, stack_id, new_commit_oid, &message)?;

        if let Some(out) = out.for_human() {
            writeln!(out)?;
            writeln!(out, "{}", "New commit message:".bold())?;
            writeln!(out, "{}", message)?;
        }
    } else {
        // Fall back to just reporting the squash without AI message
        if let Some(out) = out.for_human() {
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

/// Sort commits by their position in the commit graph.
/// Returns commits ordered from newest (child-most) to oldest (parent-most).
fn sort_commits_by_ancestry(ctx: &mut Context, commits: &[ObjectId]) -> Result<Vec<ObjectId>> {
    let git2_repo = ctx.git2_repo.get()?;

    // Verify all commits are in a linear ancestry chain
    // First, find which commits are ancestors of which
    let mut ordered: Vec<ObjectId> = commits.to_vec();

    // Sort by checking ancestry relationships
    ordered.sort_by(|a, b| {
        let a_is_ancestor_of_b = git2_repo
            .merge_base(a.to_git2(), b.to_git2())
            .map(|base| base == a.to_git2())
            .unwrap_or(false);

        let b_is_ancestor_of_a = git2_repo
            .merge_base(a.to_git2(), b.to_git2())
            .map(|base| base == b.to_git2())
            .unwrap_or(false);

        if a_is_ancestor_of_b {
            std::cmp::Ordering::Greater // a is older, should come later
        } else if b_is_ancestor_of_a {
            std::cmp::Ordering::Less // b is older, a should come first
        } else {
            std::cmp::Ordering::Equal
        }
    });

    // Verify all commits form a contiguous chain
    for i in 0..ordered.len() - 1 {
        let child = &ordered[i];
        let parent = &ordered[i + 1];

        // Check if parent is actually an ancestor of child
        let is_ancestor = git2_repo
            .merge_base(parent.to_git2(), child.to_git2())
            .map(|base| base == parent.to_git2())
            .unwrap_or(false);

        if !is_ancestor {
            bail!(
                "Commits {} and {} are not in a parent-child relationship. \
                 All commits must be in the same linear history.",
                &child.to_string()[..7],
                &parent.to_string()[..7]
            );
        }
    }

    Ok(ordered)
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
async fn generate_combined_message(commit_messages: &[(ObjectId, String)]) -> Result<String> {
    let openai = OpenAiProvider::with(None).ok_or_else(|| {
        anyhow::anyhow!(
            "No OpenAI provider available. Please configure OpenAI credentials or log in to GitButler."
        )
    })?;

    let client = openai.client()?;

    let system_message =
        "You are a version control assistant that helps write git commit messages.".to_string();

    // Build the commit messages section
    let messages_section: String = commit_messages
        .iter()
        .enumerate()
        .map(|(i, (oid, msg))| {
            format!(
                "### Commit {} ({}):\n{}",
                i + 1,
                &oid.to_string()[..7],
                msg.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let commit_count = commit_messages.len();
    let user_message = format!(
        r#"{commit_count} commits have been squashed together. Generate a single, well-written commit message that combines the intent of all original messages.

{SQUASH_COMMIT_MESSAGE_INSTRUCTIONS}

## Original commit messages (from newest to oldest):

{messages_section}

Generate a single commit message that captures all the combined changes.
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
- When squashing many commits, synthesize the key changes rather than listing every detail

## Example of a good squash commit message:

Add user authentication with session management

This combines the initial authentication scaffolding with the session
handling implementation. Users can now log in and their sessions are
properly managed with secure cookie storage.

Key changes:
- Implement login/logout endpoints
- Add session token generation and validation
- Store sessions in database with expiry"#;

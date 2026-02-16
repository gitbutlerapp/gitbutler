//! AI-powered commit message generation.
//!
//! This module provides functionality to generate commit messages using AI (LLM) based on
//! the unified diff of changes and an optional user-provided summary.

use std::fmt::Write as _;

use anyhow::{Context, Result};
use but_llm::{ChatMessage, LLMProvider};
use colored::Colorize;
use schemars::JsonSchema;

use crate::utils::OutputChannel;

/// Generate a commit message using AI based on the unified diff and optional user summary.
///
/// This function uses an LLM (Large Language Model) to analyze the provided diff and generate
/// a well-formatted commit message that follows best practices. The generated message will
/// include a short summary line and a longer explanation of the changes.
///
/// # Arguments
///
/// * `out` - Output channel for displaying progress messages to the user.
/// * `diff` - A unified diff string showing the changes to be committed. Should be in standard
///   unified diff format with file headers and hunks.
/// * `user_summary` - An optional user-provided summary that gives context about the changes.
///   If provided, the AI will use this to generate a more accurate and contextual commit message.
///   If `None`, the AI will generate the message based solely on the diff.
///
/// # Returns
///
/// Returns a formatted commit message string on success, or an error if:
/// - The OpenAI LLM provider cannot be initialized (e.g., missing API credentials)
/// - The AI request fails
/// - The AI response cannot be parsed as a valid commit message
pub fn generate_commit_message(out: &mut OutputChannel, diff: &str, user_summary: Option<String>) -> Result<String> {
    let mut progress = out.progress_channel();

    if out.for_human().is_some() {
        writeln!(progress, "{}", "Generating commit message...".bright_cyan())?;
    }
    let llm = LLMProvider::default_openai()
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize default OpenAI LLM provider"))?;
    let system_message = "You are a version control assistant that helps with Git branch committing.".to_string();
    let summary = user_summary.unwrap_or_default();
    let user_message = format!(
        r#"Extract the git commit data from the user summary if provided and the diff output.
Return the commit message. Determine from this user summary and diff output what the git commit data should be.

{DEFAULT_COMMIT_MESSAGE_INSTRUCTIONS}

Here is the data:

User summary (optional): {summary}

unified diff:
```patch
{diff}
```
"#
    );

    let chat_messages = vec![ChatMessage::User(user_message)];
    let response = llm
        .structured_output::<StructuredOutput>(&system_message, chat_messages, "gpt-5-mini")?
        .context("Failed to generate structured content for commit message")?;

    Ok(response.commit_message)
}

/// Generate an updated commit message when squashing multiple commits together.
///
/// This function uses an LLM to create a cohesive commit message that combines the
/// destination commit message with relevant details from the source commits being squashed.
/// The AI favors the destination message as the primary message and enhances it with
/// substantial details from the source commits without making it overly long.
///
/// # Arguments
///
/// * `out` - Output channel for displaying progress messages to the user.
/// * `source_messages` - Commit messages from the commits being squashed into the destination.
///   Each message will be labeled and presented to the AI as "Commit #N that is being squashed".
/// * `destination_message` - The original commit message that serves as the base for the updated
///   message. This is given priority in the final result.
/// * `user_summary` - An optional user-provided summary or instructions to guide how the AI should
///   update the commit message. For example, the user might want to highlight specific changes or
///   exclude details from certain squashed commits.
///
/// # Returns
///
/// Returns an updated commit message string on success, or an error if:
/// - The OpenAI LLM provider cannot be initialized (e.g., missing API credentials)
/// - The AI request fails
/// - The AI response cannot be parsed as a valid commit message
pub fn generate_commit_message_from_multiple_messages(
    out: &mut OutputChannel,
    source_messages: Vec<String>,
    destination_message: String,
    user_summary: Option<String>,
) -> Result<String> {
    let mut progress = out.progress_channel();
    if out.for_human().is_some() {
        writeln!(progress, "{}", "Generating commit message...".bright_cyan())?;
    }
    let llm = LLMProvider::default_openai()
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize default OpenAI LLM provider"))?;
    let system_message = "You are a version control assistant that helps with Git branch committing.".to_string();
    let summary = user_summary.unwrap_or_default();

    // Format source messages with descriptive introductions
    let formatted_source_messages = source_messages
        .iter()
        .enumerate()
        .map(|(i, msg)| format!("Commit #{} that is being squashed:\n\n{}", i + 1, msg))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let user_message = format!(
        r#"Update an existing commit message to reflect the outcome of squashing another commit into it.
Return the updated commit message. Use the original commit message and include any relevant details from the commit messages of the commits that are being squashed into it.
Favor the original commit message, and only enhance it with details if they are substantial. Don't make the updated message overly long.
If a user summary is provided, use it to guide how to update the commit message appropriately, e.g. the user may want to highlight something or exclude the details of the squashed commits.

{DEFAULT_COMMIT_MESSAGE_INSTRUCTIONS}

Here is the data to work with:

User summary or instructions (optional): {summary}

Original commit message to update (the most important one for the final message):

{destination_message}

A list of incoming commit message(s) from commits that are being squashed into the original:

{formatted_source_messages}
"#
    );

    let chat_messages = vec![ChatMessage::User(user_message)];
    let response = llm
        .structured_output::<StructuredOutput>(&system_message, chat_messages, "gpt-5-mini")?
        .context("Failed to generate structured content for commit message")?;

    Ok(response.commit_message)
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
struct StructuredOutput {
    pub commit_message: String,
}

const DEFAULT_COMMIT_MESSAGE_INSTRUCTIONS: &str = r#"The message should be a short summary line, followed by two newlines, then a SHORT paragraph explaining WHY the change was needed based off the prompt.

- If a summary is provided, use it to create more short paragraphs or bullet points explaining the changes.
- The first summary line should be no more than 50 characters.
- Use the imperative mood for the message (e.g. "Add user authentication system" instead of "Adding user authentication system").
- The lines after the first should be hard wrapped at 72 characters.

Here is an example of a good commit message:

mingw: implement readlink()

Implement `readlink()` by reading NTFS reparse points via the
`read_reparse_point()` function that was introduced earlier to determine
the length of symlink targets. Works for symlinks and directory
junctions."#;

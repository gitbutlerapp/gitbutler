use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
        CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
};
use schemars::{JsonSchema, schema_for};

use crate::OpenAiProvider;

#[expect(dead_code)]
pub fn commit_message_blocking(
    openai: &OpenAiProvider,
    external_summary: &str,
    external_prompt: &str,
    diff: &str,
) -> anyhow::Result<String> {
    let change_summary_owned = external_summary.to_string();
    let external_prompt_owned = external_prompt.to_string();
    let diff_owned = diff.to_string();
    let client = openai.client()?;

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(commit_message(
                &client,
                &change_summary_owned,
                &external_prompt_owned,
                &diff_owned,
            ))
    })
    .join()
    .unwrap()
}

pub async fn commit_message(
    // openai_provider: OpenAiProvider,
    client: &Client<OpenAIConfig>,
    external_summary: &str,
    external_prompt: &str,
    diff: &str,
) -> anyhow::Result<String> {
    let system_message =
        "You are a version control assistant that helps with Git branch committing.".to_string();
    let user_message = format!(
        r#"Extract the git commit data from the prompt, summary and diff output.
Return the commit message. Determine from this AI prompt, summary and diff output what the git commit data should be.

{DEFAULT_COMMIT_MESSAGE_INSTRUCTIONS}

Here is the data:

Prompt: {external_prompt}

Summary: {external_summary}

unified diff:
```patch
{diff}
```
"#
    );

    let schema = schema_for!(StructuredOutput);
    let schema_json = serde_json::to_value(schema).unwrap();
    let response_format = ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            description: None,
            name: "commit_message".into(),
            schema: Some(schema_json),
            strict: Some(true),
        },
    };

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-5-mini")
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
        .unwrap()
        .message
        .content
        .as_ref()
        .unwrap();

    let structured_output: StructuredOutput = serde_json::from_str(response_string)
        .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

    Ok(structured_output.commit_message)
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct StructuredOutput {
    pub commit_message: String,
}

pub async fn branch_name(
    client: &Client<OpenAIConfig>,
    commit_messages: &[String],
    diffs: &[String],
    existing_branch_names: &[String],
) -> anyhow::Result<String> {
    let system_message =
        "You are a version control assistant that helps with Git branch naming.".to_string();
    let user_message = format!(
        "Generate a concise and descriptive branch name based on the provided commit messages.
        Keep the branch name short, ideally under 50 characters. Only user lowercase letters, numbers, and hyphens.
        Don't use other special characters or spaces.

        <important_notes>
            The branch name should reflect the main content of the commit messages and, if available, change diffs.
            Try to make the branch name unique and noticeably different from existing branch names.
            In order to make it noticeably different, use a different first word for the branh name.
            Do not use any of the existing branch names.

        </important_notes>
        
        <exisiting_branch_names>
        {}
        </existing_branch_names>

        <commit_messages>
        {}
        </commit_messages>
        
        <diffs>
        {}
        </diffs>
        ",
        existing_branch_names.join(",\n"),
        commit_messages.join("\n==================\n"),
        diffs.join("\n==================\n")
    );

    let schema = schema_for!(GenerateBranchNameOutput);
    let schema_json = serde_json::to_value(schema)?;
    let response_format = ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema {
            description: None,
            name: "branch_name".into(),
            schema: Some(schema_json),
            strict: Some(false),
        },
    };

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-5-mini")
        .messages([
            ChatCompletionRequestSystemMessage::from(system_message).into(),
            ChatCompletionRequestUserMessage::from(user_message).into(),
        ])
        .response_format(response_format)
        .build()?;

    let response = client.chat().create(request).await?;
    let choice = response
        .choices
        .first()
        .ok_or_else(|| anyhow::anyhow!("No choices returned from OpenAI response"))?;

    let response_string = choice
        .message
        .content
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No content in OpenAI response message"))?;

    let structured_output: GenerateBranchNameOutput = serde_json::from_str(response_string)
        .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

    Ok(structured_output.branch_name)
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateBranchNameOutput {
    #[schemars(description = "
    <description>
        The generated branch name based on the commit messages.
    </description>

    <important_notes>
        The branch name should be concise, descriptive, and follow the naming conventions.
        It should not contain spaces or special characters other than hyphens.
        Return the branch name only, no backticks or quotes.
        It should be noticeably different from existing branch names.
    </important_notes>
    ")]
    pub branch_name: String,
}

const DEFAULT_COMMIT_MESSAGE_INSTRUCTIONS: &str = r#"The message should be a short summary line, followed by two newlines, then a short paragraph explaining WHY the change was needed based off the prompt.

- If a summary is provided, use it to create more short paragraphs or bullet points explaining the changes.
- The first summary line should be no more than 50 characters.
- Use the imperative mood for the message (e.g. "Add user authentication system" instead of "Adding user authentication system").

Here is an example of a good commit message:

bundle-uri: copy all bundle references ino the refs/bundle space

When downloading bundles via the bundle-uri functionality, we only copy the
references from refs/heads into the refs/bundle space. I'm not sure why this
refspec is hardcoded to be so limited, but it makes the ref negotiation on
the subsequent fetch suboptimal, since it won't use objects that are
referenced outside of the current heads of the bundled repository.

This change to copy everything in refs/ in the bundle to refs/bundles/
significantly helps the subsequent fetch, since nearly all the references
are now included in the negotiation.

The update to the bundle-uri unbundling refspec puts all the heads from a
bundle file into refs/bundle/heads instead of directly into refs/bundle/ so
the tests also need to be updated to look in the new heirarchy."#;

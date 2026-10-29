use anyhow::Context;
use but_llm::ChatMessage;
use schemars::JsonSchema;

pub fn commit_message(
    llm: &but_llm::LLMProvider,
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

    let chat_messages = vec![ChatMessage::User(user_message)];
    let model = llm.model_or_default();
    let response = llm
        .structured_output::<StructuredOutput>(&system_message, chat_messages, &model)?
        .context("Failed to generate structured content for commit message")?;

    Ok(response.commit_message)
}

#[derive(serde::Serialize, serde::Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct StructuredOutput {
    pub commit_message: String,
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
the tests also need to be updated to look in the new hierarchy."#;

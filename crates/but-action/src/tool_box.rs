use anyhow::Context;
use but_workspace::ui::StackEntry;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use std::fmt::Debug;

use async_openai::types::{ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage};
use gitbutler_command_context::CommandContext;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

use crate::OpenAiProvider;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Tool {
    #[schemars(description = "The name of the tool")]
    pub name: String,
    #[schemars(description = "A description of what the tool does")]
    pub description: String,
    #[schemars(description = "The parameters required by the tool")]
    pub parameters: serde_json::Value,
}

impl From<&Tool> for async_openai::types::ChatCompletionTool {
    fn from(tool: &Tool) -> Self {
        let description = tool.description.clone();
        let parameters = tool.parameters.clone();
        let function = async_openai::types::FunctionObject {
            name: tool.name.clone(),
            description: Some(description),
            parameters: Some(parameters),
            strict: Some(false),
        };

        async_openai::types::ChatCompletionTool {
            r#type: async_openai::types::ChatCompletionToolType::Function,
            function,
        }
    }
}

pub fn create_tool<T: Debug + Serialize + JsonSchema>(name: &str, description: &str) -> Tool {
    let schema = schema_for!(T);
    let json_schema = serde_json::to_value(&schema).expect("Failed to serialize schema to JSON");

    Tool {
        name: name.to_string(),
        description: description.to_string(),
        parameters: json_schema,
    }
}

pub struct ToolBox<'a> {
    openai: &'a OpenAiProvider,
    ctx: &'a mut CommandContext,
    pub tools: Vec<Tool>,
}

impl<'a> ToolBox<'a> {
    pub fn new(ctx: &'a mut CommandContext, openai: &'a OpenAiProvider) -> Self {
        let tools = vec![
            create_tool::<CommitParameters>("commit", "Commit the file changes into a stack"),
            create_tool::<CreateBranchParameters>(
                "create_branch",
                "Create a new branch in the project",
            ),
        ];
        ToolBox { ctx, openai, tools }
    }

    pub fn execute(&self, prompt: &str) -> anyhow::Result<String> {
        let tools: Vec<async_openai::types::ChatCompletionTool> =
            self.tools.iter().map(|tool| tool.into()).collect();
        let system_message =
            "You are a helpful assistant that can commit changes to a project repository. \
            You can create branches, commit changes, and amend commits. \
            Use the tools provided to perform these actions.
            Don't respond with anything other than with a 'done'.
            ";

        let mut messages = vec![
            ChatCompletionRequestSystemMessage::from(system_message).into(),
            ChatCompletionRequestUserMessage::from(prompt).into(),
        ];

        let mut response =
            crate::openai::tool_calling_blocking(self.openai, messages.clone(), tools.clone())?;

        while let Some(tool_calls) = response
            .choices
            .first()
            .and_then(|choice| choice.message.tool_calls.as_ref())
        {
            let mut tool_calls_messages: Vec<async_openai::types::ChatCompletionMessageToolCall> =
                vec![];
            let mut tool_response_messages: Vec<async_openai::types::ChatCompletionRequestMessage> =
                vec![];

            for call in tool_calls {
                let function_name = call.function.name.clone();
                let function_args = call.function.arguments.clone();

                let tool_response = self
                    .call_tool(&function_name, &function_args)
                    .context(format!("Failed to call tool: {}", function_name))?;

                let tool_response_str = serde_json::to_string(&tool_response)
                    .context("Failed to serialize tool response")?;

                tool_calls_messages.push(async_openai::types::ChatCompletionMessageToolCall {
                    id: call.id.clone(),
                    r#type: async_openai::types::ChatCompletionToolType::Function,
                    function: async_openai::types::FunctionCall {
                        name: function_name,
                        arguments: function_args,
                    },
                });

                tool_response_messages.push(
                    async_openai::types::ChatCompletionRequestMessage::Tool(
                        async_openai::types::ChatCompletionRequestToolMessage {
                            tool_call_id: call.id.clone(),
                            content:
                                async_openai::types::ChatCompletionRequestToolMessageContent::Text(
                                    tool_response_str,
                                ),
                        },
                    ),
                );
            }

            messages.push(
                async_openai::types::ChatCompletionRequestMessage::Assistant(
                    async_openai::types::ChatCompletionRequestAssistantMessage {
                        tool_calls: Some(tool_calls_messages),
                        ..Default::default()
                    },
                ),
            );

            messages.extend(tool_response_messages);

            response =
                crate::openai::tool_calling_blocking(self.openai, messages.clone(), tools.clone())?;
        }

        let response_message = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_else(|| "No response from AI".to_string());

        Ok(response_message)
    }

    fn call_tool(&self, tool_name: &str, params: &str) -> anyhow::Result<serde_json::Value> {
        match tool_name {
            "commit" => {
                let commit_params: CommitParameters =
                    serde_json::from_str(params).context("Failed to parse commit parameters")?;
                let outcome = self.commit(commit_params)?;
                let value =
                    serde_json::to_value(outcome).context("Failed to serialize commit outcome")?;
                Ok(value)
            }
            "create_branch" => {
                let branch_params: CreateBranchParameters = serde_json::from_str(params)
                    .context("Failed to parse create branch parameters")?;
                let stack_entry = self.create_branch(branch_params)?;
                let value = serde_json::to_value(stack_entry)?;
                Ok(value)
            }
            _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        }
    }

    fn commit(
        &self,
        params: CommitParameters,
    ) -> anyhow::Result<but_workspace::commit_engine::ui::CreateCommitOutcome> {
        let repo = self.ctx.gix_repo()?;
        let mut guard = self.ctx.project().exclusive_worktree_access();

        let worktree = but_core::diff::worktree_changes(&repo)?;
        let file_changes: Vec<but_workspace::DiffSpec> = worktree
            .changes
            .iter()
            .filter(|change| params.files.contains(&change.path.to_string()))
            .map(Into::into)
            .collect::<Vec<_>>();

        let stacks = crate::stacks(self.ctx, &repo)?;

        let stack_id = stacks
            .iter()
            .find(|s| s.heads.iter().any(|h| h.name == params.branch_name))
            .map(|s| s.id)
            .ok_or_else(|| anyhow::anyhow!("Branch '{}' not found", params.branch_name))?;

        let snapshot_tree = self.ctx.prepare_snapshot(guard.read_permission());

        let outcome = but_workspace::commit_engine::create_commit_simple(
            self.ctx,
            stack_id,
            None,
            file_changes,
            params.message.clone(),
            params.branch_name,
            guard.write_permission(),
        );

        let _ = snapshot_tree.and_then(|snapshot_tree| {
            self.ctx.snapshot_commit_creation(
                snapshot_tree,
                outcome.as_ref().err(),
                params.message,
                None,
                guard.write_permission(),
            )
        });

        let outcome = outcome?.into();
        Ok(outcome)
    }

    fn create_branch(&self, params: CreateBranchParameters) -> anyhow::Result<StackEntry> {
        let mut guard = self.ctx.project().exclusive_worktree_access();
        let perm = guard.write_permission();

        let branch = BranchCreateRequest {
            name: Some(params.branch_name),
            ..Default::default()
        };

        gitbutler_branch_actions::create_virtual_branch(self.ctx, &branch, perm)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitParameters {
    /// The commit message to use.
    #[schemars(description = "The commit message to use")]
    pub message: String,
    /// The branch name to commit to.
    #[schemars(description = "The branch name to commit to")]
    pub branch_name: String,
    /// The list of files to commit.
    #[schemars(
        description = "The list of files to commit. This should be the paths of the files relative to the project root."
    )]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchParameters {
    /// The name of the new branch to create.
    #[schemars(description = "The name of the new branch to create")]
    pub branch_name: String,
    /// The description of the new branch.
    #[schemars(
        description = "The description of the new branch. This should be a short summary of the branch's purpose."
    )]
    pub description: String,
}

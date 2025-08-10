use but_action::OpenAiProvider;
use but_tools::emit::Emittable;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;

use crate::{
    agent::{Agent, AgentGraphNode},
    state::{AgentState, Todo},
};

const MODEL: &str = "gpt-4.1";

const SYS_PROMPT: &str = "
You are a GitButler agent that can perform various actions on a Git project.
Your name is ButBot. Your main goal is to help the user with handling file changes in the project.
Use the tools provided to you to perform the actions and respond with a summary of the action you've taken.
Don't be too verbose, but be thorough and outline everything you did.

### Core concepts
- **Project**: A Git repository that has been initialized with GitButler.
- **Stack**: A collection of dependent branches that are used to manage changes in the project. With GitButler (as opposed to normal Git), multiple stacks can be applied at the same time.
- **Branch**: A pointer to a specific commit in the project. Branches can contain multiple commits. Commits are always listed newest to oldest.
- **Commit**: A snapshot of the project at a specific point in time.
- **File changes**: A set of changes made to the files in the project. This can include additions, deletions, and modifications of files. The user can assign these changes to stacks to keep things ordering.
- **Lock**: A lock or dependency on a file change. This refers to the fact that certain uncommitted file changes can only be committed to a specific stack.
    This is because the uncommitted changes were done on top of previously committed file changes that are part of the stack.

### Main task
Please, take a look at the provided prompt and the project status below, and perform the actions you think are necessary.
In order to do that, please follow these steps:
    1. Take a look at the prompt and reflect on what the intention of the user is.
    2. Take a look at the project status and see what changes are present in the project. It's important to understand what stacks and branche are present, and what the file changes are.
    3. Try to correlate the prompt with the project status and determine what actions you can take to help the user.
    4. Use the tools provided to you to perform the actions.

### Capabilities
You can generally perform the normal Git operations, such as creating branches and committing to them.
You can also perform more advanced operations, such as:
- `absorb`: Take a set of file changes and amend them into the existing commits in the project.
    This requires you to figure out where the changes should go based on the locks, assingments and any other user provided information.
- `split a commit`: Take an existing commit and split it into multiple commits based on the the user directive.
    This can be achieved by using the `split_commit` tool.
- `split a branch`: Take an existing branch and split it into two branches. This basically takes a set of committed file changes and moves them to a new branch, removing them from the original branch.
    This is useful when you want to separate the changes into a new branch for further work.
    In order to do this, you will need to get the branch changes for the intended source branch (call the `get_branch_changes` tool), and then call the split branch tool with the changes you want to split off.

### Good practices
- Small commits are better than large commits. Try to keep commits focused (4-5 file changes at most unless otherwise specified).
- Assume that all commits should be made to the same branch, unless the user specifies otherwise.
- If the user asks you to commit to a specific branch, make sure to commit to that branch only.

### Important notes
- Only perform the action on the file changes specified in the prompt.
- If the prompt is not clear, ask the user for clarification.
- When told to commit to the existing branch, commit to the applied stack-branch. Don't create a new branch unless explicitly asked to do so.
";

pub struct ButBot<'a> {
    state: AgentState,
    ctx: &'a mut CommandContext,
    emitter: std::sync::Arc<but_tools::emit::Emitter>,
    message_id: String,
    project_id: ProjectId,
    openai: &'a OpenAiProvider,
    chat_messages: Vec<but_action::ChatMessage>,
    text_response_buffer: Vec<String>,
}

impl<'a> ButBot<'a> {
    pub fn new(
        ctx: &'a mut CommandContext,
        emitter: std::sync::Arc<but_tools::emit::Emitter>,
        message_id: String,
        project_id: ProjectId,
        openai: &'a OpenAiProvider,
        chat_messages: Vec<but_action::ChatMessage>,
    ) -> Self {
        Self {
            state: AgentState::new(project_id, message_id.clone(), emitter.clone()),
            ctx,
            emitter,
            message_id,
            project_id,
            openai,
            chat_messages,
            text_response_buffer: vec![],
        }
    }

    /// Update the agent's state and the provided todos.
    ///
    /// Based on the provided chat messages and the project status, this function will
    /// update the agent's internal todo list.
    pub fn update_state(&mut self) -> anyhow::Result<()> {
        let repo = self.ctx.gix_repo()?;
        let project_status = but_tools::workspace::get_project_status(self.ctx, &repo, None)?;
        let serialized_status = serde_json::to_string_pretty(&project_status)
            .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;

        let conversation = self
            .chat_messages
            .iter()
            .map(|msg| msg.to_string())
            .collect::<Vec<_>>()
            .join("\n\n");

        let request = format!(
            "
Alright, let's create the plan for what the user wants to do.
The plan should be based on the user request found in the conversation below.
Add items to the todo list based on the plan.
Reference relevant resources from the project status (e.g. branches, commits, file changes) when creating the todo items.

<CONVERSATION>
{}
</CONVERSATION>
",
            conversation
        );

        let internal_chat_messages: Vec<but_action::ChatMessage> = vec![
            but_action::ChatMessage::ToolCall(but_action::ToolCallContent {
                id: "project_status".to_string(),
                name: "get_project_status".to_string(),
                arguments: "{\"filterChanges\": null}".to_string(),
            }),
            but_action::ChatMessage::ToolResponse(but_action::ToolResponseContent {
                id: "project_status".to_string(),
                result: serialized_status,
            }),
            but_action::ChatMessage::User(request),
        ];

        but_action::tool_calling_loop(
            self.openai,
            &self.state.sys_prompt.clone(),
            internal_chat_messages,
            &mut self.state,
            Some(MODEL.to_string()),
        )?;

        Ok(())
    }

    /// Update the status of a todo item based on the conversation and project status.
    ///
    /// Will take a look a the conversation and the project status, and update the status of the todo item.
    /// This also updates the todo list, adding new todos if necessary.
    pub fn update_todo_status(&mut self, todo: &Todo) -> anyhow::Result<()> {
        let repo = self.ctx.gix_repo()?;
        let project_status = but_tools::workspace::get_project_status(self.ctx, &repo, None)?;
        let serialized_status = serde_json::to_string_pretty(&project_status)
            .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;

        let conversation = self
            .chat_messages
            .iter()
            .map(|msg| msg.to_string())
            .collect::<Vec<_>>()
            .join("\n\n");

        let request = format!(
            "
Based on the conversation below and the project status, please update the status of the todo item.

<TODO_ITEM_TO_UPDATE>
{}
</TODO_ITEM_TO_UPDATE>


<CONVERSATION>
{}
</CONVERSATION>
",
            todo, conversation
        );

        let internal_chat_messages: Vec<but_action::ChatMessage> = vec![
            but_action::ChatMessage::ToolCall(but_action::ToolCallContent {
                id: "project_status".to_string(),
                name: "get_project_status".to_string(),
                arguments: "{\"filterChanges\": null}".to_string(),
            }),
            but_action::ChatMessage::ToolResponse(but_action::ToolResponseContent {
                id: "project_status".to_string(),
                result: serialized_status,
            }),
            but_action::ChatMessage::User(request),
        ];

        but_action::tool_calling_loop(
            self.openai,
            &self.state.sys_prompt.clone(),
            internal_chat_messages,
            &mut self.state,
            Some(MODEL.to_string()),
        )?;

        Ok(())
    }

    /// This is the workspace loop. This handles the main workspace actions.
    fn workspace_loop(&mut self) -> anyhow::Result<String> {
        let repo = self.ctx.gix_repo()?;
        let project_status = but_tools::workspace::get_project_status(self.ctx, &repo, None)?;
        let serialized_status = serde_json::to_string_pretty(&project_status)
            .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;

        let mut toolset = but_tools::workspace::workspace_toolset(
            self.ctx,
            self.emitter.clone(),
            self.message_id.clone(),
        );

        let mut internal_chat_messages = self.chat_messages.clone();

        // Add the project status to the chat messages.
        internal_chat_messages.push(but_action::ChatMessage::ToolCall(
            but_action::ToolCallContent {
                id: "project_status".to_string(),
                name: "get_project_status".to_string(),
                arguments: "{\"filterChanges\": null}".to_string(),
            },
        ));

        internal_chat_messages.push(but_action::ChatMessage::ToolResponse(
            but_action::ToolResponseContent {
                id: "project_status".to_string(),
                result: serialized_status,
            },
        ));

        // Now we trigger the tool calling loop.
        let message_id_cloned = self.message_id.clone();
        let project_id_cloned = self.project_id;
        let on_token_cb: std::sync::Arc<dyn Fn(&str) + Send + Sync + 'static> =
            std::sync::Arc::new({
                let emitter = self.emitter.clone();
                let message_id = message_id_cloned;
                let project_id = project_id_cloned;
                move |token: &str| {
                    let token_update = but_tools::emit::TokenUpdate {
                        token: token.to_string(),
                        project_id,
                        message_id: message_id.clone(),
                    };
                    let (name, payload) = token_update.emittable();
                    (emitter)(&name, payload);
                }
            });

        let (response, _) = but_action::tool_calling_loop_stream(
            self.openai,
            SYS_PROMPT,
            internal_chat_messages,
            &mut toolset,
            Some(MODEL.to_string()),
            on_token_cb,
        )?;

        Ok(response)
    }

    /// Given a todo, execute the action.
    ///
    /// This function will take the todo item, the chat messages, and the project status,
    /// and execute the action specified in the todo item.
    fn execute_todo(
        &mut self,
        todo: &Todo,
    ) -> anyhow::Result<(String, Vec<but_action::ChatMessage>)> {
        let repo = self.ctx.gix_repo()?;
        let project_status = but_tools::workspace::get_project_status(self.ctx, &repo, None)?;
        let serialized_status = serde_json::to_string_pretty(&project_status)
            .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;

        let mut toolset = but_tools::workspace::workspace_toolset(
            self.ctx,
            self.emitter.clone(),
            self.message_id.clone(),
        );

        let mut internal_chat_messages = self.chat_messages.clone();

        let request = format!(
            "
<TODO_DIRECTIVE>
<TODO>
{}
</TODO>

<IMPORTANT_NOTES>
Keep your response short please.
Follow best practices.
ONLY perform the actions that are necessary to complete the todo item above.
If you need to perform investigations, do so, and be detailed in your findings. Don't perform actions.
If you need to perform actions, do so, and be concise in the description of the actions.
</IMPORTANT_NOTES>
</TODO_DIRECTIVE>
",
            todo.as_prompt()
        );

        internal_chat_messages.push(but_action::ChatMessage::User(request));

        // Add the project status to the chat messages.
        internal_chat_messages.push(but_action::ChatMessage::ToolCall(
            but_action::ToolCallContent {
                id: "project_status".to_string(),
                name: "get_project_status".to_string(),
                arguments: "{\"filterChanges\": null}".to_string(),
            },
        ));

        internal_chat_messages.push(but_action::ChatMessage::ToolResponse(
            but_action::ToolResponseContent {
                id: "project_status".to_string(),
                result: serialized_status,
            },
        ));

        // Now we trigger the tool calling loop.
        let message_id_cloned = self.message_id.clone();
        let project_id_cloned = self.project_id;
        let on_token_cb: std::sync::Arc<dyn Fn(&str) + Send + Sync + 'static> =
            std::sync::Arc::new({
                let emitter = self.emitter.clone();
                let message_id = message_id_cloned;
                let project_id = project_id_cloned;
                move |token: &str| {
                    let token_update = but_tools::emit::TokenUpdate {
                        token: token.to_string(),
                        project_id,
                        message_id: message_id.clone(),
                    };
                    let (name, payload) = token_update.emittable();
                    (emitter)(&name, payload);
                }
            });

        let (response, messages) = but_action::tool_calling_loop_stream(
            self.openai,
            SYS_PROMPT,
            internal_chat_messages,
            &mut toolset,
            Some(MODEL.to_string()),
            on_token_cb,
        )?;

        // Remove the injected project status tool calls and responses from the messages.
        internal_chat_messages = messages
            .into_iter()
            .filter(|msg| match msg {
                but_action::ChatMessage::ToolCall(tool_call) => tool_call.id != "project_status",
                but_action::ChatMessage::ToolResponse(tool_response) => {
                    tool_response.id != "project_status"
                }
                _ => true,
            })
            .collect();

        // Emit a new like
        let end_token_update = but_tools::emit::TokenEnd {
            project_id: self.project_id,
            message_id: self.message_id.clone(),
        };
        let (end_name, end_payload) = end_token_update.emittable();
        (self.emitter)(&end_name, end_payload);

        Ok((response, internal_chat_messages))
    }
}

impl Agent for ButBot<'_> {
    fn create_todos(&mut self) -> anyhow::Result<AgentGraphNode> {
        self.update_state()?;

        if self.state.nothig_to_do() {
            let response = self.workspace_loop()?;
            Ok(AgentGraphNode::Done(response))
        } else {
            Ok(AgentGraphNode::ExecuteTodo)
        }
    }

    fn execute_todo(&mut self) -> anyhow::Result<AgentGraphNode> {
        if let Some(todo) = self.state.next_todo() {
            println!("Executing todo: {}", todo.clone().as_prompt());
            let (text_response, messages) = self.execute_todo(&todo)?;
            self.chat_messages = messages;
            self.text_response_buffer.push(text_response);
            self.update_todo_status(&todo)?;
            Ok(AgentGraphNode::ExecuteTodo)
        } else {
            let final_response = self.text_response_buffer.join("\n\n---\n\n");
            Ok(AgentGraphNode::Done(final_response))
        }
    }
}

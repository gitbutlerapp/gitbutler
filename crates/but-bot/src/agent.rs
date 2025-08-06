use but_action::OpenAiProvider;
use but_tools::emit::Emittable;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;

pub enum TodoStatus {
    Waiting,
    InProgress,
    Success { message: String },
    Failed { message: String },
}

pub struct Todo {
    pub id: uuid::Uuid,
    pub title: String,
    pub description: String,
    pub status: TodoStatus,
}

impl Todo {
    pub fn new(title: String, description: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            title,
            description,
            status: TodoStatus::Waiting,
        }
    }

    pub fn set_status(&mut self, status: TodoStatus) {
        self.status = status;
    }
}

pub struct AgentState {
    pub todos: Vec<Todo>,
}

impl AgentState {
    pub fn add_todo(&mut self, title: String, description: String) -> &Todo {
        let todo = Todo::new(title, description);
        self.todos.push(todo);
        self.todos.last().unwrap()
    }

    pub fn update_todo_status(&mut self, id: uuid::Uuid, status: TodoStatus) -> Result<(), String> {
        match self.todos.iter_mut().find(|todo| todo.id == id) {
            Some(todo) => {
                todo.set_status(status);
                Ok(())
            }
            None => Err("Todo not found".to_string()),
        }
    }

    pub fn get_todo(&self, id: uuid::Uuid) -> Option<&Todo> {
        self.todos.iter().find(|todo| todo.id == id)
    }

    pub fn list_todos(&self) -> &Vec<Todo> {
        &self.todos
    }
}

pub trait Agent {
    fn todos(&self) -> &[Todo];
    fn evaluate(&mut self, chat_messages: Vec<but_action::ChatMessage>) -> anyhow::Result<String>;
}

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
}

impl<'a> ButBot<'a> {
    pub fn new(
        ctx: &'a mut CommandContext,
        emitter: std::sync::Arc<but_tools::emit::Emitter>,
        message_id: String,
        project_id: ProjectId,
        openai: &'a OpenAiProvider,
    ) -> Self {
        Self {
            state: AgentState { todos: Vec::new() },
            ctx,
            emitter,
            message_id,
            project_id,
            openai,
        }
    }
}

impl Agent for ButBot<'_> {
    fn todos(&self) -> &[Todo] {
        self.state.list_todos()
    }

    fn evaluate(&mut self, chat_messages: Vec<but_action::ChatMessage>) -> anyhow::Result<String> {
        let repo = self.ctx.gix_repo()?;
        let project_status = but_tools::workspace::get_project_status(self.ctx, &repo, None)?;
        let serialized_status = serde_json::to_string_pretty(&project_status)
            .map_err(|e| anyhow::anyhow!("Failed to serialize project status: {}", e))?;

        let mut toolset = but_tools::workspace::workspace_toolset(
            self.ctx,
            self.emitter.clone(),
            self.message_id.clone(),
        )?;

        let mut internal_chat_messages = chat_messages;

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

        let response = but_action::tool_calling_loop_stream(
            self.openai,
            SYS_PROMPT,
            internal_chat_messages,
            &mut toolset,
            Some(MODEL.to_string()),
            on_token_cb,
        )?;

        Ok(response.unwrap_or_default())
    }
}

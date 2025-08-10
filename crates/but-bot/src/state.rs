use std::{collections::BTreeMap, fmt::Display};

use but_tools::tool::{Tool, Toolset};
use gitbutler_command_context::CommandContext;
use gix::ObjectId;
use schemars::{JsonSchema, schema_for};
use serde_json::json;

const SYS_PROMPT: &str = "
You are a GitButler agent that can perform various actions on a Git project.
Your name is ButBot. Your main goal is to determine the set of steps needed to complete the user's request.
ONLY UPDATE THE STATE IF THERE ARE USER REQUESTS PENDING OR IF THERE ARE TODOS TO BE UPDATED.

### Core concepts
- **Project**: A Git repository that has been initialized with GitButler.
- **Stack**: A collection of dependent branches that are used to manage changes in the project. With GitButler (as opposed to normal Git), multiple stacks can be applied at the same time.
- **Branch**: A pointer to a specific commit in the project. Branches can contain multiple commits. Commits are always listed newest to oldest.
- **Commit**: A snapshot of the project at a specific point in time.
- **File changes**: A set of changes made to the files in the project. This can include additions, deletions, and modifications of files. The user can assign these changes to stacks to keep things ordering.
- **Lock**: A lock or dependency on a file change. This refers to the fact that certain uncommitted file changes can only be committed to a specific stack.
    This is because the uncommitted changes were done on top of previously committed file changes that are part of the stack.

### Main task
Based on the provided conversation, the project status and any existinf todos, update the list of todos.
Take a look at your capabilities in order to populate the todo list.
Ideally, before todos that perform actions, you should add todos with clear planning steps, leveraging the tools like ('get_branch_changes', and 'get_commit_details') to gather the necessary information.

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
- Only add items to the list if there's a clear user request to perform an action.
- Only add items to the todo list that are relevant to the user's request.
- Update the todo list depending on the project status.
- If there are todos that seem outdated based onf the project status, or user messages, update them accordingly.
- Only respond with 'done'.
";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TodoStatus {
    Waiting,
    InProgress,
    Success { message: String },
    Failed { message: String },
}

impl Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoStatus::Waiting => write!(f, "Waiting"),
            TodoStatus::InProgress => write!(f, "InProgress"),
            TodoStatus::Success { message } => write!(f, "Success({})", message),
            TodoStatus::Failed { message } => write!(f, "Failed({})", message),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Todo {
    pub id: uuid::Uuid,
    pub title: String,
    pub description: String,
    pub status: TodoStatus,
}

impl Display for Todo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Todo(\nid: {},\ntitle: {},\ndescription: {:?},\nstatus: {}\n)",
            self.id, self.title, self.description, self.status
        )
    }
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

    pub fn as_prompt(&self) -> String {
        format!(
            "Please complete the following todo: {}\nDescription: {}\nStatus: {}",
            self.title, self.description, self.status
        )
    }
}

pub struct AgentState {
    pub todos: Vec<Todo>,
    pub sys_prompt: String,
    tools: BTreeMap<String, std::sync::Arc<dyn Tool>>,
}

impl Default for AgentState {
    fn default() -> Self {
        let mut state = AgentState {
            todos: Vec::new(),
            sys_prompt: SYS_PROMPT.to_string(),
            tools: BTreeMap::new(),
        };

        state.register_tool(AddTodos);
        state.register_tool(UpdateTodoStatus);

        state
    }
}

impl AgentState {
    pub fn nothig_to_do(&self) -> bool {
        self.todos.is_empty()
    }

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

    pub fn next_todo(&mut self) -> Option<Todo> {
        let next_todo = self
            .todos
            .iter_mut()
            .find(|todo| matches!(todo.status, TodoStatus::Waiting | TodoStatus::InProgress));

        next_todo.map(|todo| {
            todo.set_status(TodoStatus::InProgress);
            todo.to_owned()
        })
    }

    fn call_tool_inner(
        &mut self,
        name: &str,
        parameters: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let params: serde_json::Value = serde_json::from_str(parameters)
            .map_err(|e| anyhow::anyhow!("Failed to parse parameters: {}", e))?;
        let state_tool = AgentStateTool::try_from((name, params))?;

        match state_tool {
            AgentStateTool::AddTodos(params) => {
                for todo in params.todos {
                    self.add_todo(todo.title, todo.description);
                }

                Ok(json!({"status": "success", "message": "Todos added successfully"}))
            }
            AgentStateTool::UpdateTodoStatus(params) => {
                let uuid = uuid::Uuid::parse_str(&params.id)
                    .map_err(|e| anyhow::anyhow!("Failed to parse UUID: {}", e))?;
                self.update_todo_status(uuid, params.status)
                    .map_err(|e| anyhow::anyhow!("Failed to update todo status: {}", e))?;

                Ok(json!({"status": "success", "message": "Todo status updated successfully"}))
            }
        }
    }
}

impl Toolset for AgentState {
    fn register_tool<T: Tool>(&mut self, tool: T) {
        self.tools.insert(tool.name(), std::sync::Arc::new(tool));
    }

    fn get(&self, name: &str) -> Option<std::sync::Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    fn list(&self) -> Vec<std::sync::Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    fn call_tool(&mut self, name: &str, parameters: &str) -> serde_json::Value {
        self.call_tool_inner(name, parameters).unwrap_or_else(|e| {
            serde_json::json!({
                "error": format!("Failed to call tool '{}': {}", name, e.to_string())
            })
        })
    }
}

#[derive(Debug, Clone)]
enum AgentStateTool {
    AddTodos(AddTodosParamters),
    UpdateTodoStatus(UpdateTodoStatusParameters),
}

impl TryFrom<(&str, serde_json::Value)> for AgentStateTool {
    type Error = anyhow::Error;

    fn try_from(value: (&str, serde_json::Value)) -> Result<Self, Self::Error> {
        match value.0 {
            "add_todos" => {
                let params: AddTodosParamters = serde_json::from_value(value.1)
                    .map_err(|e| anyhow::anyhow!("Failed to parse parameters: {}", e))?;
                Ok(AgentStateTool::AddTodos(params))
            }
            "update_todo_status" => {
                let params: UpdateTodoStatusParameters = serde_json::from_value(value.1)
                    .map_err(|e| anyhow::anyhow!("Failed to parse parameters: {}", e))?;
                Ok(AgentStateTool::UpdateTodoStatus(params))
            }
            _ => Err(anyhow::anyhow!("Unknown tool: {}", value.0)),
        }
    }
}

pub struct AddTodos;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoDescription {
    /// The title of the todo item.
    #[schemars(description = "
    <description>Title of the todo item</description>

    <important_notes>
        This is a brief title that summarizes the todo item.
        It should be concise and descriptive.
    </important_notes>
    ")]
    title: String,
    /// A detailed description of the todo item.
    #[schemars(description = "
    <description>The description of the todo</description>

    <important_notes>
        This should provide more context about the todo item.
        Mention any specific requirements and relevant resources (branch names, commit ids, files etc.).
        It should have a clear defintion of done.
        Keep the description to under 1000 characters.
    </important_notes>
    ")]
    description: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddTodosParamters {
    /// The list of todos to add.
    #[schemars(description = "
    <description>List of todos to add</description>

    <important_notes>
        Each todo should have a title and a description.
        The description should be clear and concise.
        Prefer using multiple todos for different tasks, with clear objectives.
    </important_notes>
    ")]
    pub todos: Vec<TodoDescription>,
}

impl Tool for AddTodos {
    fn name(&self) -> String {
        "add_todos".to_string()
    }

    fn description(&self) -> String {
        "Adds todos to the agent's state".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        schema_for!(AddTodosParamters).into()
    }

    fn call(
        self: std::sync::Arc<Self>,
        _: serde_json::Value,
        _: &mut CommandContext,
        _: std::sync::Arc<but_tools::emit::Emitter>,
        _: &mut std::collections::HashMap<ObjectId, ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        Err(anyhow::anyhow!(
            "The tool AddTodos doesn't support contextual calls."
        ))
    }
}

pub struct UpdateTodoStatus;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTodoStatusParameters {
    /// The id of the todo item to update.
    #[schemars(description = "The UUID of the todo item to update.")]
    pub id: String,
    /// The new status for the todo item.
    #[schemars(
        description = "The new status for the todo item. One of: Waiting, InProgress, Success, Failed. If Success or Failed, provide a message."
    )]
    pub status: TodoStatus,
}

impl Tool for UpdateTodoStatus {
    fn name(&self) -> String {
        "update_todo_status".to_string()
    }

    fn description(&self) -> String {
        "Updates the status of a todo item in the agent's state".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        schema_for!(UpdateTodoStatusParameters).into()
    }

    fn call(
        self: std::sync::Arc<Self>,
        _: serde_json::Value,
        _: &mut CommandContext,
        _: std::sync::Arc<but_tools::emit::Emitter>,
        _: &mut std::collections::HashMap<ObjectId, ObjectId>,
    ) -> anyhow::Result<serde_json::Value> {
        Err(anyhow::anyhow!(
            "The tool UpdateTodoStatus doesn't support contextual calls."
        ))
    }
}

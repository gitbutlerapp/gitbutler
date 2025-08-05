use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};

pub mod db;
pub mod handler;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRule {
    /// A UUID unique identifier for the rule.
    id: String,
    /// The time when the rule was created, represented as a Unix timestamp in milliseconds.
    created_at: chrono::NaiveDateTime,
    /// Whether the rule is currently enabled or not.
    enabled: bool,
    /// The trigger of the rule is what causes it to be evaluated in the app.
    trigger: Trigger,
    /// These filtes determine what files or changes the rule applies to.
    /// Within a rule, multiple filters are combined with AND logic (i.e. all conditions must be met).
    /// This allows for the expressions of rules like "If a file is modified, its path matches
    /// the regex 'src/.*', and its content matches the regex 'TODO', then do something."
    filters: Vec<Filter>,
    /// The action determines what happens to the files or changes that matched the filters.
    action: Action,
}

/// Represents the kinds of events in the app that can cause a rule to be evaluated.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// When a file is added, removed or modified in the Git worktree.
    FileSytemChange,
}

/// A filter is a condition that determines what files or changes the rule applies to.
/// Within a filter, multiple conditions are combined with AND logic (i.e. to match all conditions must be met)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum Filter {
    /// Matches the file path (relative to the repository root).
    #[serde(with = "serde_regex")]
    PathMatchesRegex(regex::Regex),
    /// Match the file content.
    #[serde(with = "serde_regex")]
    ContentMatchesRegex(regex::Regex),
    /// Matches the file change operation type (e.g. addition, deletion, modification, rename)
    FileChangeType(TreeStatus),
    /// Matches the semantic type of the change.
    SemanticType(SemanticType),
}

/// Represents the type of change that occurred in the Git worktree.
/// Matches the TreeStatus of the TreeChange
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TreeStatus {
    /// Something was added or scheduled to be added.
    Addition,
    /// Something was deleted.
    Deletion,
    /// A tracked entry was modified, which might mean.
    Modification,
    /// An entry was renamed.
    Rename,
}

/// Represents a semantic type of change that was inferred for the change.
/// Typically this means a heuristic or an LLM determinded that a change represents a refactor, a new feature, a bug fix, or documentation update.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum SemanticType {
    /// A change that is a refactor, meaning it does not change the external behavior of the code but improves its structure.
    Refactor,
    /// A change that introduces a new feature or functionality to the codebase.
    NewFeature,
    /// A change that fixes a bug or an issue in the code.
    BugFix,
    /// A change that updates or adds documentation, such as code inline docs, comments or README files.
    Documentation,
    /// A change that is not recognized or does not fit into the predefined categories.
    UserDefined(String),
}

/// Represents an action that can be taken based on the rule evaluation.
/// An action can be either explicit, meaning the user defined something like "Assign in Lane A" or "Ammend into Commit X"
/// or it is implicit, meaning the action was determined by heuristics or AI, such as "Assign to appropriate branch" or "Absorb in dependent commit".
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum Action {
    /// An action that has an explicit operation defined by the user.
    Explicit(Operation),
    /// An action where the operation is determined by heuristics or AI.
    Implicit(ImplicitOperation),
}

/// Represents the operation that a user can configure to be performed in an explicit action.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum Operation {
    /// Assign the matched changes to a specific stack ID.
    Assign { target: StackTarget },
    /// Amend the matched changes into a specific commit.
    Amend { commit_id: String },
    /// Create a new commit with the matched changes on a specific branch.
    NewCommit { branch_name: String },
}

/// The target stack for a given operation. It's either specifying a specific stack ID, or alternaitvely the leftmost or rightmost stack in the workspace.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum StackTarget {
    StackId(String),
    Leftmost,
    Rightmost,
}

/// Represents the implicit operation that is determined by heuristics or AI.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum ImplicitOperation {
    /// Assign the matched changes to the appropriate branch based on offline heuristics.
    AssignToAppropriateBranch,
    /// Absorb the matched changes into a dependent commit based on offline heuristics.
    AbsorbIntoDependentCommit,
    /// Perform an operation based on LLM-driven analysis and tool calling.
    LLMPrompt(String),
}

/// A request to create a new workspace rule.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateRuleRequest {
    /// The trigger that causes the rule to be evaluated.
    pub trigger: Trigger,
    /// The filters that determine what files or changes the rule applies to. If left empty, all files will be matched
    pub filters: Vec<Filter>,
    /// The action that determines what happens to the files or changes that matched the filters.
    pub action: Action,
}

/// Creates a new workspace rule
pub fn create_rule(
    ctx: &mut CommandContext,
    req: CreateRuleRequest,
) -> anyhow::Result<WorkspaceRule> {
    let rule = WorkspaceRule {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: chrono::Local::now().naive_local(),
        enabled: true,
        trigger: req.trigger,
        filters: req.filters,
        action: req.action,
    };

    ctx.db()?
        .workspace_rules()
        .insert(rule.clone().try_into()?)
        .map_err(|e| anyhow::anyhow!("Failed to insert workspace rule: {}", e))?;
    Ok(rule)
}

/// Deletes an existing workspace rule by its ID.
pub fn delete_rule(ctx: &mut CommandContext, id: &str) -> anyhow::Result<()> {
    ctx.db()?
        .workspace_rules()
        .delete(id)
        .map_err(|e| anyhow::anyhow!("Failed to delete workspace rule: {}", e))?;
    Ok(())
}

/// A request to update an existing workspace rule.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRuleRequest {
    /// The ID of the rule to update.
    id: String,
    /// The new enabled state of the rule. If not provided, the existing state is retained.
    enabled: Option<bool>,
    /// The new trigger for the rule. If not provided, the existing trigger is retained.
    trigger: Option<Trigger>,
    /// The new filters for the rule. If not provided, the existing filters are retained.
    filters: Option<Vec<Filter>>,
    /// The new action for the rule. If not provided, the existing action is retained.
    action: Option<Action>,
}

/// Updates an existing workspace rule with the provided request data.
pub fn update_rule(
    ctx: &mut CommandContext,
    req: UpdateRuleRequest,
) -> anyhow::Result<WorkspaceRule> {
    let mut rule: WorkspaceRule = ctx
        .db()?
        .workspace_rules()
        .get(&req.id)?
        .ok_or_else(|| anyhow::anyhow!("Rule with ID {} not found", req.id))?
        .try_into()?;

    if let Some(enabled) = req.enabled {
        rule.enabled = enabled;
    }
    if let Some(trigger) = req.trigger {
        rule.trigger = trigger;
    }
    if let Some(filters) = req.filters {
        rule.filters = filters;
    }
    if let Some(action) = req.action {
        rule.action = action;
    }

    ctx.db()?
        .workspace_rules()
        .update(&req.id, rule.clone().try_into()?)
        .map_err(|e| anyhow::anyhow!("Failed to update workspace rule: {}", e))?;
    Ok(rule)
}

/// Lists all workspace rules in the database.
pub fn list_rules(ctx: &mut CommandContext) -> anyhow::Result<Vec<WorkspaceRule>> {
    let rules = ctx
        .db()?
        .workspace_rules()
        .list()?
        .into_iter()
        .map(|r| r.try_into())
        .collect::<Result<Vec<WorkspaceRule>, _>>()?;
    Ok(rules)
}

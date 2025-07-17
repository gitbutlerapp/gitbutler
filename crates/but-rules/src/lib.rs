use serde::{Deserialize, Serialize};

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
    /// Within a rule, multiple filters are combined with OR logic (i.e. it's sufficient to match any of the filters)
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
#[serde(rename_all = "camelCase")]
pub enum Filter {
    /// Matches the file path (relative to the repository root) against all provided regex patterns.
    #[serde(with = "serde_regex")]
    PathMatchesRegex(Vec<regex::Regex>),
    /// Match the file content against all provided regex patterns.
    #[serde(with = "serde_regex")]
    ContentMatchesRegex(Vec<regex::Regex>),
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// An action that has an explicit operation defined by the user.
    Explicit(Operation),
    /// An action where the operation is determined by heuristics or AI.
    Implicit(ImplicitOperation),
}

/// Represents the operation that a user can configure to be performed in an explicit action.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    /// Assign the matched changes to a specific stack ID.
    Assign { stack_id: String },
    /// Amend the matched changes into a specific commit.
    Amend { commit_id: String },
    /// Create a new commit with the matched changes on a specific branch.
    NewCommit { branch_name: String },
}

/// Represents the implicit operation that is determined by heuristics or AI.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ImplicitOperation {
    /// Assign the matched changes to the appropriate branch based on offline heuristics.
    AssignToAppropriateBranch,
    /// Absorb the matched changes into a dependent commit based on offline heuristics.
    AbsorbIntoDependentCommit,
    /// Perform an operation based on LLM-driven analysis and tool calling.
    LLMPrompt(String),
}

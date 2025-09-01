use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};

/// A wrapper around gix-glob pattern that implements serde traits
#[derive(Debug, Clone)]
pub struct GlobPattern {
    pattern: gix::glob::Pattern,
    case_sensitive: bool,
}

impl GlobPattern {
    pub fn new(pattern: &str, case_sensitive: bool) -> Result<Self, String> {
        let pattern = gix::glob::Pattern::from_bytes(pattern.as_bytes())
            .ok_or_else(|| "Empty pattern".to_string())?;
        Ok(Self {
            pattern,
            case_sensitive,
        })
    }

    pub fn is_match(&self, text: &str) -> bool {
        use bstr::BStr;
        let value = BStr::new(text.as_bytes());
        let case = if self.case_sensitive {
            gix::glob::pattern::Case::Sensitive
        } else {
            gix::glob::pattern::Case::Fold
        };
        
        // Use matches_repo_relative_path for proper case handling
        self.pattern.matches_repo_relative_path(
            value,
            None, // basename_start_pos
            None, // is_dir
            case,
            gix::glob::wildmatch::Mode::NO_MATCH_SLASH_LITERAL
        )
    }

    pub fn pattern_str(&self) -> String {
        String::from_utf8_lossy(&self.pattern.text).to_string()
    }
}

impl Serialize for GlobPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("GlobPattern", 2)?;
        state.serialize_field("pattern", &self.pattern_str())?;
        state.serialize_field("case_sensitive", &self.case_sensitive)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for GlobPattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct GlobPatternData {
            pattern: String,
            case_sensitive: bool,
        }

        let data = GlobPatternData::deserialize(deserializer)?;
        Self::new(&data.pattern, data.case_sensitive).map_err(serde::de::Error::custom)
    }
}

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
    /// the glob pattern 'src/**/*.rs', and its content matches the regex 'TODO', then do something."
    filters: Vec<Filter>,
    /// The action determines what happens to the files or changes that matched the filters.
    action: Action,
}

impl WorkspaceRule {
    /// If the rule has a session ID filter, this returns the first one found.
    pub fn session_id(&self) -> Option<String> {
        self.filters.iter().find_map(|f| match f {
            Filter::ClaudeCodeSessionId(id) => Some(id.clone()),
            _ => None,
        })
    }

    /// Returns the target stack ID if the action is an explicit assignment operation.
    pub fn target_stack_id(&self) -> Option<String> {
        if let Action::Explicit(Operation::Assign { target }) = &self.action {
            match target {
                StackTarget::StackId(id) => Some(id.clone()),
                StackTarget::Leftmost | StackTarget::Rightmost => None,
            }
        } else {
            None
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn created_at(&self) -> chrono::NaiveDateTime {
        self.created_at
    }
}

/// Represents the kinds of events in the app that can cause a rule to be evaluated.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// When a file is added, removed or modified in the Git worktree.
    FileSytemChange,
    /// Whenever a Claude Code hook is invoked.
    ClaudeCodeHook,
}

/// A filter is a condition that determines what files or changes the rule applies to.
/// Within a filter, multiple conditions are combined with AND logic (i.e. to match all conditions must be met)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum Filter {
    /// Matches the file path (relative to the repository root) using glob patterns.
    PathMatchesGlob(GlobPattern),
    /// Match the file content using regular expressions.
    #[serde(with = "serde_regex")]
    ContentMatchesRegex(regex::Regex),
    /// Matches the file change operation type (e.g. addition, deletion, modification, rename)
    FileChangeType(TreeStatus),
    /// Matches the semantic type of the change.
    SemanticType(SemanticType),
    /// Matches changes that originated from a specific Claude Code session.
    ClaudeCodeSessionId(String),
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
    process_rules(ctx).ok(); // Reevaluate rules after creating
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
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRuleRequest {
    /// The ID of the rule to update.
    id: String,
    /// The new enabled state of the rule. If not provided, the existing state is retained.
    pub enabled: Option<bool>,
    /// The new trigger for the rule. If not provided, the existing trigger is retained.
    pub trigger: Option<Trigger>,
    /// The new filters for the rule. If not provided, the existing filters are retained.
    pub filters: Option<Vec<Filter>>,
    /// The new action for the rule. If not provided, the existing action is retained.
    pub action: Option<Action>,
}

impl From<WorkspaceRule> for UpdateRuleRequest {
    fn from(rule: WorkspaceRule) -> Self {
        UpdateRuleRequest {
            id: rule.id,
            enabled: Some(rule.enabled),
            trigger: Some(rule.trigger),
            filters: Some(rule.filters),
            action: Some(rule.action),
        }
    }
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
    process_rules(ctx).ok(); // Reevaluate rules after updating
    Ok(rule)
}

/// Retrieves a workspace rule by its ID.
pub fn get_rule(ctx: &mut CommandContext, id: &str) -> anyhow::Result<WorkspaceRule> {
    let rule = ctx
        .db()?
        .workspace_rules()
        .get(id)?
        .ok_or_else(|| anyhow::anyhow!("Rule with ID {} not found", id))?
        .try_into()?;
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

fn process_rules(ctx: &mut CommandContext) -> anyhow::Result<()> {
    let wt_changes = but_core::diff::worktree_changes(&ctx.gix_repo()?)?;

    let dependencies = hunk_dependencies_for_workspace_changes_by_worktree_dir(
        ctx,
        &ctx.project().path,
        &ctx.project().gb_dir(),
        Some(wt_changes.changes.clone()),
    )?;

    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        false,
        Some(wt_changes.changes),
        Some(&dependencies),
    )
    .map_err(|e| anyhow::anyhow!("Failed to get assignments: {}", e))?;

    handler::process_workspace_rules(ctx, &assignments, &Some(dependencies))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_pattern_basic_matching() {
        // Test basic glob pattern matching
        let pattern = GlobPattern::new("*.rs", true).unwrap();
        assert!(pattern.is_match("main.rs"));
        assert!(pattern.is_match("lib.rs"));
        assert!(!pattern.is_match("main.txt"));
        assert!(!pattern.is_match("main"));
    }

    #[test]
    fn test_glob_pattern_case_sensitive() {
        let pattern_sensitive = GlobPattern::new("*.RS", true).unwrap();
        let pattern_insensitive = GlobPattern::new("*.RS", false).unwrap();
        
        assert!(!pattern_sensitive.is_match("main.rs")); // Case sensitive, should not match
        assert!(pattern_insensitive.is_match("main.rs")); // Case insensitive, should match
        assert!(pattern_sensitive.is_match("main.RS")); // Exact case match
        assert!(pattern_insensitive.is_match("main.RS")); // Exact case match
    }

    #[test]
    fn test_glob_pattern_path_matching() {
        let pattern = GlobPattern::new("src/**/*.rs", true).unwrap();
        assert!(pattern.is_match("src/main.rs"));
        assert!(pattern.is_match("src/lib/mod.rs"));
        assert!(pattern.is_match("src/deep/nested/file.rs"));
        assert!(!pattern.is_match("tests/main.rs"));
        assert!(!pattern.is_match("src/main.txt"));
    }

    #[test]
    fn test_glob_pattern_serialization() {
        let pattern = GlobPattern::new("*.rs", true).unwrap();
        let serialized = serde_json::to_string(&pattern).unwrap();
        let deserialized: GlobPattern = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(pattern.pattern_str(), deserialized.pattern_str());
        assert_eq!(pattern.case_sensitive, deserialized.case_sensitive);
        assert!(deserialized.is_match("main.rs"));
    }

    #[test]
    fn test_filter_path_matches_glob() {
        let pattern = GlobPattern::new("*.rs", true).unwrap();
        let filter = Filter::PathMatchesGlob(pattern);
        
        // Test that the filter can be serialized and deserialized
        let serialized = serde_json::to_string(&filter).unwrap();
        let deserialized: Filter = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            Filter::PathMatchesGlob(glob) => {
                assert!(glob.is_match("main.rs"));
                assert!(!glob.is_match("main.txt"));
            }
            _ => panic!("Expected PathMatchesGlob filter"),
        }
    }



    #[test]
    fn test_core_ignore_case_integration() {
        // Test that patterns respect case sensitivity configuration
        let case_sensitive_pattern = GlobPattern::new("*.RS", true).unwrap();
        let case_insensitive_pattern = GlobPattern::new("*.RS", false).unwrap();
        
        // Case sensitive should not match lowercase extension
        assert!(!case_sensitive_pattern.is_match("main.rs"));
        assert!(case_sensitive_pattern.is_match("main.RS"));
        
        // Case insensitive should match both
        assert!(case_insensitive_pattern.is_match("main.rs"));
        assert!(case_insensitive_pattern.is_match("main.RS"));
        assert!(case_insensitive_pattern.is_match("main.Rs"));
    }
}

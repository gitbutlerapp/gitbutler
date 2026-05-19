use but_core::{ChangeId, RefMetadata, sync::RepoExclusive};
use but_ctx::Context;
use but_db::DbHandle;
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

    /// Return the target change ID if its action is an explicit amend operation.
    pub fn target_change_id(&self) -> Option<ChangeId> {
        if let Action::Explicit(Operation::Amend { change_id }) = &self.action {
            Some(change_id.clone())
        } else {
            None
        }
    }

    /// Return the persistent rule ID.
    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Return the creation timestamp.
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
/// Typically this means a heuristic or an LLM determined that a change represents a refactor, a new feature, a bug fix, or documentation update.
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
/// An action can be either explicit, meaning the user defined something like "Stage to Lane A" or "Amend into Commit X"
/// or it is implicit, meaning the action was determined by heuristics or AI, such as "Stage to appropriate branch" or "Absorb in dependent commit".
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
    Amend { change_id: ChangeId },
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

/// Create a new workspace rule and attempt to reevaluate all workspace rules.
///
/// `ctx` provides database access for insertion and repository/workspace state for reevaluation.
/// `req` contains the trigger, filters, and action to persist. `perm` is the caller-held exclusive
/// worktree permission used while reevaluating rules after the insert.
pub fn create_rule(
    ctx: &mut Context,
    req: CreateRuleRequest,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceRule> {
    let rule = {
        let mut db = ctx.db.get_cache_mut()?;
        insert_rule(&mut db, req)?
    };
    process_rules_from_context(ctx, perm).ok(); // Reevaluate rules after creating
    Ok(rule)
}

/// Insert a new workspace rule record into `db` from `req`.
fn insert_rule(db: &mut DbHandle, req: CreateRuleRequest) -> anyhow::Result<WorkspaceRule> {
    let rule = WorkspaceRule {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: chrono::Local::now().naive_local(),
        enabled: true,
        trigger: req.trigger,
        filters: req.filters,
        action: req.action,
    };

    db.workspace_rules_mut().insert(rule.clone().try_into()?)?;
    Ok(rule)
}

/// Delete the workspace rule with `id` from `db`.
pub fn delete_rule(db: &mut DbHandle, id: &str) -> anyhow::Result<()> {
    db.workspace_rules_mut().delete(id)?;
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

/// Update an existing workspace rule and attempt to reevaluate all workspace rules.
///
/// `ctx` provides database access for the update and repository/workspace state for reevaluation.
/// `req` contains the rule ID and optional replacement fields. `perm` is the caller-held exclusive
/// worktree permission used while reevaluating rules after the update.
pub fn update_rule(
    ctx: &mut Context,
    req: UpdateRuleRequest,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceRule> {
    let rule = {
        let mut db = ctx.db.get_cache_mut()?;
        update_rule_record(&mut db, req)?
    };
    process_rules_from_context(ctx, perm).ok(); // Reevaluate rules after updating
    Ok(rule)
}

/// Apply `req` to an existing workspace rule record in `db`.
///
/// Fields omitted from `req` retain their current values.
fn update_rule_record(db: &mut DbHandle, req: UpdateRuleRequest) -> anyhow::Result<WorkspaceRule> {
    let mut rule: WorkspaceRule = {
        db.workspace_rules()
            .get(&req.id)?
            .ok_or_else(|| anyhow::anyhow!("Rule with ID {} not found", req.id))?
            .try_into()?
    };

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

    db.workspace_rules_mut()
        .update(&req.id, rule.clone().try_into()?)?;
    Ok(rule)
}

/// Retrieve the workspace rule with `id` from `db`.
pub fn get_rule(db: &DbHandle, id: &str) -> anyhow::Result<WorkspaceRule> {
    let rule = db
        .workspace_rules()
        .get(id)?
        .ok_or_else(|| anyhow::anyhow!("Rule with ID {id} not found"))?
        .try_into()?;
    Ok(rule)
}

/// List all workspace rules stored in `db`.
pub fn list_rules(db: &DbHandle) -> anyhow::Result<Vec<WorkspaceRule>> {
    let rules = db
        .workspace_rules()
        .list()?
        .into_iter()
        .map(|r| r.try_into())
        .collect::<Result<Vec<WorkspaceRule>, _>>()?;
    Ok(rules)
}

/// Reevaluate workspace rules using state extracted from `ctx`.
///
/// `ctx` provides rules from the database, settings, metadata, repository, workspace, and hunk
/// assignment storage. `perm` is the caller-held exclusive worktree permission used for workspace
/// access and any rule action that mutates workspace state.
///
/// NOTE: may create an empty branch!
fn process_rules_from_context(ctx: &mut Context, perm: &mut RepoExclusive) -> anyhow::Result<()> {
    let context_lines = ctx.settings.context_lines;
    let mut meta = ctx.meta()?;
    let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let rules = list_rules(&db)?;
    process_rules(
        rules,
        &repo,
        &mut ws,
        &mut db,
        &mut meta,
        perm,
        context_lines,
    )
}

/// Reevaluate `rules` against current worktree changes and apply matching actions.
///
/// `rules` are the workspace rules to evaluate. `repo` is used to read worktree changes and create
/// or amend commits. `ws` is the mutable workspace view that rule actions inspect and update. `db`
/// provides hunk assignment storage. `meta` provides mutable ref metadata for stack creation and
/// rebase operations. `perm` is the caller-held exclusive worktree permission for workspace
/// mutations. `context_lines` controls the diff context used while deriving hunk assignments and
/// applying rule actions.
///
/// NOTE: may create an empty branch!
pub fn process_rules(
    rules: Vec<WorkspaceRule>,
    repo: &gix::Repository,
    ws: &mut but_graph::Workspace,
    db: &mut DbHandle,
    meta: &mut impl RefMetadata,
    perm: &mut RepoExclusive,
    context_lines: u32,
) -> anyhow::Result<()> {
    let assignments = {
        let wt_changes = but_core::diff::worktree_changes(repo)?;

        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            repo,
            ws,
            Some(wt_changes.changes),
            context_lines,
        )
        .map_err(|e| anyhow::anyhow!("Failed to get assignments: {e}"))?;
        assignments
    };

    handler::process_workspace_rules(rules, &assignments, repo, ws, db, meta, perm, context_lines)?;
    Ok(())
}

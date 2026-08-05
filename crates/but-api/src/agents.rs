//! Managing coding-agent skills and the `but` CLI symlink from a GUI.
//!
//! These mirror what `but agent setup` and `but skill` do interactively, but
//! report structured state instead of prompting, so a settings screen can show
//! what is installed and change it one framework at a time.

use anyhow::{Context as _, Result};
use but_api_macros::but_api;
use but_ctx::ProjectHandleOrLegacyProjectId;
use but_skill::{
    Scope,
    framework::{FRAMEWORKS, framework_by_id},
    plan::join_components,
    policy::{WizardAnswers, WorkflowOption},
};
use tracing::instrument;

use json::{
    AgentFramework, AgentsStatus, CliInstallState, FrameworkScopeState, InstalledSkill,
    PolicyOptions, PolicySource, PolicyState, SkillScope, WorkflowOptionInfo,
};

/// Transport types for the agent-setup commands.
pub mod json {
    use serde::{Deserialize, Serialize};

    /// Where an agent artifact lives.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[cfg_attr(feature = "export-schema", schemars(extend("x-input" = true)))]
    #[serde(rename_all = "camelCase")]
    pub enum SkillScope {
        /// The user's home directory, applying to all their projects.
        Global,
        /// The current repository only.
        Repository,
    }

    impl From<SkillScope> for but_skill::Scope {
        fn from(value: SkillScope) -> Self {
            match value {
                SkillScope::Global => but_skill::Scope::Global,
                SkillScope::Repository => but_skill::Scope::Repository,
            }
        }
    }

    /// Everything the settings screen needs to render the agent list.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct AgentsStatus {
        /// The CLI version installed skills are compared against.
        pub cli_version: String,
        /// The user's home directory, when it can be determined.
        pub home_dir: Option<String>,
        /// The repository root, absent when the request had no project.
        pub repo_root: Option<String>,
        /// Every known framework, detected ones first.
        pub frameworks: Vec<AgentFramework>,
    }

    /// One coding agent and its per-scope skill state.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct AgentFramework {
        /// Stable identifier, and the key every mutating call takes.
        pub id: String,
        /// Display name.
        pub name: String,
        /// One-line description of what installing does.
        pub description: String,
        /// A config marker for this agent was found under `$HOME`.
        pub detected_globally: bool,
        /// A config marker was found in the project. Always false without one.
        pub detected_in_repo: bool,
        /// One entry per scope this framework can install into. Some agents
        /// are global-only, so this is not always both.
        pub scopes: Vec<FrameworkScopeState>,
    }

    /// What is installed for one framework at one scope.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct FrameworkScopeState {
        /// The scope this entry describes.
        pub scope: SkillScope,
        /// Where a fresh install would write.
        pub skill_path: String,
        /// The GitButler skill found here, if any. Identity comes from
        /// `SKILL.md` frontmatter, so the folder name may differ from
        /// `skill_path`.
        pub installed: Option<InstalledSkill>,
        /// The instruction file the managed policy block goes in. `None` when
        /// this agent has no supported file at this scope, in which case the
        /// policy can only be shown for manual copying.
        pub instruction_path: Option<String>,
        /// Whether that instruction file currently holds a managed block.
        pub has_managed_block: bool,
    }

    /// A discovered skill installation.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct InstalledSkill {
        /// Where it actually lives.
        pub path: String,
        /// Version from its `SKILL.md`, or "unknown".
        pub version: String,
        /// Whether it matches the running CLI version.
        pub up_to_date: bool,
    }

    /// The `but` CLI symlink, as the settings screen sees it.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct CliInstallState {
        /// The `but` binary this app links to.
        pub target_path: String,
        /// Whether that binary exists. False in a dev build that has not built
        /// `but` yet.
        pub target_exists: bool,
        /// The link location, absent on Windows.
        pub link_path: Option<String>,
        /// Whether the link is present and points at our binary.
        pub installed: bool,
        /// Set when the link cannot be managed: something that is not ours
        /// sits at the link path, or the platform has no symlink install.
        pub blocked_reason: Option<String>,
    }

    impl From<but_skill::cli_link::CliInstallState> for CliInstallState {
        fn from(value: but_skill::cli_link::CliInstallState) -> Self {
            use but_skill::cli_link::CliLinkStatus;
            let blocked_reason = match &value.status {
                CliLinkStatus::Installed | CliLinkStatus::NotInstalled => None,
                CliLinkStatus::InstalledElsewhere { actual } => {
                    Some(format!("A different `but` is linked here: {actual}"))
                }
                CliLinkStatus::Blocked => Some(
                    "A real file already exists here, so GitButler will not replace it.".into(),
                ),
                CliLinkStatus::Unsupported => {
                    Some("Automatic CLI installation is not supported on this platform.".into())
                }
            };
            Self {
                installed: value.is_installed(),
                target_path: value.target_path,
                target_exists: value.target_exists,
                link_path: value.link_path,
                blocked_reason,
            }
        }
    }

    /// One of the workflow preferences that render into the managed block.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[cfg_attr(feature = "export-schema", schemars(extend("x-input" = true)))]
    #[serde(rename_all = "camelCase")]
    pub enum WorkflowOptionId {
        /// Fold small follow-up fixes into the commit they belong to.
        FoldFixes,
        /// Suggest splitting large or mixed commits.
        SuggestSplits,
        /// Favor stacked branches and PRs for dependent work.
        StackedBranches,
        /// Update from the target branch automatically.
        AutoUpdate,
        /// Open pull requests as drafts by default.
        DraftPrs,
        /// Land onto the target instead of opening pull requests.
        PushToTarget,
        /// Publish everything on a shortcut phrase.
        PublishPhrase,
        /// Follow a preferred branch naming pattern.
        BranchPattern,
        /// Follow a preferred commit message convention.
        CommitConvention,
        /// Commit a checkpoint after each agent turn.
        CommitAfterTurn,
    }

    impl From<but_skill::policy::WorkflowOption> for WorkflowOptionId {
        fn from(value: but_skill::policy::WorkflowOption) -> Self {
            use but_skill::policy::WorkflowOption as W;
            match value {
                W::FoldFixes => Self::FoldFixes,
                W::SuggestSplits => Self::SuggestSplits,
                W::StackedBranches => Self::StackedBranches,
                W::AutoUpdate => Self::AutoUpdate,
                W::DraftPrs => Self::DraftPrs,
                W::PushToTarget => Self::PushToTarget,
                W::PublishPhrase => Self::PublishPhrase,
                W::BranchPattern => Self::BranchPattern,
                W::CommitConvention => Self::CommitConvention,
                W::CommitAfterTurn => Self::CommitAfterTurn,
            }
        }
    }

    impl From<WorkflowOptionId> for but_skill::policy::WorkflowOption {
        fn from(value: WorkflowOptionId) -> Self {
            use but_skill::policy::WorkflowOption as W;
            match value {
                WorkflowOptionId::FoldFixes => W::FoldFixes,
                WorkflowOptionId::SuggestSplits => W::SuggestSplits,
                WorkflowOptionId::StackedBranches => W::StackedBranches,
                WorkflowOptionId::AutoUpdate => W::AutoUpdate,
                WorkflowOptionId::DraftPrs => W::DraftPrs,
                WorkflowOptionId::PushToTarget => W::PushToTarget,
                WorkflowOptionId::PublishPhrase => W::PublishPhrase,
                WorkflowOptionId::BranchPattern => W::BranchPattern,
                WorkflowOptionId::CommitConvention => W::CommitConvention,
                WorkflowOptionId::CommitAfterTurn => W::CommitAfterTurn,
            }
        }
    }

    /// Static description of a workflow option, so the UI never hardcodes the
    /// wording.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct WorkflowOptionInfo {
        /// Which option this describes.
        pub id: WorkflowOptionId,
        /// Checkbox label.
        pub label: String,
        /// Explanatory help text.
        pub help: String,
        /// Whether a fresh setup enables it.
        pub default_selected: bool,
        /// Whether it only makes sense for a single repository. The policy is
        /// rendered once and written everywhere the setup targets, so a
        /// repo-local rule must not be offered for a global setup.
        pub repo_local_only: bool,
        /// Why it is unavailable outside repository scope, when it is.
        pub repo_local_help: Option<String>,
    }

    /// The workflow preferences themselves, as both input and output.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[cfg_attr(feature = "export-schema", schemars(extend("x-input" = true)))]
    #[serde(rename_all = "camelCase")]
    pub struct PolicyOptions {
        /// The enabled options.
        pub selected: Vec<WorkflowOptionId>,
        /// Phrase that means "publish everything".
        pub publish_phrase: String,
        /// Preferred branch naming pattern.
        pub branch_pattern: Option<String>,
        /// Preferred commit message convention.
        pub commit_convention: Option<String>,
    }

    /// One instruction file inspected while reading the policy.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct PolicySource {
        /// The file.
        pub path: String,
        /// Framework ids whose steering lives in it. Several agents share one
        /// `AGENTS.md`, so this is often more than one.
        pub frameworks: Vec<String>,
        /// Whether it currently holds a managed block.
        pub has_managed_block: bool,
        /// The options read out of that block.
        pub options: Option<PolicyOptions>,
    }

    /// The catalogue plus whatever the installed blocks currently say.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct PolicyState {
        /// Every option, so the UI renders from this rather than hardcoding.
        pub available: Vec<WorkflowOptionInfo>,
        /// The options currently installed, or `None` when no block exists.
        pub current: Option<PolicyOptions>,
        /// What a fresh setup would select, for pre-filling.
        pub defaults: PolicyOptions,
        /// Every instruction file inspected and what each held.
        pub sources: Vec<PolicySource>,
        /// Whether two inspected files hold blocks that disagree, in which
        /// case `current` is only the first of them.
        pub diverged: bool,
    }

    #[cfg(feature = "export-schema")]
    mod schema {
        but_schemars::register_sdk_type!(super::SkillScope);
        but_schemars::register_sdk_type!(super::AgentsStatus);
        but_schemars::register_sdk_type!(super::AgentFramework);
        but_schemars::register_sdk_type!(super::FrameworkScopeState);
        but_schemars::register_sdk_type!(super::InstalledSkill);
        but_schemars::register_sdk_type!(super::CliInstallState);
        but_schemars::register_sdk_type!(super::WorkflowOptionId);
        but_schemars::register_sdk_type!(super::WorkflowOptionInfo);
        but_schemars::register_sdk_type!(super::PolicyOptions);
        but_schemars::register_sdk_type!(super::PolicySource);
        but_schemars::register_sdk_type!(super::PolicyState);
    }
}

impl From<WizardAnswers> for PolicyOptions {
    fn from(value: WizardAnswers) -> Self {
        Self {
            selected: value.selected.into_iter().map(Into::into).collect(),
            publish_phrase: value.publish_phrase,
            branch_pattern: value.branch_pattern,
            commit_convention: value.commit_convention,
        }
    }
}

impl From<PolicyOptions> for WizardAnswers {
    fn from(value: PolicyOptions) -> Self {
        Self {
            selected: value.selected.into_iter().map(Into::into).collect(),
            publish_phrase: value.publish_phrase,
            branch_pattern: value.branch_pattern,
            commit_convention: value.commit_convention,
        }
    }
}

/// The repository worktree for `project_id`, or `None` for a global-only
/// request.
///
/// Resolved here rather than taken as a `Context` because these commands are
/// meaningful without a project at all — the global settings screen manages
/// `$HOME` skills before any repository is open.
fn repo_root(
    project_id: Option<ProjectHandleOrLegacyProjectId>,
) -> Result<Option<std::path::PathBuf>> {
    let Some(project_id) = project_id else {
        return Ok(None);
    };
    let ctx = but_ctx::Context::try_from(project_id)?;
    let repo = ctx.repo.get()?;
    let Some(workdir) = repo.workdir() else {
        return Ok(None);
    };
    // A repository discovered from a relative working directory reports a
    // relative workdir; absolutize it so reported paths are usable as-is.
    Ok(Some(std::path::absolute(workdir).with_context(|| {
        format!("Could not absolutize repository workdir {workdir:?}")
    })?))
}

/// The base directory a scope installs into.
fn base_dir(
    scope: Scope,
    home: Option<&std::path::Path>,
    root: Option<&std::path::Path>,
) -> Result<std::path::PathBuf> {
    match scope {
        Scope::Global => home
            .map(std::path::Path::to_path_buf)
            .context("Could not determine home directory"),
        Scope::Repository => root
            .map(std::path::Path::to_path_buf)
            .context("This action needs an open project"),
        Scope::Both => anyhow::bail!("BUG: Scope::Both has no single base directory"),
    }
}

/// Detect which agents the user works with and what GitButler has installed
/// for each, at global scope and — when a project is given — repository scope.
#[but_api]
#[instrument(err(Debug))]
pub fn agents_status(project_id: Option<ProjectHandleOrLegacyProjectId>) -> Result<AgentsStatus> {
    let root = repo_root(project_id)?;
    let home = but_path::home_dir();
    let repo_info = root.as_ref().map(|root| but_skill::RepoInfo {
        root: root.clone(),
        // Only used by the setup plan, which this read-only call does not build.
        needs_setup: false,
    });

    let mut frameworks: Vec<AgentFramework> = FRAMEWORKS
        .iter()
        .map(|framework| {
            let mut scopes = Vec::new();
            for (scope, dto_scope) in [
                (Scope::Global, SkillScope::Global),
                (Scope::Repository, SkillScope::Repository),
            ] {
                if !framework.supports(scope) {
                    continue;
                }
                let Ok(base) = base_dir(scope, home.as_deref(), root.as_deref()) else {
                    continue;
                };
                let Some(components) = framework.skill_path_components(scope) else {
                    continue;
                };
                let skill_path = join_components(&base, components);

                // Identity comes from frontmatter, so a custom folder name is
                // still found; report the first match rather than the
                // canonical path.
                let installed = framework
                    .format(matches!(scope, Scope::Global))
                    .map(|format| but_skill::status::find_format_installations(format, &base))
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .map(|path| {
                        let version =
                            but_skill::status::extract_installed_version(&path.join("SKILL.md"))
                                .unwrap_or_else(|| "unknown".to_string());
                        InstalledSkill {
                            up_to_date: but_skill::status::is_current_skill_installation(
                                &path,
                                but_skill::cli_version(),
                            ),
                            path: path.to_string_lossy().to_string(),
                            version,
                        }
                    });

                let instruction_path = framework
                    .instruction_components(scope)
                    .map(|components| join_components(&base, components));
                let has_managed_block = instruction_path.as_deref().is_some_and(|path| {
                    but_skill::files::read_managed_block_file(path)
                        .ok()
                        .flatten()
                        .is_some()
                });

                scopes.push(FrameworkScopeState {
                    scope: dto_scope,
                    skill_path: skill_path.to_string_lossy().to_string(),
                    installed,
                    instruction_path: instruction_path
                        .map(|path| path.to_string_lossy().to_string()),
                    has_managed_block,
                });
            }

            AgentFramework {
                id: framework.id.to_string(),
                name: framework.name.to_string(),
                description: framework.description.to_string(),
                detected_globally: framework.detected_globally(home.as_deref()),
                detected_in_repo: framework.detected_in_repo(repo_info.as_ref()),
                scopes,
            }
        })
        .collect();

    // Detected agents first so the settings list leads with what the user
    // actually uses; stable within each group so the order stays predictable.
    frameworks
        .sort_by_key(|framework| !(framework.detected_globally || framework.detected_in_repo));

    Ok(AgentsStatus {
        cli_version: but_skill::cli_version().to_string(),
        home_dir: home.map(|home| home.to_string_lossy().to_string()),
        repo_root: root.map(|root| root.to_string_lossy().to_string()),
        frameworks,
    })
}

/// Install or refresh the GitButler skill for one framework at one scope.
///
/// Returns the refreshed status so a UI needs a single round trip.
#[but_api]
#[instrument(err(Debug))]
pub fn agent_skill_install(
    framework_id: String,
    scope: SkillScope,
    project_id: Option<ProjectHandleOrLegacyProjectId>,
) -> Result<AgentsStatus> {
    let framework = framework_by_id(&framework_id)
        .with_context(|| format!("Unknown agent framework {framework_id}"))?;
    let scope: Scope = scope.into();
    let root = repo_root(project_id.clone())?;
    let base = base_dir(scope, but_path::home_dir().as_deref(), root.as_deref())?;
    let components = framework
        .skill_path_components(scope)
        .with_context(|| format!("{} has no skill format for this scope", framework.name))?;

    but_skill::install::write_skill_files(&join_components(&base, components))?;
    agents_status(project_id)
}

/// Rewrite outdated GitButler skills in place, bringing them up to the
/// running version.
///
/// `framework_id` limits this to one agent; `None` refreshes every outdated
/// installation at `scope`.
///
/// Updates the path each skill was *discovered* at rather than the canonical
/// install path. Those differ when a skill was installed into a custom folder
/// name, and writing to the canonical path would leave the outdated copy in
/// place and add a second one beside it.
#[but_api]
#[instrument(err(Debug))]
pub fn agent_skills_update(
    scope: SkillScope,
    project_id: Option<ProjectHandleOrLegacyProjectId>,
    framework_id: Option<String>,
) -> Result<AgentsStatus> {
    let scope: Scope = scope.into();
    let root = repo_root(project_id.clone())?;
    let base = base_dir(scope, but_path::home_dir().as_deref(), root.as_deref())?;
    let version = but_skill::cli_version();

    for framework in FRAMEWORKS {
        if framework_id.as_deref().is_some_and(|id| id != framework.id) {
            continue;
        }
        let Some(format) = framework.format(matches!(scope, Scope::Global)) else {
            continue;
        };
        for path in but_skill::status::find_format_installations(format, &base) {
            if but_skill::status::is_current_skill_installation(&path, version) {
                continue;
            }
            but_skill::install::write_skill_files(&path)?;
        }
    }

    agents_status(project_id)
}

/// Remove the GitButler skill for one framework at one scope.
///
/// `remove_instructions` additionally strips the managed policy block from
/// that framework's instruction file. It defaults to leaving the file alone,
/// because several agents share one `AGENTS.md` and the block may still be
/// serving them.
#[but_api]
#[instrument(err(Debug))]
pub fn agent_skill_uninstall(
    framework_id: String,
    scope: SkillScope,
    project_id: Option<ProjectHandleOrLegacyProjectId>,
    remove_instructions: Option<bool>,
) -> Result<AgentsStatus> {
    let framework = framework_by_id(&framework_id)
        .with_context(|| format!("Unknown agent framework {framework_id}"))?;
    let scope: Scope = scope.into();
    let root = repo_root(project_id.clone())?;
    let base = base_dir(scope, but_path::home_dir().as_deref(), root.as_deref())?;

    // Remove every installation discovered for this format, not just the
    // canonical path: a skill installed into a custom folder name is still
    // ours and would otherwise be left behind.
    if let Some(format) = framework.format(matches!(scope, Scope::Global)) {
        for path in but_skill::status::find_format_installations(format, &base) {
            but_skill::install::remove_skill_files(&path)?;
        }
    }

    if remove_instructions.unwrap_or(false)
        && let Some(components) = framework.instruction_components(scope)
    {
        but_skill::files::remove_managed_block_file(&join_components(&base, components))?;
    }

    agents_status(project_id)
}

/// Every instruction file in `scope`, and which frameworks share each one.
fn policy_sources(
    scope: Scope,
    base: &std::path::Path,
) -> Vec<(std::path::PathBuf, Vec<&'static str>)> {
    let mut by_path: std::collections::BTreeMap<std::path::PathBuf, Vec<&'static str>> =
        Default::default();
    for framework in FRAMEWORKS {
        if !framework.supports(scope) {
            continue;
        }
        if let Some(components) = framework.instruction_components(scope) {
            by_path
                .entry(join_components(base, components))
                .or_default()
                .push(framework.id);
        }
    }
    by_path.into_iter().collect()
}

/// Read the workflow policy currently installed at `scope`, along with the
/// catalogue of options and their defaults.
#[but_api]
#[instrument(err(Debug))]
pub fn agent_policy_get(
    scope: SkillScope,
    project_id: Option<ProjectHandleOrLegacyProjectId>,
) -> Result<PolicyState> {
    let scope: Scope = scope.into();
    let root = repo_root(project_id)?;
    let base = base_dir(scope, but_path::home_dir().as_deref(), root.as_deref())?;

    let mut sources = Vec::new();
    let mut found: Option<PolicyOptions> = None;
    let mut diverged = false;

    for (path, frameworks) in policy_sources(scope, &base) {
        let block = but_skill::files::read_managed_block_file(&path)
            .ok()
            .flatten();
        let options = block
            .as_deref()
            .map(but_skill::policy::parse_managed_policy_block)
            .map(PolicyOptions::from);

        if let Some(options) = &options {
            match &found {
                None => found = Some(options.clone()),
                // Two files disagreeing means the user edited one by hand or
                // set them up separately; surface it rather than silently
                // showing whichever came first alphabetically.
                Some(first) if first.selected != options.selected => diverged = true,
                Some(_) => {}
            }
        }

        sources.push(PolicySource {
            path: path.to_string_lossy().to_string(),
            frameworks: frameworks.iter().map(|id| id.to_string()).collect(),
            has_managed_block: block.is_some(),
            options,
        });
    }

    Ok(PolicyState {
        available: WorkflowOption::ALL
            .into_iter()
            .map(|option| WorkflowOptionInfo {
                id: option.into(),
                label: option.label().to_string(),
                help: option.help().to_string(),
                default_selected: option.default_selected(),
                repo_local_only: option.repo_local_only(),
                repo_local_help: option
                    .repo_local_only()
                    .then(|| option.repo_local_help().to_string()),
            })
            .collect(),
        current: found,
        defaults: WizardAnswers::default().into(),
        sources,
        diverged,
    })
}

/// Rewrite the managed policy block at `scope` from `options`.
///
/// Only writes instruction files that already hold a managed block, so saving
/// preferences never seeds GitButler steering into an agent's file the user
/// never set up.
#[but_api]
#[instrument(err(Debug))]
pub fn agent_policy_set(
    scope: SkillScope,
    project_id: Option<ProjectHandleOrLegacyProjectId>,
    options: PolicyOptions,
) -> Result<PolicyState> {
    let dto_scope = scope;
    let scope: Scope = scope.into();
    let root = repo_root(project_id.clone())?;
    let base = base_dir(scope, but_path::home_dir().as_deref(), root.as_deref())?;

    let answers: WizardAnswers = options.into();
    // Normalize first so what we write matches what a later read gives back.
    let answers = answers.normalized();
    let block = but_skill::policy::render_managed_policy_block(&answers);

    for (path, _) in policy_sources(scope, &base) {
        if but_skill::files::read_managed_block_file(&path)
            .ok()
            .flatten()
            .is_none()
        {
            continue;
        }
        but_skill::files::upsert_managed_block_file(&path, &block)?;
    }

    agent_policy_get(dto_scope, project_id)
}

/// Whether the `but` CLI symlink is installed, and where it points.
#[but_api]
#[instrument(err(Debug))]
pub fn cli_install_state() -> Result<CliInstallState> {
    Ok(but_skill::cli_link::cli_install_state()?.into())
}

/// Remove the `but` CLI symlink, leaving the binary it points at intact.
#[but_api]
#[instrument(err(Debug))]
pub fn uninstall_cli() -> Result<CliInstallState> {
    Ok(but_skill::cli_link::uninstall_cli()?.into())
}

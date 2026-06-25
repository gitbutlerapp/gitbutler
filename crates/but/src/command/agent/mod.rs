use std::{
    fmt::{self, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use nonempty::NonEmpty;

use crate::{
    args::agent,
    command::skill,
    theme::{self, Paint},
    utils::{InputOutputChannel, OutputChannel, PromptLine, detect_agent},
};

mod files;
mod plan;
mod policy;
#[cfg(test)]
mod tests;

use files::upsert_managed_block_file;
use plan::{AgentTarget, Plan};
use policy::{WizardAnswers, WorkflowOption, render_managed_policy_block};

const MANAGED_BLOCK_START: &str = "<!-- gitbutler-agent-setup:start -->";
const MANAGED_BLOCK_END: &str = "<!-- gitbutler-agent-setup:end -->";

/// Error type for user-initiated cancellation.
#[derive(Debug, Clone, Copy)]
struct UserCancelled;

impl fmt::Display for UserCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Agent setup cancelled")
    }
}

impl std::error::Error for UserCancelled {}

pub fn handle(
    current_dir: &Path,
    out: &mut OutputChannel,
    cmd: Option<agent::Subcommands>,
) -> Result<()> {
    match cmd {
        // Bare `but agent` runs the setup wizard, same as `but agent setup`.
        None => setup(current_dir, out, false),
        Some(agent::Subcommands::Setup { print }) => setup(current_dir, out, print),
    }
}

fn setup(current_dir: &Path, out: &mut OutputChannel, print_only: bool) -> Result<()> {
    if print_only {
        let default_policy = render_managed_policy_block(&WizardAnswers::default());
        print_policy(out, &default_policy)?;
        return Ok(());
    }

    let repo = discover_repo(current_dir);

    if !out.can_prompt() {
        anyhow::bail!(
            "Interactive setup requires a terminal. Use `but agent setup --print` to print the default instructions without modifying files."
        );
    }

    let mut input = out
        .prepare_for_terminal_input()
        .context("Interactive setup requires a terminal.")?;

    let plan = collect_plan(&mut input, repo)?;
    drop(input);

    match plan {
        Some(plan) => apply_plan(out, current_dir, &plan),
        None => print_cancelled(out),
    }
}

/// Run the interactive wizard. Returns the confirmed plan, or `None` if the user
/// cancelled at any point — an `Esc`/`Ctrl-C` on any screen, or the final
/// "Cancel". Cancelling never writes anything.
fn collect_plan(
    input: &mut InputOutputChannel<'_>,
    repo: Option<RepoInfo>,
) -> Result<Option<Plan>> {
    match collect_plan_inner(input, repo) {
        Ok(plan) => Ok(Some(plan)),
        Err(err) if err.downcast_ref::<UserCancelled>().is_some() => Ok(None),
        Err(err) => Err(err),
    }
}

fn collect_plan_inner(input: &mut InputOutputChannel<'_>, repo: Option<RepoInfo>) -> Result<Plan> {
    write_intro(input, repo.as_ref())?;

    // The scope question only appears inside a repository; outside one,
    // everything is personal (global), so there is nothing to choose.
    let mut steps = Steps::new(if repo.is_some() { 4 } else { 3 });

    let agents = prompt_agents(&mut steps, input, repo.as_ref())?;
    let scope = match repo.as_ref() {
        Some(repo) => prompt_scope(&mut steps, input, repo)?,
        None => Scope::Global,
    };
    let workflow = prompt_workflow_options(&mut steps, input)?;
    let answers = prompt_follow_up_answers(input, workflow)?;
    let policy = render_managed_policy_block(&answers);

    let plan = Plan::new(repo.as_ref(), scope, agents, policy)?;

    if prompt_review(&mut steps, input, &plan)? {
        Ok(plan)
    } else {
        Err(UserCancelled.into())
    }
}

fn print_cancelled(out: &mut OutputChannel) -> Result<()> {
    if let Some(writer) = out.for_human() {
        writeln!(writer)?;
        writeln!(
            writer,
            "Cancelled. No skill was installed and no agent files were modified."
        )?;
    }
    Ok(())
}

fn print_policy(out: &mut OutputChannel, policy: &str) -> Result<()> {
    if let Some(json_out) = out.for_json() {
        json_out.write_value(serde_json::json!({ "policy": policy }))?;
        return Ok(());
    }
    let writer = out
        .for_human_or_shell()
        .context("Text output is required for `but agent setup --print`.")?;
    writeln!(writer, "{policy}")?;
    Ok(())
}

/// Width used for the banner rule and right-aligned step counter. Clamped so the
/// header stays tidy on both narrow and very wide terminals.
fn banner_width() -> usize {
    crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(72)
        .clamp(40, 72)
}

/// Tracks the wizard's position so each screen can show "Step N of M".
struct Steps {
    current: usize,
    total: usize,
}

impl Steps {
    fn new(total: usize) -> Self {
        Self { current: 0, total }
    }

    /// Advance to the next step and print its banner: the step's short title on
    /// the left and the `Step N of M` counter on the right. The product name is
    /// only shown once, in the intro, so it does not repeat on every screen.
    fn banner(&mut self, writer: &mut impl fmt::Write, title: &str) -> fmt::Result {
        self.current += 1;
        let t = theme::get();
        let width = banner_width();
        let right = format!("Step {} of {}", self.current, self.total);
        let gap = width
            .saturating_sub(title.chars().count() + right.chars().count())
            .max(1);
        writeln!(writer)?;
        writeln!(
            writer,
            "{}{}{}",
            t.important.paint(title),
            " ".repeat(gap),
            t.hint.paint(right)
        )?;
        writeln!(writer, "{}", t.border.paint("─".repeat(width)))
    }
}

fn write_intro(writer: &mut impl fmt::Write, repo: Option<&RepoInfo>) -> fmt::Result {
    let t = theme::get();
    let width = banner_width();

    writeln!(writer)?;
    writeln!(writer, "{}", t.important.paint("GitButler · agent setup"))?;
    writeln!(writer, "{}", t.border.paint("─".repeat(width)))?;
    writeln!(
        writer,
        "Set up your coding agent to work well with GitButler."
    )?;
    writeln!(writer)?;
    writeln!(writer, "Here's what this does:")?;
    writeln!(
        writer,
        "  {} Install the GitButler skill so your agent can drive {}",
        t.info.paint("•"),
        t.command_suggestion.paint("but")
    )?;
    writeln!(
        writer,
        "  {} Save a few preferences for how it commits, branches, and opens PRs",
        t.info.paint("•")
    )?;
    writeln!(writer)?;
    writeln!(
        writer,
        "{}",
        t.hint.paint(
            "Nothing is written until you review and confirm — you'll see exactly what changes first."
        )
    )?;
    if repo.is_none() {
        writeln!(
            writer,
            "{}",
            t.hint
                .paint("No repository here, so this applies to all your projects.")
        )?;
    }
    writeln!(
        writer,
        "{}",
        t.hint
            .paint("↑/↓ move · space select · enter continue · esc cancel")
    )
}

#[derive(Debug, Clone)]
struct RepoInfo {
    root: PathBuf,
    needs_setup: bool,
}

fn discover_repo(current_dir: &Path) -> Option<RepoInfo> {
    let repo = gix::discover(current_dir).ok()?;
    let root = repo.workdir()?.to_path_buf();
    let needs_setup = repo_needs_setup(&root);
    Some(RepoInfo { root, needs_setup })
}

#[cfg(feature = "legacy")]
fn repo_needs_setup(workdir: &Path) -> bool {
    let Ok(project) = but_ctx::LegacyProject::find_by_worktree_dir(workdir) else {
        return true;
    };
    let Ok(ctx) = but_ctx::Context::new_from_legacy_project(project) else {
        return true;
    };
    let guard = ctx.shared_worktree_access();
    crate::command::legacy::setup::check_project_setup(&ctx, guard.read_permission()).is_err()
}

#[cfg(not(feature = "legacy"))]
fn repo_needs_setup(_workdir: &Path) -> bool {
    false
}

/// Where the user wants this setup to apply. A single choice that drives both
/// where skills install and where workflow preferences are written, so the user
/// is not asked the same repository-vs-global question several times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Repository,
    Global,
    Both,
}

impl Scope {
    fn summary(self) -> &'static str {
        match self {
            Self::Repository => "this project",
            Self::Global => "all your projects",
            Self::Both => "this project and your global setup",
        }
    }
}

#[derive(Debug, Clone)]
struct PickerLabel {
    label: String,
    help: &'static str,
}

impl PickerLabel {
    fn new(label: impl Into<String>, help: &'static str) -> Self {
        Self {
            label: label.into(),
            help,
        }
    }
}

impl fmt::Display for PickerLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn prompt_agents(
    steps: &mut Steps,
    input: &mut InputOutputChannel<'_>,
    repo: Option<&RepoInfo>,
) -> Result<Vec<AgentTarget>> {
    steps.banner(input, "Agents")?;

    // Pre-select every agent the user appears to use: the one currently invoking
    // the CLI (if any), plus any whose config we can find on this machine or in
    // this repository.
    let running = detect_agent::detect().and_then(AgentTarget::from_detected);
    let home = dirs::home_dir();
    let preselect =
        |agent: AgentTarget| Some(agent) == running || agent.in_use(home.as_deref(), repo);

    let flags = AgentTarget::ALL
        .into_iter()
        .map(preselect)
        .collect::<Vec<_>>();
    let options = AgentTarget::ALL
        .into_iter()
        .zip(flags.iter().copied())
        .map(|(agent, detected)| {
            let label = if detected {
                format!("{} (detected)", agent.name())
            } else {
                agent.name().to_string()
            };
            (PickerLabel::new(label, agent.help()), agent)
        })
        .collect::<Vec<_>>();

    let mut defaults = flags
        .iter()
        .enumerate()
        .filter_map(|(idx, &detected)| detected.then_some(idx))
        .collect::<Vec<_>>();

    let options = NonEmpty::from_vec(options).context("agent options cannot be empty")?;
    loop {
        let selected = input
            .prompt_multi_select_with_help(
                "Which agents do you use?",
                &options,
                defaults.clone(),
                |label| Some(label.help),
            )?
            .ok_or(UserCancelled)?;
        if selected.is_empty() {
            // The user cleared every agent on purpose; re-prompt from a clean
            // slate so they don't have to deselect the detected defaults again.
            defaults.clear();
            let t = theme::get();
            writeln!(
                input,
                "{}",
                t.attention.paint("Pick at least one agent to continue.")
            )?;
            continue;
        }
        return Ok(selected.into_iter().copied().collect());
    }
}

fn prompt_scope(
    steps: &mut Steps,
    input: &mut InputOutputChannel<'_>,
    repo: &RepoInfo,
) -> Result<Scope> {
    steps.banner(input, "Where it applies")?;

    let options = scope_options(repo);
    input
        .prompt_select_with_help("Where should this apply?", &options, Some(0), |label| {
            Some(label.help)
        })?
        .copied()
        .ok_or_else(|| UserCancelled.into())
}

fn scope_options(repo: &RepoInfo) -> NonEmpty<(PickerLabel, Scope)> {
    // Lead with the global option: it is the better default, since the skill is
    // useful in every repository and personal preferences travel with the user.
    // The repository option is for deliberate, team-shared setups.
    NonEmpty::from_vec(vec![
        (
            PickerLabel::new(
                "All my projects (global)",
                "Install the skill and save preferences in your personal config, for every repository.",
            ),
            Scope::Global,
        ),
        (
            PickerLabel::new(
                format!("Just this project ({})", repo_display_name(repo)),
                "Install the skill and save preferences in this repository only.",
            ),
            Scope::Repository,
        ),
        (
            PickerLabel::new("Both", "Set up this project and your personal config."),
            Scope::Both,
        ),
    ])
    .expect("scope options are non-empty")
}

fn repo_display_name(repo: &RepoInfo) -> String {
    display_name_from_path(&repo.root)
        .or_else(|| {
            repo.root
                .canonicalize()
                .ok()
                .and_then(|path| display_name_from_path(&path))
        })
        .unwrap_or_else(|| repo.root.display().to_string())
}

fn display_name_from_path(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    (!name.is_empty() && name != "." && name != "..").then(|| name.into_owned())
}

fn prompt_workflow_options(
    steps: &mut Steps,
    input: &mut InputOutputChannel<'_>,
) -> Result<Vec<WorkflowOption>> {
    steps.banner(input, "Preferences")?;

    let t = theme::get();
    writeln!(input, "These shape how your agent works with GitButler.")?;
    writeln!(
        input,
        "{}",
        t.hint
            .paint("Safe defaults are always on. These are extras you can override in any prompt.")
    )?;
    writeln!(
        input,
        "{}",
        t.hint
            .paint("Learn more: https://docs.gitbutler.com/ai-agents/tuning-agent-behavior")
    )?;
    writeln!(input)?;

    let options = WorkflowOption::ALL
        .into_iter()
        .map(|option| (PickerLabel::new(option.label(), option.help()), option))
        .collect::<Vec<_>>();
    let defaults = options
        .iter()
        .enumerate()
        .filter_map(|(idx, (_, option))| option.default_selected().then_some(idx))
        .collect();
    let options = NonEmpty::from_vec(options).context("workflow options cannot be empty")?;
    let selected = input
        .prompt_multi_select_with_help(
            "Pick any that fit how you like to work:",
            &options,
            defaults,
            |label| Some(label.help),
        )?
        .ok_or(UserCancelled)?;
    Ok(selected.into_iter().copied().collect())
}

fn prompt_follow_up_answers(
    input: &mut InputOutputChannel<'_>,
    selected: Vec<WorkflowOption>,
) -> Result<WizardAnswers> {
    let mut answers = WizardAnswers {
        selected,
        ..WizardAnswers::default()
    };

    // The follow-ups below are sub-questions of the Preferences step, so caption
    // them once instead of giving each its own numbered banner.
    if answers.has(WorkflowOption::PublishPhrase)
        || answers.has(WorkflowOption::BranchPattern)
        || answers.has(WorkflowOption::CommitConvention)
    {
        writeln!(input)?;
        writeln!(
            input,
            "{}",
            theme::get().hint.paint("A few details for your picks:")
        )?;
    }

    if answers.has(WorkflowOption::PublishPhrase) {
        answers.publish_phrase = prompt_optional_text(input, "Publish shortcut phrase", "ship it")?;
    }

    if answers.has(WorkflowOption::BranchPattern) {
        answers.branch_pattern = Some(prompt_optional_text(
            input,
            "Branch naming pattern",
            "<name>/<short-description>",
        )?);
    }

    if answers.has(WorkflowOption::CommitConvention) {
        answers.commit_convention = Some(prompt_optional_text(
            input,
            "Commit message convention",
            "type(scope): summary",
        )?);
    }

    Ok(answers)
}

/// Ask for an optional value, defaulting to `default` on an empty line. The
/// prompt spells out that pressing Enter accepts the default.
fn prompt_optional_text(
    input: &mut InputOutputChannel<'_>,
    label: &str,
    default: &str,
) -> Result<String> {
    let prompt = format!("{label} (press Enter for '{default}'):");
    match input.prompt_single_line_input(&prompt)? {
        // `readline` already trims and routes empty input to PromptLine::Empty.
        // Strip backticks, since the value is rendered inside inline code
        // (`` `..` ``) in the generated steering and a stray backtick would break
        // it. If nothing usable is left (e.g. only backticks), fall back to the
        // default rather than emit an empty inline-code value.
        PromptLine::Text(value) => {
            let cleaned = value.replace('`', "");
            let cleaned = cleaned.trim();
            Ok(if cleaned.is_empty() {
                default.to_string()
            } else {
                cleaned.to_string()
            })
        }
        PromptLine::Empty => Ok(default.to_string()),
        PromptLine::Cancelled => Err(UserCancelled.into()),
    }
}

/// Show the concrete plan, then ask for a single confirmation. Returns `true` to
/// apply and `false` to cancel; an `Esc`/`Ctrl-C` also cancels (propagated as
/// `UserCancelled`).
fn prompt_review(
    steps: &mut Steps,
    input: &mut InputOutputChannel<'_>,
    plan: &Plan,
) -> Result<bool> {
    steps.banner(input, "Review")?;
    write_review(input, plan)?;

    let options = NonEmpty::from_vec(vec![
        (
            PickerLabel::new("Apply", "Install the skill and write the files now."),
            true,
        ),
        (
            PickerLabel::new("Cancel", "Change nothing and exit."),
            false,
        ),
    ])
    .expect("review options are non-empty");
    input
        .prompt_select_with_help("Apply these changes?", &options, Some(0), |label| {
            Some(label.help)
        })?
        .copied()
        .ok_or_else(|| UserCancelled.into())
}

fn write_review(writer: &mut impl fmt::Write, plan: &Plan) -> fmt::Result {
    let t = theme::get();
    let check = &t.sym().success;

    writeln!(
        writer,
        "Here's what GitButler will set up for {}.",
        t.important.paint(plan.scope.summary())
    )?;

    if !plan.skill_installs.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "{}", t.important.paint("Install the skill"))?;
        for install in &plan.skill_installs {
            writeln!(
                writer,
                "  {check} {} {} {}",
                install.agent.name(),
                t.sym().arrow,
                t.config_value.paint(display_path(&install.path))
            )?;
        }
    }

    if !plan.instruction_writes.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "{}", t.important.paint("Save preferences to"))?;
        for write in &plan.instruction_writes {
            let agents = write
                .agents
                .iter()
                .map(|agent| agent.name())
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                writer,
                "  {check} {} {}",
                t.config_value.paint(display_path(&write.path)),
                t.hint.paint(format!("({agents})"))
            )?;
        }
    }

    if !plan.print_only_notes.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "{}", t.important.paint("Heads up"))?;
        for note in &plan.print_only_notes {
            writeln!(writer, "  {} {note}", t.sym().warning)?;
        }
    }

    if plan.setup_needed {
        writeln!(writer)?;
        writeln!(writer, "{}", t.important.paint("Prepare this repository"))?;
        writeln!(
            writer,
            "  {check} Run {}",
            t.command_suggestion.paint("but setup")
        )?;
    }

    writeln!(writer)?;
    writeln!(writer, "{}", t.important.paint("The exact text"))?;
    write_policy_preview(writer, &plan.policy)?;
    writeln!(writer)
}

/// Shorten a path for display: collapse the home directory to `~` and drop a
/// leading `.` component so review paths read cleanly (`AGENTS.md`,
/// `~/.codex/...`).
fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
        };
    }
    // Strip a leading `.` component (`./AGENTS.md` -> `AGENTS.md`); component-aware
    // so it also handles Windows' `.\AGENTS.md`.
    path.strip_prefix(".")
        .map(|rest| rest.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

/// Render the generated rules indented, with the managed-block markers dimmed so
/// the human-facing content reads clearly while still being honest about what
/// lands in the file.
fn write_policy_preview(writer: &mut impl fmt::Write, policy: &str) -> fmt::Result {
    let t = theme::get();
    for line in policy.lines() {
        if line == MANAGED_BLOCK_START || line == MANAGED_BLOCK_END {
            writeln!(writer, "  {}", t.hint.paint(line))?;
        } else if line.is_empty() {
            writeln!(writer)?;
        } else {
            writeln!(writer, "  {line}")?;
        }
    }
    Ok(())
}

fn apply_plan(out: &mut OutputChannel, current_dir: &Path, plan: &Plan) -> Result<()> {
    // Run `but setup` first: it is the step most likely to fail (it needs a
    // discoverable repo + project registration) and aborts before any file is
    // written, keeping the intro's "nothing is written until you confirm"
    // promise on the realistic abort path. The remaining skill→instruction
    // writes are idempotent upserts, so a partial run is re-runnable.
    if plan.setup_needed {
        run_but_setup(current_dir, out)?;
    }

    for install in &plan.skill_installs {
        skill::write_skill_files(&install.path)
            .with_context(|| format!("Failed to install skill at {}", install.path.display()))?;
    }

    for write in &plan.instruction_writes {
        upsert_managed_block_file(&write.path, &plan.policy)
            .with_context(|| format!("Failed to update {}", write.path.display()))?;
    }

    if let Some(writer) = out.for_human() {
        let t = theme::get();
        writeln!(writer)?;
        writeln!(
            writer,
            "{} GitButler agent setup complete.",
            t.sym().success
        )?;
    }
    Ok(())
}

#[cfg(feature = "legacy")]
fn run_but_setup(current_dir: &Path, out: &mut OutputChannel) -> Result<()> {
    let repo = gix::discover(current_dir).context("No git repository found for `but setup`.")?;
    let mut ctx = but_ctx::Context::from_repo(repo)?;
    let mut guard = ctx.exclusive_worktree_access();
    crate::command::legacy::setup::repo(&mut ctx, current_dir, out, guard.write_permission())
        .context("Failed to set up GitButler project.")
}

#[cfg(not(feature = "legacy"))]
fn run_but_setup(_current_dir: &Path, _out: &mut OutputChannel) -> Result<()> {
    Ok(())
}

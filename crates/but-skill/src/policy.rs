//! The workflow preferences a user picks during agent setup, and how they
//! render into (and parse back out of) the managed steering block.

use std::fmt::Write as _;

use crate::files::{MANAGED_BLOCK_END, MANAGED_BLOCK_START};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowOption {
    FoldFixes,
    SuggestSplits,
    StackedBranches,
    AutoUpdate,
    DraftPrs,
    PushToTarget,
    PublishPhrase,
    BranchPattern,
    CommitConvention,
    CommitAfterTurn,
}

impl WorkflowOption {
    pub const ALL: [Self; 10] = [
        Self::FoldFixes,
        Self::SuggestSplits,
        Self::StackedBranches,
        Self::AutoUpdate,
        Self::DraftPrs,
        Self::PushToTarget,
        Self::PublishPhrase,
        Self::BranchPattern,
        Self::CommitConvention,
        Self::CommitAfterTurn,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FoldFixes => "Prefer folding small follow-up fixes into the matching commit",
            Self::SuggestSplits => "Suggest splitting large or mixed commits into smaller commits",
            Self::StackedBranches => "Favor stacked branches and PRs for dependent work",
            Self::AutoUpdate => "Automatically update from the target branch (e.g. origin/main)",
            Self::DraftPrs => "Open pull requests as drafts unless I say they are ready",
            Self::PushToTarget => {
                "\"Push to main\" / skip-the-PR workflow — land onto the target branch"
            }
            Self::PublishPhrase => "Use a shortcut phrase to publish everything",
            Self::BranchPattern => "Set a preferred branch naming pattern",
            Self::CommitConvention => "Set a preferred commit message convention",
            Self::CommitAfterTurn => "Commit after each agent coding turn",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Self::FoldFixes => {
                "Small cleanup fixes go into the commit they belong to instead of becoming extra fixup commits."
            }
            Self::SuggestSplits => {
                "For large or mixed work, the agent suggests a cleaner split before committing."
            }
            Self::StackedBranches => {
                "For dependent work, prefer smaller stacked branches and PRs over one large branch."
            }
            Self::AutoUpdate => {
                "Bring in latest target-branch changes when they apply cleanly; ask before conflicts or surprising context changes."
            }
            Self::DraftPrs => {
                "New pull requests start as drafts unless you explicitly ask for a ready PR."
            }
            Self::PushToTarget => {
                "When you tell your agent the work is ready to ship, it lands the branch directly onto the target (e.g. main) instead of opening a pull request."
            }
            Self::PublishPhrase => {
                "Default phrase: \"ship it\". You'll be asked next if you select this."
            }
            Self::BranchPattern => {
                "You'll choose the pattern next, for example `<name>/<short-description>` or `feature/<ticket>-<slug>`."
            }
            Self::CommitConvention => {
                "You'll choose the convention next, for example `type(scope): summary` or `summary only, no prefix`."
            }
            Self::CommitAfterTurn => {
                "Your agent makes a local checkpoint commit after each coding turn, then tidies the history with GitButler when you tell it to."
            }
        }
    }

    pub fn default_selected(self) -> bool {
        matches!(self, Self::FoldFixes | Self::SuggestSplits)
    }

    /// Whether this preference only makes sense for a single repository. The
    /// generated rules are rendered once and written to every place the setup
    /// targets, so a repo-local rule (like landing onto the target) must not be
    /// offered for a global or combined setup, where it would also land in the
    /// user's global config.
    pub fn repo_local_only(self) -> bool {
        matches!(self, Self::PushToTarget)
    }

    /// The `###` heading this option renders as inside the managed block.
    ///
    /// Single-sourced because [`parse_managed_policy_block`] maps these
    /// headings back to options: reword one here and the parser follows,
    /// instead of silently failing to recognise existing installs.
    pub fn section_title(self) -> &'static str {
        match self {
            Self::FoldFixes => "Amend local fixes into the right commits",
            Self::SuggestSplits => "Split unrelated changes into separate commits",
            Self::StackedBranches => "Create stacked pull requests",
            Self::AutoUpdate => "Update from the target branch automatically",
            Self::DraftPrs => "Open draft pull requests by default",
            Self::PushToTarget => "Skip pull requests and land onto the target",
            Self::PublishPhrase => "Publish on a shortcut phrase",
            Self::BranchPattern => "Branch naming",
            Self::CommitConvention => "Commit message convention",
            Self::CommitAfterTurn => "Commit checkpoints after each turn",
        }
    }

    /// Help shown for a repo-local-only option when the current setup is not
    /// scoped to a single repository: spells out how to enable it and what it
    /// does.
    pub fn repo_local_help(self) -> &'static str {
        "Re-run setup for a single repo (pick \"Just this project\") to enable landing work directly onto the target (e.g. main) instead of opening pull requests."
    }
}

#[derive(Debug, Clone)]
pub struct WizardAnswers {
    pub selected: Vec<WorkflowOption>,
    pub publish_phrase: String,
    pub branch_pattern: Option<String>,
    pub commit_convention: Option<String>,
}

impl Default for WizardAnswers {
    fn default() -> Self {
        Self {
            selected: WorkflowOption::ALL
                .into_iter()
                .filter(|option| option.default_selected())
                .collect(),
            publish_phrase: default_publish_phrase().to_string(),
            branch_pattern: None,
            commit_convention: None,
        }
    }
}

impl WizardAnswers {
    pub fn has(&self, option: WorkflowOption) -> bool {
        self.selected.contains(&option)
    }

    /// The answers as they survive a render/parse round trip.
    ///
    /// Rendering is not injective: the branch-pattern and commit-convention
    /// sections render from their value being set rather than from the option
    /// being selected, and a publish phrase is only written when its option is
    /// on. Normalizing resolves those disagreements the same way rendering
    /// does, so a settings UI that saves and reloads sees a stable value
    /// instead of a checkbox that silently un-checks itself.
    pub fn normalized(&self) -> Self {
        let mut selected: Vec<_> = WorkflowOption::ALL
            .into_iter()
            .filter(|option| match option {
                // These two render from the value, not the checkbox.
                WorkflowOption::BranchPattern => self.branch_pattern.is_some(),
                WorkflowOption::CommitConvention => self.commit_convention.is_some(),
                other => self.has(*other),
            })
            .collect();
        selected.dedup();

        let publish_phrase = if selected.contains(&WorkflowOption::PublishPhrase) {
            self.publish_phrase.clone()
        } else {
            // Not rendered, so it cannot be read back; keep it predictable.
            default_publish_phrase().to_string()
        };

        Self {
            selected,
            publish_phrase,
            branch_pattern: self.branch_pattern.clone(),
            commit_convention: self.commit_convention.clone(),
        }
    }
}

/// The selected-change commit fast-path rule. Named so `super::cleanup` can
/// rewrite the retired wording in already-installed policy blocks to exactly
/// this text. Rewording it strands old installs on the previous wording unless
/// the cleanup learns that wording as another retired variant.
pub const FAST_PATH_BULLET: &str = "For commit just/only/specific changes on a new branch (selected-change requests), use the two-command fast path from the GitButler skill: `but diff`, then `but commit -b <branch> -m \"message\" <id> <id>`.";

/// Render the GitButler steering as a managed block. Mirrors the published
/// guidance: an always-on `## Version control` baseline (see the docs "Getting
/// started" page) followed by one `###` section per selected preference (see
/// the docs "Tuning agent behavior" page). The bullets are kept close to the
/// docs text so the result matches hand-copying the relevant snippets, and they
/// are phrased as direct instructions an agent can act on.
pub fn render_managed_policy_block(answers: &WizardAnswers) -> String {
    let mut body = String::new();
    body.push_str(MANAGED_BLOCK_START);
    body.push('\n');
    body.push_str("## Version control\n\n");
    write_bullets(
        &mut body,
        &[
            "Use GitButler (`but`) for version-control inspection and write operations, including status, diffs, branching, committing, pushing, and history edits.",
            "Assume multiple agents may be working in this repository. Do not move, amend, squash, discard, commit, push, or otherwise modify another agent's work unless the user asks.",
            FAST_PATH_BULLET,
            "For that fast path, after the commit succeeds, stop and summarize; do not run separate branch, staging, status, or diff commands unless the commit output is missing information you need.",
            "Use the installed GitButler skill for command recipes and syntax before guessing flags, using `--help`, or translating Git habits directly.",
            "Mutation commands report their result without appending workspace status. Add `--status-after` only when the next step needs resulting workspace IDs or details; otherwise do not rerun status or diff to verify success.",
            "Use a dedicated GitButler branch for each agent session, unless the user asks for a different branch structure. Commit only changes that belong to that session.",
            "Do not push or open pull requests unless the user asks.",
            "Keep commit messages and pull request descriptions succinct: explain what changed, why it changed, and any important decision.",
        ],
    );

    if answers.has(WorkflowOption::FoldFixes) {
        write_section(
            &mut body,
            WorkflowOption::FoldFixes.section_title(),
            &[
                "For small cleanup or follow-up fixes, amend an unpublished local commit when the change clearly belongs with that commit's intent.",
                "Do not create tiny fixup commits unless the user asks.",
                "Use GitButler to move the relevant changes into the commit where they belong.",
                "Ask before rewriting pushed, reviewed, shared, or ambiguous history.",
            ],
        );
    }
    if answers.has(WorkflowOption::SuggestSplits) {
        write_section(
            &mut body,
            WorkflowOption::SuggestSplits.section_title(),
            &[
                "If one file contains unrelated changes, split them by hunk instead of committing the whole file.",
                "Keep tests with the behavior they verify.",
                "Split generated output, docs-only edits, or mechanical cleanup into separate commits when each commit remains coherent on its own.",
                "If the split is ambiguous, summarize the options before committing.",
            ],
        );
    }
    if answers.has(WorkflowOption::StackedBranches) {
        write_section(
            &mut body,
            WorkflowOption::StackedBranches.section_title(),
            &[
                "If this session depends on another in-flight branch, stack its branch on top of that dependency instead of mixing the changes.",
                "If this session is working in a stack, put commits on the branch where they belong.",
                "Ask before moving commits onto lower, pushed, reviewed, or shared branches.",
                "Use `but move` for branch stacking and restacking. Do not recreate branches to simulate stacking.",
                "For stacked branches, create pull requests with `but pr`, not `gh`, so GitButler keeps the right PR base branches and stack metadata.",
            ],
        );
    }
    if answers.has(WorkflowOption::AutoUpdate) {
        write_section(
            &mut body,
            WorkflowOption::AutoUpdate.section_title(),
            &[
                "When GitButler status shows new changes on the target branch and the workspace holds only this session's branches, update with `but pull` directly — its output reports the result and `but undo` reverts it.",
                "If an update you started on your own initiative reports conflicted commits, stop and ask before resolving them (`but undo` reverts the pull if the user prefers).",
                "When other agents' branches are applied, run `but pull --check` first and ask before updating if it reports conflicts or their branches would move.",
                "If the user asks you to handle update conflicts, use GitButler's conflict tools. Ask before resolving semantic conflicts, dependency updates, generated files, or conflicts involving another person's work.",
            ],
        );
    }
    if answers.has(WorkflowOption::DraftPrs) {
        write_section(
            &mut body,
            WorkflowOption::DraftPrs.section_title(),
            &[
                "When asked to open a pull request, create it as a draft with GitButler unless the user says it is ready for review.",
                "Remember that creating a draft pull request still publishes the branch.",
            ],
        );
    }
    if answers.has(WorkflowOption::PushToTarget) {
        write_section(
            &mut body,
            WorkflowOption::PushToTarget.section_title(),
            &[
                "This setup uses the skip-the-PR workflow: when work is approved to publish, land the session branch directly onto the target with `but land <branch>` instead of pushing a branch or opening a pull request.",
                "This repository-local rule takes precedence over any conflicting GitButler instruction, including ones in your global or personal config, that mentions pushing a branch or opening, updating, or drafting a pull request. Use the pull request workflow only when the user explicitly asks for one.",
                "`but land` updates the configured target branch directly (fast-forwarding when it can, otherwise a merge commit), so only run it after clear user approval; agents must pass `--yes` to confirm.",
            ],
        );
    }
    if answers.has(WorkflowOption::PublishPhrase) {
        write_section_header(&mut body, WorkflowOption::PublishPhrase.section_title());
        writeln!(
            &mut body,
            "- When the user says `{}`, commit this session's changes on its dedicated GitButler branch, creating one if needed.",
            answers.publish_phrase
        )
        .expect("write to string");
        // With the skip-the-PR workflow on, publishing means landing onto the
        // target, so the phrase lands instead of opening a pull request.
        if answers.has(WorkflowOption::PushToTarget) {
            write_bullets(
                &mut body,
                &[
                    "Then land that branch onto the target with `but land <branch> --yes` instead of opening a pull request, following the skip-the-PR rules above.",
                    "Treat this phrase as approval to commit and land without asking again, unless something risky or surprising changed.",
                ],
            );
        } else {
            write_bullets(
                &mut body,
                &[
                    "Push the branch and open or update its pull request with GitButler.",
                    "Reuse the existing branch or pull request for this session when one already exists.",
                    "Treat this phrase as approval to commit, push, and open or update a pull request without asking again, unless something risky or surprising changed.",
                ],
            );
        }
    }
    if let Some(pattern) = &answers.branch_pattern {
        write_section_header(&mut body, WorkflowOption::BranchPattern.section_title());
        writeln!(
            &mut body,
            "- When creating a GitButler branch for an agent session, use `{pattern}`."
        )
        .expect("write to string");
    }
    if let Some(convention) = &answers.commit_convention {
        write_section_header(&mut body, WorkflowOption::CommitConvention.section_title());
        writeln!(
            &mut body,
            "- Follow the `{convention}` commit-message convention when writing commit messages."
        )
        .expect("write to string");
    }
    if answers.has(WorkflowOption::CommitAfterTurn) {
        write_section(
            &mut body,
            WorkflowOption::CommitAfterTurn.section_title(),
            &[
                "Commit after a working checkpoint, when the requested change is complete and relevant checks have passed or been reported.",
                "Treat checkpoint commits as local savepoints, not final review history.",
                "When the user asks you to tidy the history, use GitButler to squash commits, reword commits, and move changes between commits where appropriate.",
                "Only tidy unpublished local history unless the user explicitly authorizes changing pushed or shared history.",
            ],
        );
    }

    body.push_str(MANAGED_BLOCK_END);
    body.push('\n');
    body
}

/// Write a blank separator, a `### {title}` heading, and a blank line.
fn write_section_header(body: &mut String, title: &str) {
    body.push('\n');
    body.push_str("### ");
    body.push_str(title);
    body.push_str("\n\n");
}

/// Write a `### {title}` section followed by its bullets.
fn write_section(body: &mut String, title: &str, bullets: &[&str]) {
    write_section_header(body, title);
    write_bullets(body, bullets);
}

fn write_bullets(body: &mut String, bullets: &[&str]) {
    for bullet in bullets {
        body.push_str("- ");
        body.push_str(bullet);
        body.push('\n');
    }
}

/// Read the answers back out of a rendered managed block.
///
/// Works by matching the `###` headings emitted by
/// [`render_managed_policy_block`], so it recognises blocks already sitting on
/// users' disks — the wizard never persisted its answers anywhere else.
/// Unknown headings are ignored, so a block written by a newer version parses
/// as far as this version understands it rather than failing outright.
///
/// Note this is inherently lossy in one direction: an option whose section
/// renders nothing (a branch pattern with no pattern set) leaves no trace to
/// read back. [`WizardAnswers::normalized`] models exactly that loss, which is
/// what makes `parse(render(a)) == a.normalized()` hold.
pub fn parse_managed_policy_block(block: &str) -> WizardAnswers {
    let mut selected = Vec::new();
    for option in WorkflowOption::ALL {
        if has_section(block, option.section_title()) {
            selected.push(option);
        }
    }

    // The free-text values live in the single bullet under their heading, each
    // wrapped in an inline-code span. `prompt_optional_text` strips backticks
    // from user input, so the closing backtick is unambiguous.
    let publish_phrase = inline_code_after(block, "- When the user says `")
        .unwrap_or_else(|| default_publish_phrase().to_string());
    let branch_pattern = inline_code_after(
        block,
        "- When creating a GitButler branch for an agent session, use `",
    );
    let commit_convention = inline_code_after(block, "- Follow the `");

    WizardAnswers {
        selected,
        publish_phrase,
        branch_pattern,
        commit_convention,
    }
    .normalized()
}

/// Whether `block` contains a line-anchored `### {title}` heading.
fn has_section(block: &str, title: &str) -> bool {
    block
        .lines()
        .any(|line| line.strip_prefix("### ").is_some_and(|rest| rest == title))
}

/// The contents of the inline-code span that immediately follows `prefix`.
fn inline_code_after(block: &str, prefix: &str) -> Option<String> {
    let start = block.find(prefix)? + prefix.len();
    let rest = &block[start..];
    let end = rest.find('`')?;
    // A heading with an empty value would round-trip to `None`, so treat blank
    // as absent rather than inventing an empty pattern.
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// The phrase used when the shortcut-publish option is on but no phrase was
/// chosen.
pub fn default_publish_phrase() -> &'static str {
    "ship it"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers(selected: &[WorkflowOption]) -> WizardAnswers {
        WizardAnswers {
            selected: selected.to_vec(),
            publish_phrase: "ship it".to_string(),
            branch_pattern: None,
            commit_convention: None,
        }
    }

    /// The property that makes heading-matching safe: every combination of
    /// options survives a render/parse round trip. This fails the moment a
    /// `###` title is reworded without updating `section_title`.
    #[test]
    fn every_option_combination_round_trips() {
        let all = WorkflowOption::ALL;
        for bits in 0u32..(1 << all.len()) {
            let selected: Vec<_> = all
                .into_iter()
                .enumerate()
                .filter(|(i, _)| bits & (1 << i) != 0)
                .map(|(_, option)| option)
                .collect();
            let original = answers(&selected);
            let parsed = parse_managed_policy_block(&render_managed_policy_block(&original));
            assert_eq!(
                parsed.selected,
                original.normalized().selected,
                "options {selected:?} should survive a round trip"
            );
        }
    }

    #[test]
    fn free_text_values_round_trip() {
        let original = WizardAnswers {
            selected: vec![
                WorkflowOption::PublishPhrase,
                WorkflowOption::BranchPattern,
                WorkflowOption::CommitConvention,
            ],
            publish_phrase: "make it so".to_string(),
            branch_pattern: "<name>/<short-description>".to_string().into(),
            commit_convention: "type(scope): summary".to_string().into(),
        };

        let parsed = parse_managed_policy_block(&render_managed_policy_block(&original));
        assert_eq!(parsed.publish_phrase, "make it so");
        assert_eq!(
            parsed.branch_pattern.as_deref(),
            Some("<name>/<short-description>")
        );
        assert_eq!(
            parsed.commit_convention.as_deref(),
            Some("type(scope): summary")
        );
    }

    /// A pattern option ticked with no pattern renders nothing, so it must not
    /// come back selected — otherwise the settings UI would show a checkbox
    /// that never persists.
    #[test]
    fn a_pattern_option_without_a_value_is_dropped() {
        let original = answers(&[WorkflowOption::BranchPattern]);
        assert!(!original.normalized().has(WorkflowOption::BranchPattern));

        let parsed = parse_managed_policy_block(&render_managed_policy_block(&original));
        assert!(!parsed.has(WorkflowOption::BranchPattern));
    }

    /// Conversely, a value with no ticked option still renders, so parsing
    /// must report the option as on.
    #[test]
    fn a_value_without_its_option_still_counts_as_selected() {
        let original = WizardAnswers {
            branch_pattern: Some("feature/<ticket>".into()),
            ..answers(&[])
        };

        let parsed = parse_managed_policy_block(&render_managed_policy_block(&original));
        assert!(parsed.has(WorkflowOption::BranchPattern));
        assert_eq!(parsed.branch_pattern.as_deref(), Some("feature/<ticket>"));
    }

    #[test]
    fn normalizing_is_idempotent() {
        let original = WizardAnswers {
            selected: vec![WorkflowOption::PublishPhrase, WorkflowOption::FoldFixes],
            publish_phrase: "ship it".to_string(),
            branch_pattern: Some("x".into()),
            commit_convention: None,
        };
        let once = original.normalized();
        assert_eq!(once.selected, once.normalized().selected);
    }

    /// A block a user hand-edited, or one written by a newer version, must not
    /// break parsing of the parts this version does understand.
    #[test]
    fn unknown_headings_and_prose_are_ignored() {
        let mut block = render_managed_policy_block(&answers(&[WorkflowOption::FoldFixes]));
        block.push_str("\n### Something a future version added\n\n- A bullet.\n");

        let parsed = parse_managed_policy_block(&block);
        assert_eq!(parsed.selected, vec![WorkflowOption::FoldFixes]);
    }

    /// The parser must not treat the option list itself as a rendered section.
    #[test]
    fn a_block_with_no_optional_sections_parses_as_defaults_off() {
        let parsed = parse_managed_policy_block(&render_managed_policy_block(&answers(&[])));
        assert!(parsed.selected.is_empty(), "got {:?}", parsed.selected);
        assert_eq!(parsed.publish_phrase, default_publish_phrase());
    }
}

use anyhow::bail;
use bstr::ByteSlice;
use but_api::branch::{
    self, IntegrateBranchResult,
    json::{
        self, BranchIntegrationStrategy as JsonBranchIntegrationStrategy,
        IntegrationDivergenceCommit, IntegrationDivergenceDisplay,
    },
};
use but_api::json as api_json;
use but_core::DryRun;
use gix::refs::{Category, FullName, FullNameRef};
use std::collections::HashSet;

use crate::{
    args::branch::IntegrationStrategy,
    theme::{self, Paint},
    tui::get_text,
    tui::text::strip_ansi_codes,
    utils::OutputChannel,
};

pub fn update(
    ctx: &mut but_ctx::Context,
    branch: &str,
    strategy: IntegrationStrategy,
    dry_run: bool,
    verbose: bool,
    interactive: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let branch_ref = resolve_local_branch(ctx, branch)?;
    let initial =
        branch::get_initial_branch_integration(ctx, branch_ref.as_ref(), Some(strategy.into()))?;
    let integration = if interactive {
        integration_from_editor(&initial)?
    } else {
        workspace_integration_to_json(initial.integration)
    };
    let result = branch::apply_branch_integration(
        ctx,
        branch_ref.as_ref(),
        integration,
        if dry_run { DryRun::Yes } else { DryRun::No },
    )?;

    output_apply_result(
        branch_ref.as_ref(),
        &initial.divergence,
        dry_run,
        verbose,
        result,
        out,
    )
}

fn integration_from_editor(
    initial: &but_workspace::branch::InitialBranchIntegration,
) -> anyhow::Result<json::InteractiveIntegration> {
    let script = build_integration_editor_script(initial);
    let edited = get_text::from_editor("branch-integration", &script, None, ".txt")?;
    let steps = but_workspace::branch::parse_integration_steps_script(
        edited.as_slice(),
        &initial.divergence,
    )?;

    Ok(workspace_integration_to_json(
        but_workspace::branch::integrate_branch_upstream::InteractiveIntegration {
            steps,
            merge_base: initial.integration.merge_base,
            first_local_not_integrated: initial.integration.first_local_not_integrated,
        },
    ))
}

fn build_integration_editor_script(
    initial: &but_workspace::branch::InitialBranchIntegration,
) -> String {
    let divergence = strip_ansi_codes(&format_divergence(&initial.divergence)).into_owned();
    let mut lines = vec!["# Edit the integration steps below.".to_owned()];
    lines.extend(
        but_workspace::branch::render_integration_steps_script(&initial.integration.steps)
            .lines()
            .map(ToOwned::to_owned),
    );
    lines.extend(vec![
        "#".to_owned(),
        "# Blank lines and comment lines are ignored.".to_owned(),
        "# Available commands:".to_owned(),
        "#   pick <commit>".to_owned(),
        "#   merge <commit>".to_owned(),
        "#   squash <commit> <commit>... | message=\"...\"".to_owned(),
        "# Only non-integrated local commits and upstream commits may be referenced.".to_owned(),
        "#".to_owned(),
    ]);
    lines.push("#".to_owned());
    lines.extend(divergence.lines().map(|line| format!("# {line}")));
    lines.push(String::new());
    lines.join("\n")
}

fn workspace_integration_to_json(
    integration: but_workspace::branch::integrate_branch_upstream::InteractiveIntegration,
) -> json::InteractiveIntegration {
    json::InteractiveIntegration {
        merge_base: integration.merge_base.into(),
        first_local_not_integrated: integration.first_local_not_integrated.map(Into::into),
        steps: integration
            .steps
            .into_iter()
            .map(|step| match step {
                but_workspace::branch::InteractiveIntegrationStep::Pick { commit_id } => {
                    json::InteractiveIntegrationStep::Pick {
                        commit_id: commit_id.into(),
                    }
                }
                but_workspace::branch::InteractiveIntegrationStep::Squash { commits, message } => {
                    json::InteractiveIntegrationStep::Squash {
                        commits: commits.into_iter().map(Into::into).collect(),
                        message,
                    }
                }
                but_workspace::branch::InteractiveIntegrationStep::Merge { commit_id } => {
                    json::InteractiveIntegrationStep::Merge {
                        commit_id: commit_id.into(),
                    }
                }
            })
            .collect(),
    }
}

fn resolve_local_branch(ctx: &but_ctx::Context, branch: &str) -> anyhow::Result<FullName> {
    let candidate = normalize_local_branch_ref(branch)?;
    let repo = ctx.repo.get()?;
    let Some(reference) = repo.try_find_reference(candidate.as_ref())? else {
        bail!("Local branch '{}' not found", candidate.shorten());
    };
    let reference = reference.detach();
    match reference.name.category() {
        Some(Category::LocalBranch) => Ok(reference.name),
        Some(Category::RemoteBranch) => {
            bail!(
                "Expected a local branch, but '{}' is a remote-tracking branch",
                reference.name.shorten()
            )
        }
        _ => bail!(
            "Expected a local branch, but '{}' is not under refs/heads/",
            reference.name.shorten()
        ),
    }
}

fn normalize_local_branch_ref(branch: &str) -> anyhow::Result<FullName> {
    let full = if branch.starts_with("refs/heads/") {
        branch.to_owned()
    } else if branch.starts_with("refs/") {
        bail!("Only local branches under refs/heads/ are supported");
    } else {
        format!("refs/heads/{branch}")
    };
    FullName::try_from(full).map_err(|err| anyhow::anyhow!("Invalid branch name: {err}"))
}

fn output_apply_result(
    branch_ref: &FullNameRef,
    divergence: &but_workspace::branch::IntegrationDivergenceDisplay,
    dry_run: bool,
    verbose: bool,
    result: IntegrateBranchResult,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    if let Some(out) = out.for_human() {
        if dry_run {
            write!(
                out,
                "{}",
                format_dry_run(divergence, branch_ref, &result, verbose)
            )?;
        } else {
            let t = theme::get();
            writeln!(
                out,
                "Updated branch {}.",
                t.local_branch.paint(branch_ref.shorten().to_string())
            )?;
        }
    } else if let Some(out) = out.for_shell() {
        if dry_run {
            writeln!(out, "preview {branch_ref}")?;
        } else {
            writeln!(out, "integrated {branch_ref}")?;
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(json::IntegrateBranchResult::try_from(result)?)?;
    }
    Ok(())
}

fn format_divergence(divergence: &but_workspace::branch::IntegrationDivergenceDisplay) -> String {
    let t = theme::get();
    let branch_ref_name = api_json::FullRefName::from(divergence.branch_ref_name.clone());
    let upstream_ref_name = api_json::FullRefName::from(divergence.upstream_ref_name.clone());
    let short_branch_name = shorten_full_ref_name(&branch_ref_name);
    let short_upstream_name = shorten_full_ref_name(&upstream_ref_name);
    let divergence: IntegrationDivergenceDisplay = divergence.clone().into();
    let (hidden_integrated_count, local_non_integrated_count) =
        contiguous_integrated_divergence_count(&divergence.local_only);
    let mut lines = Vec::new();
    lines.push(format!(
        "Current state: {} <- {}",
        t.local_branch.paint(short_branch_name),
        t.remote_branch.paint(short_upstream_name)
    ));
    lines.push(String::new());
    lines.extend(
        divergence
            .local_only
            .iter()
            .take(local_non_integrated_count)
            .map(|commit| {
                let style = match commit.target_relation {
                    json::IntegrationDivergenceTargetRelation::HistoricallyIntegrated {
                        ..
                    } => CommitRenderStyle::Integrated,
                    json::IntegrationDivergenceTargetRelation::NotIntegrated => {
                        CommitRenderStyle::LocalOnly
                    }
                };
                format_divergence_commit("", commit, short_branch_name, short_upstream_name, style)
            }),
    );
    if hidden_integrated_count > 0 {
        lines.push(format_collapsed_integrated_summary(hidden_integrated_count));
    }
    let upstream_connector = if divergence.local_only.is_empty() {
        ""
    } else {
        "┊"
    };
    lines.extend(divergence.upstream_only.iter().map(|commit| {
        let style = match commit.target_relation {
            json::IntegrationDivergenceTargetRelation::HistoricallyIntegrated { .. } => {
                CommitRenderStyle::Integrated
            }
            json::IntegrationDivergenceTargetRelation::NotIntegrated => CommitRenderStyle::Upstream,
        };
        format_divergence_commit(
            upstream_connector,
            commit,
            short_branch_name,
            short_upstream_name,
            style,
        )
    }));
    if !divergence.local_only.is_empty() && !divergence.upstream_only.is_empty() {
        lines.push("├╯".into());
    }
    lines.push(format_divergence_base("", &divergence.merge_base));
    lines.push(String::new());
    lines.join("\n")
}

fn format_dry_run(
    divergence: &but_workspace::branch::IntegrationDivergenceDisplay,
    branch_ref: &FullNameRef,
    result: &IntegrateBranchResult,
    verbose: bool,
) -> String {
    if verbose {
        let mut out = format_divergence(divergence);
        out.push_str("\n----------------------------\n\n");
        out.push_str(&format_preview(branch_ref, result));
        out
    } else {
        format_preview(branch_ref, result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommitRenderStyle {
    LocalOnly,
    Upstream,
    Integrated,
}

fn format_divergence_commit(
    connector: &str,
    commit: &IntegrationDivergenceCommit,
    short_branch_name: &str,
    short_upstream_name: &str,
    style: CommitRenderStyle,
) -> String {
    let refs = if commit.refs.is_empty() {
        None
    } else {
        let t = theme::get();
        let refs = commit
            .refs
            .iter()
            .map(|ref_name| match ref_name.as_str() {
                name if name == short_branch_name => t.local_branch.paint(name).to_string(),
                name if name == short_upstream_name => t.remote_branch.paint(name).to_string(),
                name => name.to_owned(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        Some(refs)
    };
    format_commit_row(
        connector,
        style,
        change_id_prefix(commit.change_id.as_deref().map(str::as_bytes)),
        short_hex_json(&commit.id),
        refs.as_deref(),
        Some(commit.subject.as_str()),
        false,
    )
}

fn format_divergence_base(connector: &str, commit: &IntegrationDivergenceCommit) -> String {
    format_base_row(
        connector,
        short_hex_json(&commit.id),
        Some(commit.subject.as_str()),
    )
}

fn format_preview(branch_ref: &FullNameRef, result: &IntegrateBranchResult) -> String {
    let t = theme::get();
    let target_branch = branch_ref.as_bstr().to_owned();
    let mut lines = Vec::new();
    lines.push("Preview".to_string());
    lines.push(String::new());

    let matching_segment = result
        .workspace
        .head_info
        .stacks
        .iter()
        .flat_map(|stack| stack.segments.iter())
        .find(|segment| {
            segment
                .ref_info
                .as_ref()
                .map(|reference| reference.ref_name.as_ref().as_bstr())
                .is_some_and(|ref_name| ref_name == target_branch.as_bstr())
        });

    let Some(segment) = matching_segment else {
        lines.push("(branch not present in preview workspace)".into());
        lines.push(String::new());
        return lines.join("\n");
    };

    let branch_name = segment
        .ref_info
        .as_ref()
        .map(|reference| reference.ref_name.shorten().to_string())
        .unwrap_or_else(|| branch_ref.shorten().to_string());
    lines.push(format!("* {}", t.local_branch.paint(branch_name)));
    let represented_remote_ids: HashSet<_> = segment
        .commits
        .iter()
        .filter_map(|commit| match commit.relation {
            but_workspace::ref_info::LocalCommitRelation::LocalOnly => None,
            but_workspace::ref_info::LocalCommitRelation::LocalAndRemote(remote_id)
            | but_workspace::ref_info::LocalCommitRelation::Integrated(remote_id) => {
                Some(remote_id)
            }
        })
        .collect();
    let local_commit_rows = segment
        .commits
        .iter()
        .map(|commit| {
            let style = match commit.relation {
                but_workspace::ref_info::LocalCommitRelation::Integrated(_) => {
                    CommitRenderStyle::Integrated
                }
                but_workspace::ref_info::LocalCommitRelation::LocalOnly
                | but_workspace::ref_info::LocalCommitRelation::LocalAndRemote(_) => {
                    CommitRenderStyle::LocalOnly
                }
            };
            let message = commit
                .inner
                .message
                .lines()
                .next()
                .map(|line| line.to_str_lossy().into_owned())
                .unwrap_or_default();
            PreviewCommitRow {
                style,
                change_id_prefix: change_id_prefix(
                    commit.inner.change_id.as_ref().map(|id| id.as_ref()),
                ),
                short_id: commit.inner.id.to_hex_with_len(7).to_string(),
                subject: message,
                has_conflicts: commit.inner.has_conflicts,
            }
        })
        .collect::<Vec<_>>();
    for row in local_commit_rows.iter() {
        lines.push(format_commit_row(
            "",
            row.style,
            row.change_id_prefix.clone(),
            row.short_id.clone(),
            None,
            Some(row.subject.as_str()),
            row.has_conflicts,
        ));
    }
    for commit in &segment.commits_on_remote {
        if represented_remote_ids.contains(&commit.id) {
            continue;
        }
        let message = commit
            .message
            .lines()
            .next()
            .map(|line| line.to_str_lossy().into_owned())
            .unwrap_or_default();
        lines.push(format_commit_row(
            "┊",
            CommitRenderStyle::Upstream,
            change_id_prefix(commit.change_id.as_deref().map(|id| id.as_ref())),
            commit.id.to_hex_with_len(7).to_string(),
            None,
            Some(message.as_str()),
            false,
        ));
    }
    if let Some(base) = segment.base {
        lines.push(format_base_row(
            "",
            base.to_hex_with_len(7).to_string(),
            None,
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

struct PreviewCommitRow {
    style: CommitRenderStyle,
    change_id_prefix: String,
    short_id: String,
    subject: String,
    has_conflicts: bool,
}

fn contiguous_integrated_divergence_count(
    commits: &[IntegrationDivergenceCommit],
) -> (usize, usize) {
    let total = commits.len();
    let count = commits
        .iter()
        .rev()
        .take_while(|commit| {
            matches!(
                commit.target_relation,
                json::IntegrationDivergenceTargetRelation::HistoricallyIntegrated { .. }
            )
        })
        .count();
    let count = if count >= 2 { count } else { 0 };
    let rest = total - count;
    (count, rest)
}

fn format_collapsed_integrated_summary(hidden_count: usize) -> String {
    let t = theme::get();
    let noun = if hidden_count == 1 {
        "commit"
    } else {
        "commits"
    };
    t.hint
        .paint(format!(
            "~\n... {hidden_count} integrated local {noun} hidden\n~"
        ))
        .to_string()
}

fn format_commit_row(
    connector: &str,
    style: CommitRenderStyle,
    change_id_prefix: String,
    short_id: String,
    refs: Option<&str>,
    subject: Option<&str>,
    has_conflicts: bool,
) -> String {
    let t = theme::get();
    let dot = match style {
        CommitRenderStyle::LocalOnly => "●".to_string(),
        CommitRenderStyle::Upstream => t.attention.paint("●").to_string(),
        CommitRenderStyle::Integrated => t.remote_branch.paint("●").to_string(),
    };
    let refs = refs.map(|refs| format!(" ({refs})")).unwrap_or_default();
    let subject = subject
        .filter(|subject| !subject.is_empty())
        .map(|subject| format!(" {subject}"))
        .unwrap_or_default();
    let conflicted = if has_conflicts {
        format!(" {}", t.error.paint("{conflicted}"))
    } else {
        Default::default()
    };
    format!(
        "{connector}{dot} {change_id_prefix} {}{refs}{subject}{conflicted}",
        t.commit_id.paint(short_id),
    )
}

fn format_base_row(connector: &str, short_id: String, subject: Option<&str>) -> String {
    let t = theme::get();
    let subject = subject
        .filter(|subject| !subject.is_empty())
        .map(|subject| format!(" {subject}"))
        .unwrap_or_default();
    format!("{connector}o {}{subject}", t.commit_id.paint(short_id),)
}

fn shorten_full_ref_name(ref_name: &api_json::FullRefName) -> &str {
    ref_name
        .full
        .strip_prefix("refs/heads/")
        .or_else(|| ref_name.full.strip_prefix("refs/remotes/"))
        .unwrap_or(ref_name.full.as_str())
}

fn short_hex_json(hex: &impl serde::Serialize) -> String {
    serde_json::to_value(hex)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .map(|hex| hex.get(..7).unwrap_or(hex.as_str()).to_owned())
        .unwrap_or_else(|| "<invalid>".into())
}

fn change_id_prefix(change_id: Option<&[u8]>) -> String {
    change_id
        .map(|change_id| {
            change_id
                .as_bstr()
                .to_str_lossy()
                .chars()
                .take(2)
                .collect::<String>()
        })
        .filter(|prefix| prefix.chars().count() == 2)
        .unwrap_or_else(|| "__".to_owned())
}

impl From<IntegrationStrategy> for JsonBranchIntegrationStrategy {
    fn from(value: IntegrationStrategy) -> Self {
        match value {
            IntegrationStrategy::PullRebase => Self::PullRebase,
            IntegrationStrategy::SmartSquash => Self::SmartSquash,
            IntegrationStrategy::Merge => Self::Merge,
            IntegrationStrategy::PickRemote => Self::PickRemote,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_row_marks_conflicted_commits() {
        let row = format_commit_row(
            "",
            CommitRenderStyle::LocalOnly,
            "__".to_owned(),
            "abc1234".to_owned(),
            None,
            Some("subject"),
            true,
        );

        let row = strip_ansi_codes(&row);

        assert!(
            row.contains("subject {conflicted}"),
            "conflicted commit rows should carry the same marker used by status output"
        );
    }

    #[test]
    fn commit_row_leaves_unconflicted_commits_unmarked() {
        let row = format_commit_row(
            "",
            CommitRenderStyle::LocalOnly,
            "__".to_owned(),
            "abc1234".to_owned(),
            None,
            Some("subject"),
            false,
        );

        assert!(
            !row.contains("{conflicted}"),
            "unconflicted commit rows should not show the conflict marker"
        );
    }
}

use super::FileChange;
use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;
use but_ctx::access::RepoShared;

use crate::{
    CLI_DATE, CliId, CliResult, IdMap,
    args::atoms::CliIdArg,
    bad_input,
    id::CommitId,
    theme::{self, Paint, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, get_change_id_for_commit,
        shorten_object_id, time::format_relative_time,
    },
};

pub(crate) fn show_commit(
    ctx: &mut Context,
    out: IntermediateChannel<'_>,
    commit_id_str: &str,
    verbose: bool,
    stat: bool,
) -> CliResult<ShowOutcome> {
    // JSON always carries the details, so tools don't need extra flags.
    let is_json = out.format().is_json();
    let id_map = IdMap::legacy_new_from_context(ctx)?;
    // A branch's verbose output already has the line counts.
    let verbose = verbose || stat;

    let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
    let branch_match = branches.iter().find(|b| {
        b.name.as_bytes() == commit_id_str.as_bytes()
            || b.name.to_str_lossy().to_lowercase() == commit_id_str.to_lowercase()
    });

    if let Some(branch) = branch_match {
        return show_branch(ctx, is_json, &branch.name.to_string(), verbose, &id_map);
    }

    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;

    for stack in &stacks {
        for branch in &stack.branches {
            let head_name = branch.name.clone();
            if head_name == commit_id_str
                || head_name.to_lowercase() == commit_id_str.to_lowercase()
            {
                return show_branch(ctx, is_json, &head_name, verbose, &id_map);
            }
        }
    }

    // Not a branch, resolve the commit ID through the IdMap
    let cli_ids = id_map.parse_using_context(commit_id_str, ctx)?;

    let commit_id = if cli_ids.is_empty() {
        // If not found in IdMap, try to parse as a git commit ID directly
        let repo = ctx.repo.get()?;
        let obj = repo.rev_parse_single(commit_id_str).map_err(|_| {
            bad_input(format!("Commit '{commit_id_str}' not found"))
                .hint(CliIdArg::TARGET_MISSING_HINT)
        })?;
        let commit = obj.object()?.try_into_commit().map_err(|_| {
            bad_input(format!("'{commit_id_str}' is not a commit"))
                .hint(CliIdArg::TARGET_MISSING_HINT)
        })?;
        commit.id
    } else if cli_ids.len() > 1 {
        return Err(bad_input(format!(
            "Commit ID '{}' is ambiguous. Found {} matches",
            commit_id_str,
            cli_ids.len()
        ))
        .hint(CliIdArg::AMBIGUOUS_HINT)
        .into());
    } else {
        match &cli_ids[0] {
            CliId::Commit {
                commit: CommitId { commit_id, .. },
                id: _,
            } => *commit_id,
            CliId::Branch(branch) => {
                return show_branch(ctx, is_json, &branch.name, verbose, &id_map);
            }
            _ => {
                return Err(bad_input(format!(
                    "Target must be a commit ID or branch, not {}",
                    cli_ids[0].kind_for_humans()
                ))
                .hint(CliIdArg::TARGET_MISSING_HINT)
                .into());
            }
        }
    };

    let repo = ctx.repo.get()?;
    let raw_commit = repo.find_commit(commit_id)?;
    let decoded = raw_commit.decode()?;

    let parent_id = raw_commit.parent_ids().next().map(|id| id.detach());
    let tree_changes = but_core::diff::TreeChanges::from_trees(&repo, parent_id, commit_id)?;

    // Extract change-id if present (try both header names)
    let change_id = decoded
        .extra_headers()
        .find("change-id")
        .or_else(|| decoded.extra_headers().find("gitbutler-change-id"))
        .map(|v| v.to_str_lossy().to_string());

    let author_sig = decoded.author()?;
    let committer_sig = decoded.committer()?;
    let commit_time = raw_commit.time()?;
    let changes = tree_changes.into_tree_changes();
    let line_counts = (stat || is_json)
        .then(|| {
            changes
                .iter()
                .map(|change| super::file_change_from_tree_change(&repo, change.clone()))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;

    Ok(ShowOutcome::Commit(CommitDetails {
        commit_id,
        change_id,
        author: (
            author_sig.name.to_str_lossy().into_owned(),
            author_sig.email.to_str_lossy().into_owned(),
        ),
        committer: (
            committer_sig.name.to_str_lossy().into_owned(),
            committer_sig.email.to_str_lossy().into_owned(),
        ),
        date: commit_time.format(CLI_DATE)?,
        seconds: commit_time.seconds,
        message: decoded.message.to_str_lossy().into_owned(),
        changes,
        line_counts,
    }))
}

#[must_use]
pub(crate) enum ShowOutcome {
    Commit(CommitDetails),
    Branch(BranchDetails),
}

pub(crate) struct CommitDetails {
    commit_id: gix::ObjectId,
    change_id: Option<String>,
    /// The name and email of the author.
    author: (String, String),
    /// The name and email of the committer.
    committer: (String, String),
    date: String,
    /// The commit time in seconds since the Unix epoch.
    seconds: i64,
    message: String,
    changes: Vec<but_core::TreeChange>,
    /// The line counts of `changes`, computed for `--stat` and JSON output.
    line_counts: Option<Vec<FileChange>>,
}

pub(crate) struct BranchDetails {
    branch_name: String,
    verbose: bool,
    commits: Vec<BranchCommitInfo>,
    base_commit_info: Option<BranchCommitInfo>,
    stack_chain: Vec<StackChainBranch>,
}

impl CliOutputHuman for ShowOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        t: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            ShowOutcome::Commit(details) => show_commit_human(details, out, t),
            ShowOutcome::Branch(details) => show_branch_human(details, out, t),
        }
    }
}

impl CliOutput for ShowOutcome {
    fn on_json(self) -> impl serde::Serialize {
        match self {
            ShowOutcome::Commit(CommitDetails {
                commit_id,
                change_id,
                author,
                committer,
                date,
                seconds: _,
                message,
                changes: _,
                line_counts,
            }) => {
                let mut json_output = serde_json::json!({
                    "commit": commit_id.to_string(),
                    "author": {
                        "name": author.0,
                        "email": author.1
                    },
                    "committer": {
                        "name": committer.0,
                        "email": committer.1
                    },
                    "date": date,
                    "message": message,
                    "files": line_counts.unwrap_or_default()
                });

                if let Some(change_id) = change_id {
                    json_output["changeId"] = serde_json::json!(change_id);
                }
                json_output
            }
            ShowOutcome::Branch(BranchDetails {
                branch_name,
                verbose: _,
                commits,
                base_commit_info,
                stack_chain,
            }) => serde_json::json!({
                "branch": branch_name,
                "commits": commits,
                "stackedOn": stack_chain,
                "baseCommit": base_commit_info,
            }),
        }
    }
}

fn show_commit_human(
    details: CommitDetails,
    out: &mut dyn WriteWithUtils,
    t: &Theme,
) -> anyhow::Result<()> {
    let CommitDetails {
        commit_id,
        change_id,
        author,
        committer,
        date,
        seconds,
        message,
        changes,
        line_counts,
    } = details;
    writeln!(
        out,
        "{} {}",
        t.important.paint("Commit:   "),
        t.commit_id.paint(commit_id.to_string()),
    )?;

    if let Some(ref change_id) = change_id {
        writeln!(out, "{} {}", t.important.paint("Change-ID:"), change_id)?;
    }

    writeln!(
        out,
        "{} {} <{}>",
        t.important.paint("Author:   "),
        t.user.paint(&author.0),
        t.user.paint(&author.1)
    )?;

    let relative = format_relative_time(std::time::SystemTime::now(), seconds);
    writeln!(
        out,
        "{}  {} {}",
        t.important.paint("Date:    "),
        t.time.paint(date),
        t.hint.paint(format!("({relative})"))
    )?;

    // Committer (only if different from author)
    if committer != author {
        writeln!(
            out,
            "{} {} <{}>",
            t.important.paint("Committer:"),
            t.user.paint(&committer.0),
            t.user.paint(&committer.1)
        )?;
    }

    writeln!(out)?;

    // Commit message - first line bold, rest normal, no indentation
    let mut lines = message.lines();
    if let Some(first_line) = lines.next() {
        writeln!(out, "{}", t.important.paint(first_line))?;
        for line in lines {
            writeln!(out, "{line}")?;
        }
    }

    writeln!(out)?;

    if !changes.is_empty() {
        writeln!(out, "{}", t.important.paint("Files changed:"))?;
        for (idx, change) in changes.iter().enumerate() {
            let (status_char, status_color) = match &change.status {
                but_core::TreeStatus::Addition { .. } => ("A", t.addition),
                but_core::TreeStatus::Deletion { .. } => ("D", t.deletion),
                but_core::TreeStatus::Modification { .. } => ("M", t.modification),
                but_core::TreeStatus::Rename { .. } => ("R", t.renaming),
            };

            let line_counts = match line_counts.as_ref().and_then(|files| files.get(idx)) {
                Some(file) => format!(
                    "  ({}, {})",
                    t.addition.paint(format!("+{}", file.insertions)),
                    t.deletion.paint(format!("-{}", file.deletions))
                ),
                None => String::new(),
            };
            writeln!(
                out,
                "  {} {}{line_counts}",
                status_color.paint(status_char),
                change.path.to_str_lossy()
            )?;
        }
    }

    Ok(())
}

fn show_branch(
    ctx: &mut Context,
    is_json: bool,
    branch_name: &str,
    verbose: bool,
    id_map: &IdMap,
) -> CliResult<ShowOutcome> {
    // In JSON mode, always include file info (verbose) so agents and tools
    // get complete data without needing to pass extra flags.
    let effective_verbose = verbose || is_json;
    let (commits, base_commit_info) =
        get_branch_commits(ctx, branch_name, effective_verbose, id_map)?;

    // Get the stack chain (branches this branch is stacked on)
    let stack_chain = get_stack_chain(ctx, branch_name)?;

    Ok(ShowOutcome::Branch(BranchDetails {
        branch_name: branch_name.to_owned(),
        verbose,
        commits,
        base_commit_info,
        stack_chain,
    }))
}

fn show_branch_human(
    details: BranchDetails,
    out: &mut dyn WriteWithUtils,
    t: &Theme,
) -> anyhow::Result<()> {
    let BranchDetails {
        branch_name,
        verbose,
        commits,
        base_commit_info,
        stack_chain,
    } = details;
    writeln!(
        out,
        "{} {}",
        t.important.paint("Branch:"),
        t.local_branch.paint(branch_name)
    )?;
    writeln!(out)?;

    if commits.is_empty() {
        writeln!(out, "No commits on this branch.")?;
    } else {
        writeln!(out, "{}", t.important.paint("Commits:"))?;
        for (idx, commit) in commits.iter().enumerate() {
            if verbose {
                let now_t = std::time::SystemTime::now();
                // Verbose mode: show full commit message and files with visual separators
                writeln!(
                    out,
                    "{} {} {}",
                    t.info.paint("●"),
                    commit.display_ref(),
                    t.important.paint(&commit.message),
                )?;
                writeln!(
                    out,
                    "{} {}, {} by {}",
                    t.border.paint("│"),
                    t.time.paint(format_timestamp(commit.timestamp)),
                    t.hint.paint(format_relative_time(now_t, commit.timestamp)),
                    t.user.paint(&commit.author_name)
                )?;

                if let Some(full_message) = &commit.full_message {
                    let lines: Vec<&str> = full_message.lines().collect();
                    if lines.len() > 1 {
                        for line in lines.iter().skip(1) {
                            if !line.is_empty() {
                                writeln!(out, "{} {}", t.border.paint("│"), line)?;
                            } else {
                                writeln!(out, "{}", t.border.paint("│"))?;
                            }
                        }
                    }
                }

                if let Some(files) = &commit.files
                    && !files.is_empty()
                {
                    writeln!(out, "{}", t.border.paint("│"))?;
                    writeln!(
                        out,
                        "{} {}:",
                        t.border.paint("│"),
                        t.hint.paint("Files changed")
                    )?;
                    for file in files {
                        let status_str = match file.status.as_str() {
                            "added" => format!("A {}", t.addition.paint(&file.path)),
                            "deleted" => format!("D {}", t.deletion.paint(&file.path)),
                            "modified" => format!("M {}", t.modification.paint(&file.path)),
                            _ => format!("  {}", file.path),
                        };
                        writeln!(
                            out,
                            "{} {}  ({}, {})",
                            t.border.paint("│"),
                            status_str,
                            t.addition.paint(format!("+{}", file.insertions)),
                            t.deletion.paint(format!("-{}", file.deletions))
                        )?;
                    }
                }

                if let (Some(files_changed), Some(insertions), Some(deletions)) =
                    (commit.files_changed, commit.insertions, commit.deletions)
                {
                    writeln!(out, "{}", t.border.paint("│"))?;
                    writeln!(
                        out,
                        "{} {} file{} changed, {} insertion{}, {} deletion{}",
                        t.border.paint("│"),
                        files_changed,
                        if files_changed == 1 { "" } else { "s" },
                        insertions,
                        if insertions == 1 { "" } else { "s" },
                        deletions,
                        if deletions == 1 { "" } else { "s" }
                    )?;
                }

                if idx < commits.len() - 1 {
                    writeln!(out, "{}", t.border.paint("│"))?;
                }
            } else {
                writeln!(out, "  {} {}", commit.display_ref(), commit.message)?;
                writeln!(
                    out,
                    "    {} by {}",
                    t.time.paint(format_timestamp(commit.timestamp)),
                    t.user.paint(&commit.author_name)
                )?;
            }
        }

        if verbose {
            if let Some(base_commit) = &base_commit_info {
                let now_t = std::time::SystemTime::now();
                writeln!(out, "{}", t.border.paint("│"))?;
                writeln!(
                    out,
                    "{} {} {} {}",
                    t.border.paint("┴"),
                    base_commit.display_ref(),
                    t.hint.paint(&base_commit.message),
                    t.hint.paint("(base)")
                )?;
                writeln!(
                    out,
                    "  {}, {} by {}",
                    t.time.paint(format_timestamp(base_commit.timestamp)),
                    t.hint
                        .paint(format_relative_time(now_t, base_commit.timestamp)),
                    t.user.paint(&base_commit.author_name)
                )?;
            }

            show_branch_summary(out, &commits)?;
        }
    }

    if !stack_chain.is_empty() {
        writeln!(out)?;
        writeln!(out, "{}", t.important.paint("Stacked on:"))?;
        for (i, chain_branch) in stack_chain.iter().enumerate() {
            let connector = if i == stack_chain.len() - 1 {
                "└─"
            } else {
                "├─"
            };
            writeln!(
                out,
                "  {} {} ({})",
                connector,
                t.local_branch.paint(&chain_branch.name),
                t.hint.paint(format!(
                    "{} commit{}",
                    chain_branch.commit_count,
                    if chain_branch.commit_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                ))
            )?;
        }
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct BranchCommitInfo {
    #[serde(skip)]
    change_id: but_core::ChangeId,
    sha: String,
    short_sha: String,
    /// The stable change-ID ref shown by `but status`, when the commit has one.
    cli_id: Option<String>,
    message: String,
    full_message: Option<String>,
    author_name: String,
    author_email: String,
    timestamp: i64,
    files_changed: Option<usize>,
    insertions: Option<usize>,
    deletions: Option<usize>,
    files: Option<Vec<FileChange>>,
}

impl BranchCommitInfo {
    /// The commit ref for display: the stable change-ID ref when the commit
    /// has one, its short sha otherwise.
    fn display_ref(&self) -> String {
        theme::Commit(self.change_id.clone()).to_string()
    }
}

#[derive(Debug, serde::Serialize)]
struct StackChainBranch {
    name: String,
    commit_count: usize,
}

/// Checks both `list_branches` and the applied stacks.
fn find_branch_oid(ctx: &Context, branch_name: &str) -> Result<gix::ObjectId> {
    let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
    if let Some(branch) = branches
        .iter()
        .find(|b| b.name.as_bytes() == branch_name.as_bytes())
    {
        return Ok(branch.head);
    }

    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;

    for stack in &stacks {
        for branch in &stack.branches {
            if branch.name == branch_name {
                return Ok(branch.tip);
            }
        }
    }

    anyhow::bail!("Branch '{branch_name}' not found")
}

fn get_branch_commits(
    ctx: &Context,
    branch_name: &str,
    verbose: bool,
    id_map: &IdMap,
) -> Result<(Vec<BranchCommitInfo>, Option<BranchCommitInfo>)> {
    use gix::prelude::ObjectIdExt as _;

    let repo = ctx.repo.get()?;

    let branch_oid = find_branch_oid(ctx, branch_name)?;

    let branch_base = crate::legacy::workspace::applied_stacks(ctx)?
        .iter()
        .find_map(|stack| stack.branch(branch_name))
        .map(|branch| branch.base_commit);

    // Workspace branches are buckets, not the entire history back to the target.
    let guard = ctx.shared_worktree_access();
    let cli_id_for = |oid: gix::ObjectId| {
        id_map
            .change_id_ref(oid)
            .map(|change_id| change_id.padded_short_id())
    };
    let Some(merge_base) = (match branch_base {
        Some(base) => Some(base),
        None => merge_base_with_target_commit(ctx, guard.read_permission(), branch_oid)?,
    }) else {
        tracing::warn!(
            branch_name,
            "Could not find merge base with target branch, which is unexpected"
        );
        return Ok((Vec::new(), None));
    };

    let traversal = branch_oid
        .attach(&repo)
        .ancestors()
        .with_hidden(Some(merge_base))
        .all()?;

    let mut commits = Vec::new();
    for info in traversal {
        let info = info?;
        let commit = repo.find_commit(info.id)?;
        let commit = commit.decode()?;
        let author = commit.author()?;
        let committer = commit.committer()?;

        let full_message = (!commit.message.is_empty()).then(|| commit.message.to_string());
        let message = super::commit_summary(&commit);

        let (files_changed, insertions, deletions, files) = if verbose {
            let changes = but_core::diff::commit_changes(info.id())?;
            let stats = changes.compute_line_stats(&repo)?;
            let file_changes = changes
                .into_tree_changes()
                .into_iter()
                .map(|change| super::file_change_from_tree_change(&repo, change))
                .collect::<Result<Vec<_>>>()?;

            (
                Some(usize::try_from(stats.files_changed).unwrap_or(usize::MAX)),
                Some(usize::try_from(stats.lines_added).unwrap_or(usize::MAX)),
                Some(usize::try_from(stats.lines_removed).unwrap_or(usize::MAX)),
                Some(file_changes),
            )
        } else {
            (None, None, None, None)
        };

        commits.push(BranchCommitInfo {
            change_id: match id_map.change_id_ref(info.id).map(|c| c.change_id.clone()) {
                Some(id) => id,
                None => get_change_id_for_commit(&repo, info.id)?,
            },
            sha: info.id.to_string(),
            short_sha: shorten_object_id(&repo, info.id),
            cli_id: cli_id_for(info.id),
            message,
            full_message,
            author_name: author.name.to_str_lossy().to_string(),
            author_email: author.email.to_str_lossy().to_string(),
            timestamp: committer.time()?.seconds,
            files_changed,
            insertions,
            deletions,
            files,
        });
    }

    let base_commit_info = if verbose {
        let base_commit = repo.find_commit(merge_base)?;
        let base_commit = base_commit.decode()?;
        let base_author = base_commit.author()?;
        let base_message = super::commit_summary(&base_commit);

        Some(BranchCommitInfo {
            change_id: crate::utils::get_change_id_for_commit(&repo, merge_base)?,
            sha: merge_base.to_string(),
            short_sha: shorten_object_id(&repo, merge_base),
            cli_id: None,
            message: base_message,
            full_message: None,
            author_name: base_author.name.to_str_lossy().to_string(),
            author_email: base_author.email.to_str_lossy().to_string(),
            timestamp: base_commit.committer()?.time()?.seconds,
            files_changed: None,
            insertions: None,
            deletions: None,
            files: None,
        })
    } else {
        None
    };

    Ok((commits, base_commit_info))
}

fn merge_base_with_target_commit(
    ctx: &Context,
    perm: &RepoShared,
    branch_oid: gix::ObjectId,
) -> Result<Option<gix::ObjectId>> {
    let (_, workspace, _) = ctx.workspace_and_db_with_perm(perm)?;
    Ok(workspace
        .merge_base_with_target_commit(branch_oid)
        .map(|t| t.0))
}

fn format_timestamp(timestamp: i64) -> String {
    use chrono::{DateTime, Local, Utc};

    let datetime = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_else(Utc::now)
        .with_timezone(&Local);

    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn show_branch_summary(out: &mut dyn std::fmt::Write, commits: &[BranchCommitInfo]) -> Result<()> {
    use std::collections::HashMap;
    let t = theme::get();

    writeln!(out)?;
    writeln!(out, "{}", t.border.paint("─".repeat(50)))?;
    writeln!(out)?;
    writeln!(out, "{}", t.important.paint("Branch Summary:"))?;

    writeln!(
        out,
        "  {} commit{}",
        commits.len(),
        if commits.len() == 1 { "" } else { "s" }
    )?;

    let mut file_changes: HashMap<String, (usize, usize)> = HashMap::new();
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    for commit in commits {
        if let Some(files) = &commit.files {
            for file in files {
                let entry = file_changes.entry(file.path.clone()).or_insert((0, 0));
                entry.0 += file.insertions;
                entry.1 += file.deletions;
                total_insertions += file.insertions;
                total_deletions += file.deletions;
            }
        }
    }

    if !file_changes.is_empty() {
        writeln!(
            out,
            "  {} file{} changed",
            file_changes.len(),
            if file_changes.len() == 1 { "" } else { "s" }
        )?;
        writeln!(
            out,
            "  {} insertion{} (+)",
            total_insertions,
            if total_insertions == 1 { "" } else { "s" }
        )?;
        writeln!(
            out,
            "  {} deletion{} (-)",
            total_deletions,
            if total_deletions == 1 { "" } else { "s" }
        )?;

        let mut files_vec: Vec<_> = file_changes.iter().collect();
        files_vec.sort_by_key(|file| std::cmp::Reverse(file.1.0 + file.1.1));

        writeln!(out)?;
        writeln!(out, "  {}:", t.hint.paint("Files changed"))?;
        for (path, (insertions, deletions)) in files_vec.iter().take(10) {
            writeln!(
                out,
                "    {} ({}, {})",
                path,
                t.addition.paint(format!("+{insertions}")),
                t.deletion.paint(format!("-{deletions}"))
            )?;
        }

        if files_vec.len() > 10 {
            writeln!(
                out,
                "    ... and {} more file{}",
                files_vec.len() - 10,
                if files_vec.len() - 10 == 1 { "" } else { "s" }
            )?;
        }
    }

    Ok(())
}

fn get_stack_chain(ctx: &Context, branch_name: &str) -> Result<Vec<StackChainBranch>> {
    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;

    let stack = stacks
        .iter()
        .find(|stack| stack.contains_branch(branch_name));

    let Some(stack) = stack else {
        // Branch not in a stack or is the only branch
        return Ok(vec![]);
    };

    let branch_index = stack
        .branches
        .iter()
        .position(|branch| branch.name == branch_name);

    let Some(branch_index) = branch_index else {
        return Ok(vec![]);
    };

    // Branches are ordered tip-to-base, so dependencies follow this branch.
    let mut chain = Vec::new();
    for branch in &stack.branches[branch_index + 1..] {
        chain.push(StackChainBranch {
            name: branch.name.clone(),
            commit_count: branch.commits.len(),
        });
    }

    Ok(chain)
}

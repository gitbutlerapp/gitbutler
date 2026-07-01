use super::super::FileChange;
use bstr::ByteSlice;
use but_ctx::Context;
use but_llm::ChatMessage;
use tracing::instrument;

use crate::{
    CliResult, IdMap,
    args::atoms::{BranchArg, CliIdArg},
    command::legacy::workspace_target,
    theme::{self, Paint},
    utils::{OutputChannel, shorten_object_id},
};

pub fn show(
    ctx: &mut Context,
    branch_arg: CliIdArg,
    out: &mut OutputChannel,
    review: bool,
    show_files: bool,
    generate_ai_summary: bool,
    check_merge: bool,
) -> CliResult<()> {
    let branch_arg = {
        let guard = ctx.exclusive_worktree_access();
        let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
        let repo = ctx.repo.get()?;
        branch_arg
            .try_resolve_branch(&repo, &id_map)?
            .unwrap_or(BranchArg(branch_arg.0))
    };

    // Get the list of commits ahead of base for this branch
    let commits = get_commits_ahead(ctx, &branch_arg, show_files)?;

    // Get uncommitted files for this branch
    let uncommitted_files = get_uncommitted_files(ctx, &branch_arg)?;

    let branch_name = &branch_arg.0;

    // Get review information if requested
    let reviews = if review {
        crate::command::legacy::forge::review::get_review_map(
            ctx,
            Some(but_forge::CacheConfig::CacheOnly),
        )?
        .get(branch_name)
        .cloned()
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Generate AI summary if requested
    let ai_summary = if generate_ai_summary {
        let git_config = gix::config::File::from_globals()?;
        Some(generate_branch_summary(branch_name, &commits, &git_config)?)
    } else {
        None
    };

    // Check merge conflicts if requested
    let merge_check = if check_merge {
        Some(check_merge_conflicts(ctx, branch_name)?)
    } else {
        None
    };

    if let Some(out) = out.for_json() {
        output_json(
            branch_name,
            &commits,
            &uncommitted_files,
            &reviews,
            ai_summary.as_deref(),
            merge_check.as_ref(),
            out,
        )?;
    } else if let Some(out) = out.for_human() {
        output_human(
            branch_name,
            &commits,
            &uncommitted_files,
            &reviews,
            ai_summary.as_deref(),
            merge_check.as_ref(),
            out,
        )?;
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct MergeCheck {
    merges_cleanly: bool,
    conflicting_files: Vec<ConflictingFile>,
}

#[derive(Debug, serde::Serialize)]
struct ConflictingFile {
    path: String,
    branch_commits: Vec<CommitRef>,
    upstream_commits: Vec<CommitRef>,
}

#[derive(Debug, serde::Serialize)]
struct CommitRef {
    sha: String,
    short_sha: String,
    message: String,
    author_name: String,
    timestamp: i64,
}

// TODO(perf): re-write with `gix`, just avoid duplicate work there (get_merge_conflict_paths) isn't needed.
// TODO(perf): re-write this to use a graph that includes all branches from the listing, needs a listing that uses
//             the graph.
fn check_merge_conflicts(ctx: &Context, branch_name: &str) -> CliResult<MergeCheck> {
    use but_core::RepositoryExt;

    let guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    let branch = BranchArg(branch_name.to_owned()).resolve_branch(&repo)?;

    // Find merge base
    let (merge_base, target) = workspace_target::merge_base_with_target_with_perm(
        ctx,
        guard.read_permission(),
        branch.head,
    )?;
    let merge_repo = ctx.clone_repo_for_merging_non_persisting()?;
    let merge_base_tree_id = repo.find_commit(merge_base)?.tree_id()?.detach();
    let target_tree_id = repo.find_commit(target.oid())?.tree_id()?.detach();
    let branch_tree_id = repo.find_commit(branch.head)?.tree_id()?.detach();

    // Check if branch merges cleanly into target
    let merges_cleanly =
        merge_repo.merges_cleanly(merge_base_tree_id, target_tree_id, branch_tree_id)?;

    let mut conflicting_files = Vec::new();

    // If there are conflicts, identify which files conflict and which commits modified them
    if !merges_cleanly {
        // Get the list of conflicting files from the merge
        let conflict_paths =
            get_merge_conflict_paths(&repo, merge_base_tree_id, target_tree_id, branch_tree_id)?;

        // For each conflicting file, find which commits on both sides modified it
        for path in conflict_paths {
            let branch_commits =
                find_commits_modifying_file(&repo, &path, merge_base, branch.head)?;

            let upstream_commits =
                find_commits_modifying_file(&repo, &path, merge_base, target.oid())?;

            conflicting_files.push(ConflictingFile {
                path,
                branch_commits,
                upstream_commits,
            });
        }
    }

    Ok(MergeCheck {
        merges_cleanly,
        conflicting_files,
    })
}

fn get_merge_conflict_paths(
    gix_repo: &gix::Repository,
    base_tree: gix::ObjectId,
    ours_tree: gix::ObjectId,
    theirs_tree: gix::ObjectId,
) -> Result<Vec<String>, anyhow::Error> {
    use but_core::RepositoryExt;

    let (options, conflict_kind) = gix_repo.merge_options_fail_fast()?;
    let merge_result = gix_repo.merge_trees(
        base_tree,
        ours_tree,
        theirs_tree,
        gix_repo.default_merge_labels(),
        options,
    )?;

    let mut paths = Vec::new();

    // Extract conflicting file paths
    if merge_result.has_unresolved_conflicts(conflict_kind) {
        for conflict in merge_result.conflicts.iter() {
            if conflict.is_unresolved(conflict_kind) {
                let path = gix::path::from_bstr(conflict.ours.location())
                    .to_string_lossy()
                    .to_string();
                paths.push(path);
            }
        }
    }

    Ok(paths)
}

// TODO(perf): we need a common mechanism for doing this and let it create a per-commit cache per repository.
fn find_commits_modifying_file(
    repo: &gix::Repository,
    path: &str,
    from_commit: gix::ObjectId,
    to_commit: gix::ObjectId,
) -> Result<Vec<CommitRef>, anyhow::Error> {
    use gix::prelude::ObjectIdExt as _;

    let traversal = to_commit
        .attach(repo)
        .ancestors()
        .with_hidden(Some(from_commit))
        .all()?;

    let mut commits = Vec::new();
    let path = path.as_bytes().as_bstr();

    for info in traversal {
        let info = info?;
        let commit = repo.find_commit(info.id)?;
        let commit = commit.decode()?;

        // Check if this commit modified the file
        let modified_file = but_core::diff::commit_changes(info.id())?
            .into_tree_changes()
            .into_iter()
            .any(|change| {
                change.path.as_bstr() == path
                    || change.previous_path().is_some_and(|prev| prev == path)
            });

        if modified_file {
            let author = commit.author()?;
            commits.push(CommitRef {
                sha: info.id.to_string(),
                short_sha: shorten_object_id(repo, info.id),
                message: super::super::commit_summary(&commit),
                author_name: author.name.to_str_lossy().to_string(),
                timestamp: commit.committer()?.time()?.seconds,
            });
        }
    }

    Ok(commits)
}

fn get_commits_ahead(
    ctx: &Context,
    branch_arg: &BranchArg,
    show_files: bool,
) -> CliResult<Vec<CommitInfo>> {
    use gix::prelude::ObjectIdExt as _;

    let guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;
    let branch = branch_arg.resolve_branch(&repo)?;

    // Find merge base
    let (merge_base, _) = workspace_target::merge_base_with_target_with_perm(
        ctx,
        guard.read_permission(),
        branch.head,
    )?;

    // Walk from branch head to merge base, collecting commits
    let traversal = branch
        .head
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
        let changes = but_core::diff::commit_changes(info.id())?;
        let stats = changes.compute_line_stats(&repo)?;

        // Collect per-file stats if requested
        let files = if show_files {
            changes
                .clone()
                .into_tree_changes()
                .into_iter()
                .map(|change| super::super::file_change_from_tree_change(&repo, change))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };

        commits.push(CommitInfo {
            sha: info.id.to_string(),
            short_sha: shorten_object_id(&repo, info.id),
            message: super::super::commit_summary(&commit),
            author_name: author.name.to_str_lossy().to_string(),
            author_email: author.email.to_str_lossy().to_string(),
            timestamp: commit.committer()?.time()?.seconds,
            files_changed: usize::try_from(stats.files_changed).unwrap_or(usize::MAX),
            insertions: usize::try_from(stats.lines_added).unwrap_or(usize::MAX),
            deletions: usize::try_from(stats.lines_removed).unwrap_or(usize::MAX),
            files,
        });
    }

    Ok(commits)
}

fn get_uncommitted_files(ctx: &mut Context, branch_arg: &BranchArg) -> anyhow::Result<Vec<String>> {
    use std::collections::BTreeMap;

    use bstr::{BString, ByteSlice};
    use but_hunk_assignment::HunkAssignment;

    let stack = branch_arg.try_resolve_stack(ctx)?;

    if let Some(stack) = stack
        && let Some(stack_id) = stack.id
    {
        // Get worktree changes and assignments
        let worktree_changes = but_api::diff::changes_in_worktree(ctx)?;

        let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
        for assignment in worktree_changes.assignments {
            by_file
                .entry(assignment.path_bytes.clone())
                .or_default()
                .push(assignment);
        }

        // Collect files that have hunks assigned to this stack
        // These are the "uncommitted" files for the stack
        let mut uncommitted: Vec<String> = Vec::new();
        for (path, assignments) in &by_file {
            let has_stack_assignment = assignments.iter().any(|a| a.stack_id == Some(stack_id));
            if has_stack_assignment {
                uncommitted.push(path.to_str_lossy().to_string());
            }
        }

        Ok(uncommitted)
    } else {
        // Branch is not in workspace, so no uncommitted files
        Ok(vec![])
    }
}

#[derive(Debug, serde::Serialize)]
struct CommitInfo {
    sha: String,
    short_sha: String,
    message: String,
    author_name: String,
    author_email: String,
    timestamp: i64,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
    files: Vec<FileChange>,
}

#[instrument(skip(commits, git_config))]
fn generate_branch_summary(
    branch_name: &str,
    commits: &[CommitInfo],
    git_config: &gix::config::File<'static>,
) -> anyhow::Result<String> {
    use but_llm::LLMProvider;

    // Get OpenAI provider (tries GitButler proxied, own key, then env var)
    let llm = LLMProvider::from_git_config(git_config).ok_or_else(|| {
        anyhow::anyhow!(
            "No AI credentials found. Configure in GitButler settings or set OPENAI_API_KEY environment variable."
        )
    })?;

    // Build the prompt with commit information
    let mut prompt = format!(
        "Please provide a concise summary (2-3 sentences) of what this branch '{branch_name}' accomplishes based on the following commits:\n\n"
    );

    for commit in commits {
        prompt.push_str(&format!("- {}: {}\n", commit.short_sha, commit.message));
        if !commit.files.is_empty() {
            prompt.push_str("  Files changed:\n");
            for file in &commit.files {
                prompt.push_str(&format!(
                    "    {} ({}, +{}, -{})\n",
                    file.path, file.status, file.insertions, file.deletions
                ));
            }
        }
        prompt.push('\n');
    }

    prompt.push_str(
        "\nProvide a brief, professional summary focusing on the overall purpose and impact of these changes.",
    );
    prompt.push_str(
        "\nHere is a good example:\n\nAdd an --ai flag to the branch show command to allow generating an \nAI-powered summary of a branch's commits. This allows the user to see\nat a glance what the branch is about without reading all commit messages.\n");

    let system_message = "You are a helpful assistant that summarizes Git branch changes.";
    let chat_messages = vec![ChatMessage::User(prompt)];

    let response = llm.response(system_message, chat_messages, "gpt-5-mini")?;

    let summary = response.unwrap_or_default().trim().to_string();

    Ok(summary)
}

fn output_json(
    branch_name: &str,
    commits: &[CommitInfo],
    uncommitted_files: &[String],
    reviews: &[but_forge::ForgeReview],
    ai_summary: Option<&str>,
    merge_check: Option<&MergeCheck>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let reviews_json: Vec<_> = reviews
        .iter()
        .map(|r| {
            serde_json::json!({
                "number": r.number,
                "url": r.html_url,
                "unitSymbol": r.unit_symbol,
                "title": r.title,
                "body": r.body,
                "draft": r.draft,
            })
        })
        .collect();

    let mut output = serde_json::json!({
        "branch": branch_name,
        "commitsAhead": commits.len(),
        "commits": commits,
        "uncommittedFiles": uncommitted_files,
        "reviews": reviews_json,
    });

    // Add AI summary if available
    if let Some(summary) = ai_summary {
        output["aiSummary"] = serde_json::json!(summary);
    }

    // Add merge check if available
    if let Some(check) = merge_check {
        output["mergeCheck"] = serde_json::json!({
            "mergesCleanly": check.merges_cleanly,
            "conflictingFiles": check.conflicting_files,
        });
    }

    out.write_value(output)?;
    Ok(())
}

fn output_human(
    branch_name: &str,
    commits: &[CommitInfo],
    uncommitted_files: &[String],
    reviews: &[but_forge::ForgeReview],
    ai_summary: Option<&str>,
    merge_check: Option<&MergeCheck>,
    out: &mut dyn std::fmt::Write,
) -> anyhow::Result<()> {
    use std::fmt::Write;

    let t = theme::get();

    // Build output as a string first
    let mut buf = String::new();

    // Show branch name with review info if available
    let reviews_str = if !reviews.is_empty() {
        let review_numbers = reviews
            .iter()
            .map(|r| format!("{}{}", r.unit_symbol, r.number))
            .collect::<Vec<String>>()
            .join(", ");
        t.info.paint(format!(" ({review_numbers})"))
    } else {
        t.default.paint("")
    };

    writeln!(
        buf,
        "{} {}{} ({} commits ahead)",
        t.important.paint("Branch:"),
        t.local_branch.paint(branch_name),
        reviews_str,
        t.info.paint(commits.len().to_string()),
    )?;
    writeln!(buf)?;

    if commits.is_empty() {
        writeln!(buf, "No commits ahead of base branch.")?;
    } else {
        for (i, commit) in commits.iter().enumerate() {
            writeln!(
                buf,
                "{} {}",
                t.commit_id.paint(&commit.short_sha),
                commit.message
            )?;
            writeln!(
                buf,
                "    {} by {}",
                t.hint.paint(format_timestamp(commit.timestamp)),
                t.hint.paint(&commit.author_name)
            )?;

            // Show diff stats
            let stats_str = format!(
                "{} file{} changed, {} insertion{}, {} deletion{}",
                commit.files_changed,
                if commit.files_changed == 1 { "" } else { "s" },
                commit.insertions,
                if commit.insertions == 1 { "" } else { "s" },
                commit.deletions,
                if commit.deletions == 1 { "" } else { "s" }
            );
            writeln!(buf, "    {}", t.hint.paint(stats_str))?;

            // Show per-file changes if available
            if !commit.files.is_empty() {
                writeln!(buf)?;
                for file in &commit.files {
                    let status_color = match file.status.as_str() {
                        "added" => t.addition.paint(&file.path),
                        "deleted" => t.deletion.paint(&file.path),
                        "modified" => t.modification.paint(&file.path),
                        _ => t.default.paint(&file.path),
                    };

                    let change_str = if file.status == "added" {
                        format!("    {} (new file, +{})", status_color, file.insertions)
                    } else if file.status == "deleted" {
                        format!("    {} (deleted, -{})", status_color, file.deletions)
                    } else {
                        format!(
                            "    {} (+{}, -{})",
                            status_color, file.insertions, file.deletions
                        )
                    };

                    writeln!(buf, "{change_str}")?;
                }
            }

            // Add blank line between commits (but not after the last one)
            if i < commits.len() - 1 {
                writeln!(buf)?;
            }
        }
    }

    // Display uncommitted files if any exist
    if !uncommitted_files.is_empty() {
        writeln!(buf)?;
        writeln!(buf)?;
        writeln!(buf, "{}", t.important.paint("Uncommitted Files:"))?;
        for file in uncommitted_files {
            writeln!(buf, "  {}", t.attention.paint(file))?;
        }
    }

    // Display review details if available
    if !reviews.is_empty() {
        writeln!(buf)?;
        writeln!(buf)?;
        writeln!(buf, "{}", t.important.paint("Reviews:"))?;
        for review in reviews {
            writeln!(buf)?;
            writeln!(
                buf,
                "  {} {}{}",
                t.hint.paint("PR/MR:"),
                review.unit_symbol,
                review.number
            )?;
            writeln!(buf, "  {} {}", t.hint.paint("Title:"), review.title)?;
            writeln!(
                buf,
                "  {} {}",
                t.hint.paint("URL:"),
                t.link.paint(&review.html_url)
            )?;

            if let Some(body) = &review.body
                && !body.is_empty()
            {
                writeln!(buf, "  {}", t.hint.paint("Description:"))?;
                // Indent each line of the description
                for line in body.lines() {
                    writeln!(buf, "    {line}")?;
                }
            }

            if review.draft {
                writeln!(
                    buf,
                    "  {} {}",
                    t.hint.paint("Status:"),
                    t.attention.paint("Draft")
                )?;
            }
        }
    }

    // Display AI summary if available
    if let Some(summary) = ai_summary {
        writeln!(buf)?;
        writeln!(buf)?;
        writeln!(buf, "{}", t.info.paint("AI Summary:"))?;
        writeln!(buf, "{summary}")?;
        writeln!(buf)?;
    }

    // Display merge check if available
    if let Some(check) = merge_check {
        writeln!(buf)?;
        writeln!(buf)?;
        if check.merges_cleanly {
            writeln!(
                buf,
                "{} {}",
                t.important.paint("Merge Check:"),
                t.success.paint("Merges cleanly into upstream")
            )?;
        } else {
            writeln!(
                buf,
                "{} {}",
                t.important.paint("Merge Check:"),
                t.error.paint("Conflicts detected"),
            )?;
            writeln!(buf)?;
            writeln!(
                buf,
                "  {} file{} conflict{}:",
                check.conflicting_files.len(),
                if check.conflicting_files.len() == 1 {
                    ""
                } else {
                    "s"
                },
                if check.conflicting_files.len() == 1 {
                    "s"
                } else {
                    ""
                }
            )?;
            writeln!(buf)?;

            for file in &check.conflicting_files {
                writeln!(buf, "  {}", t.attention.paint(&file.path))?;

                // Show branch commits that modified this file
                if !file.branch_commits.is_empty() {
                    writeln!(buf, "    Modified by this branch:")?;
                    for commit in &file.branch_commits {
                        writeln!(
                            buf,
                            "      {} {}",
                            t.commit_id.paint(&commit.short_sha),
                            commit.message
                        )?;
                        writeln!(
                            buf,
                            "        {} by {}",
                            t.hint.paint(format_timestamp(commit.timestamp)),
                            t.hint.paint(&commit.author_name)
                        )?;
                    }
                    writeln!(buf)?;
                }

                // Show upstream commits that modified this file
                if !file.upstream_commits.is_empty() {
                    writeln!(buf, "    Modified by upstream:")?;
                    for commit in &file.upstream_commits {
                        writeln!(
                            buf,
                            "      {} {}",
                            t.commit_id.paint(&commit.short_sha),
                            commit.message
                        )?;
                        writeln!(
                            buf,
                            "        {} by {}",
                            t.hint.paint(format_timestamp(commit.timestamp)),
                            t.hint.paint(&commit.author_name)
                        )?;
                    }
                    writeln!(buf)?;
                }
            }
        }
    }

    out.write_str(&buf)?;
    Ok(())
}

fn format_timestamp(timestamp: i64) -> String {
    use chrono::{DateTime, Local, Utc};

    let datetime = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_else(Utc::now)
        .with_timezone(&Local);

    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

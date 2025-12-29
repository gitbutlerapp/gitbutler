use super::list::load_id_map;
use crate::utils::OutputChannel;
use but_ctx::Context;
use but_oxidize::OidExt;
use colored::Colorize;
use gitbutler_project::Project;
use tracing::instrument;

#[allow(clippy::too_many_arguments)]
pub async fn show(
    ctx: &mut Context,
    branch_id: &str,
    out: &mut OutputChannel,
    review: bool,
    show_files: bool,
    generate_ai_summary: bool,
    check_merge: bool,
) -> anyhow::Result<()> {
    let id_map = load_id_map(&ctx.legacy_project)?;

    // Find the branch name from the ID
    let branch_name = if branch_id.len() == 2 {
        // Lookup by CLI ID
        id_map
            .iter()
            .find(|(_, id)| id.as_str() == branch_id)
            .map(|(name, _)| name.clone())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Branch ID '{}' not found. Run 'but branch' to see available IDs.",
                    branch_id
                )
            })?
    } else {
        // Assume it's a branch name
        branch_id.to_string()
    };

    // Get the list of commits ahead of base for this branch
    let commits = get_commits_ahead(&ctx.legacy_project, &branch_name, show_files)?;

    // Get unassigned files for this branch
    let unassigned_files = get_unassigned_files(ctx, &branch_name)?;

    // Get review information if requested
    let reviews = if review {
        crate::command::legacy::forge::review::get_review_map(&ctx.legacy_project)
            .await?
            .get(&branch_name)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Generate AI summary if requested
    let ai_summary = if generate_ai_summary {
        Some(generate_branch_summary(&branch_name, &commits).await?)
    } else {
        None
    };

    // Check merge conflicts if requested
    let merge_check = if check_merge {
        Some(check_merge_conflicts(&ctx.legacy_project, &branch_name)?)
    } else {
        None
    };

    if let Some(out) = out.for_json() {
        output_json(
            &branch_name,
            &commits,
            &unassigned_files,
            &reviews,
            ai_summary.as_deref(),
            merge_check.as_ref(),
            out,
        )?;
    } else if let Some(out) = out.for_human() {
        output_human(
            &branch_name,
            &commits,
            &unassigned_files,
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

fn check_merge_conflicts(
    project: &Project,
    branch_name: &str,
) -> Result<MergeCheck, anyhow::Error> {
    use but_core::RepositoryExt;

    let ctx = Context::new_from_legacy_project(project.clone())?;
    let git2_repo = &*ctx.git2_repo.get()?;
    let repo = ctx.clone_repo_for_merging_non_persisting()?;

    // Get the target (remote tracking branch like origin/master)
    let stack = gitbutler_stack::VirtualBranchesHandle::new(project.gb_dir());
    let target = stack.get_default_target()?;

    // Try to find the remote tracking branch (e.g., refs/remotes/origin/master)
    let target_ref_name = format!(
        "refs/remotes/{}/{}",
        target.branch.remote(),
        target.branch.branch()
    );
    let target_commit = match repo.find_reference(&target_ref_name) {
        Ok(reference) => {
            let target_oid = reference.id();
            git2_repo.find_commit(but_oxidize::gix_to_git2_oid(target_oid))?
        }
        Err(_) => {
            // Fallback to the stored SHA if remote branch doesn't exist
            git2_repo.find_commit(target.sha)?
        }
    };

    // Find the branch by name
    let branches = but_api::legacy::virtual_branches::list_branches(project.id, None)?;
    let branch = branches
        .iter()
        .find(|b| b.name.to_string() == branch_name)
        .ok_or_else(|| anyhow::anyhow!("Branch '{}' not found", branch_name))?;

    let branch_commit = git2_repo.find_commit(branch.head)?;

    // Find merge base
    let merge_base = git2_repo.merge_base(target_commit.id(), branch_commit.id())?;
    let merge_base_commit = git2_repo.find_commit(merge_base)?;

    // Check if branch merges cleanly into target
    let merges_cleanly = repo.merges_cleanly(
        merge_base_commit.tree_id().to_gix(),
        target_commit.tree_id().to_gix(),
        branch_commit.tree_id().to_gix(),
    )?;

    let mut conflicting_files = Vec::new();

    // If there are conflicts, identify which files conflict and which commits modified them
    if !merges_cleanly {
        // Get the list of conflicting files from the merge
        let conflict_paths = get_merge_conflict_paths(
            &repo,
            merge_base_commit.tree_id(),
            target_commit.tree_id(),
            branch_commit.tree_id(),
        )?;

        // For each conflicting file, find which commits on both sides modified it
        for path in conflict_paths {
            let branch_commits =
                find_commits_modifying_file(git2_repo, &path, merge_base, branch_commit.id())?;

            let upstream_commits =
                find_commits_modifying_file(git2_repo, &path, merge_base, target_commit.id())?;

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
    base_tree: git2::Oid,
    ours_tree: git2::Oid,
    theirs_tree: git2::Oid,
) -> Result<Vec<String>, anyhow::Error> {
    use but_core::RepositoryExt;

    let (options, conflict_kind) = gix_repo.merge_options_fail_fast()?;
    let merge_result = gix_repo.merge_trees(
        but_oxidize::git2_to_gix_object_id(base_tree),
        but_oxidize::git2_to_gix_object_id(ours_tree),
        but_oxidize::git2_to_gix_object_id(theirs_tree),
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

fn find_commits_modifying_file(
    repo: &git2::Repository,
    path: &str,
    from_commit: git2::Oid,
    to_commit: git2::Oid,
) -> Result<Vec<CommitRef>, anyhow::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push(to_commit)?;
    revwalk.hide(from_commit)?;

    let mut commits = Vec::new();

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        // Check if this commit modified the file
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

        // Check if any delta in the diff touches this file
        let mut modified_file = false;
        for delta in diff.deltas() {
            let delta_path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .and_then(|p| p.to_str());

            if let Some(delta_path) = delta_path
                && delta_path == path
            {
                modified_file = true;
                break;
            }
        }

        if modified_file {
            let author = commit.author();
            commits.push(CommitRef {
                sha: oid.to_string(),
                short_sha: oid.to_string()[..7].to_string(),
                message: commit
                    .message()
                    .unwrap_or("(no message)")
                    .lines()
                    .next()
                    .unwrap_or("(no message)")
                    .to_string(),
                author_name: author.name().unwrap_or("Unknown").to_string(),
                timestamp: commit.time().seconds(),
            });
        }
    }

    Ok(commits)
}

fn get_commits_ahead(
    project: &Project,
    branch_name: &str,
    show_files: bool,
) -> Result<Vec<CommitInfo>, anyhow::Error> {
    use but_ctx::Context;

    let ctx = Context::new_from_legacy_project(project.clone())?;
    let repo = &*ctx.git2_repo.get()?;

    // Get the target (remote tracking branch like origin/master)
    let stack = gitbutler_stack::VirtualBranchesHandle::new(project.gb_dir());
    let target = stack.get_default_target()?;

    // Try to find the remote tracking branch (e.g., refs/remotes/origin/master)
    let target_ref_name = format!(
        "refs/remotes/{}/{}",
        target.branch.remote(),
        target.branch.branch()
    );
    let gix_repo = ctx.repo.get()?;
    let target_commit = match gix_repo.find_reference(&target_ref_name) {
        Ok(reference) => {
            let target_oid = reference.id();
            repo.find_commit(but_oxidize::gix_to_git2_oid(target_oid))?
        }
        Err(_) => {
            // Fallback to the stored SHA if remote branch doesn't exist
            repo.find_commit(target.sha)?
        }
    };

    // Find the branch by name
    let branches = but_api::legacy::virtual_branches::list_branches(project.id, None)?;
    let branch = branches
        .iter()
        .find(|b| b.name.to_string() == branch_name)
        .ok_or_else(|| anyhow::anyhow!("Branch '{}' not found", branch_name))?;

    let branch_commit = repo.find_commit(branch.head)?;

    // Find merge base
    let merge_base = repo.merge_base(target_commit.id(), branch_commit.id())?;

    // Walk from branch head to merge base, collecting commits
    let mut revwalk = repo.revwalk()?;
    revwalk.push(branch_commit.id())?;
    revwalk.hide(merge_base)?;

    let mut commits = Vec::new();
    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let author = commit.author();

        // Calculate diff stats
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };
        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        let stats = diff.stats()?;

        // Collect per-file stats if requested
        let files = if show_files {
            let mut files_with_stats = Vec::new();

            // Iterate through deltas and collect stats per file
            for delta_idx in 0..diff.deltas().len() {
                let delta = diff.get_delta(delta_idx).unwrap();

                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .and_then(|p| p.to_str())
                    .unwrap_or("(unknown)")
                    .to_string();

                let status = match delta.status() {
                    git2::Delta::Added => "added",
                    git2::Delta::Deleted => "deleted",
                    git2::Delta::Modified => "modified",
                    git2::Delta::Renamed => "renamed",
                    git2::Delta::Copied => "copied",
                    git2::Delta::Typechange => "typechange",
                    _ => "unknown",
                };

                // Get patch for this specific file to count lines
                let patch = git2::Patch::from_diff(&diff, delta_idx)?;
                let mut insertions = 0;
                let mut deletions = 0;

                if let Some(patch) = patch {
                    for hunk_idx in 0..patch.num_hunks() {
                        let hunk_lines = patch.num_lines_in_hunk(hunk_idx)?;
                        for line_idx in 0..hunk_lines {
                            let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                            match line.origin() {
                                '+' => insertions += 1,
                                '-' => deletions += 1,
                                _ => {}
                            }
                        }
                    }
                }

                files_with_stats.push(FileChange {
                    path,
                    status: status.to_string(),
                    insertions,
                    deletions,
                });
            }

            files_with_stats
        } else {
            Vec::new()
        };

        commits.push(CommitInfo {
            sha: oid.to_string(),
            short_sha: oid.to_string()[..7].to_string(),
            message: commit
                .message()
                .unwrap_or("(no message)")
                .lines()
                .next()
                .unwrap_or("(no message)")
                .to_string(),
            author_name: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            files_changed: stats.files_changed(),
            insertions: stats.insertions(),
            deletions: stats.deletions(),
            files,
        });
    }

    Ok(commits)
}

fn get_unassigned_files(
    ctx: &mut Context,
    branch_name: &str,
) -> Result<Vec<String>, anyhow::Error> {
    use std::collections::BTreeMap;

    use bstr::{BString, ByteSlice};
    use but_hunk_assignment::HunkAssignment;

    // Find the stack that contains this branch
    let stacks = but_api::legacy::workspace::stacks(
        ctx.legacy_project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Find the stack ID for this branch
    let stack_id = stacks
        .iter()
        .find(|stack| stack.heads.iter().any(|head| head.name == branch_name))
        .and_then(|stack| stack.id);

    if let Some(stack_id) = stack_id {
        // Get worktree changes and assignments
        let worktree_changes = but_api::legacy::diff::changes_in_worktree(ctx)?;

        let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
        for assignment in worktree_changes.assignments {
            by_file
                .entry(assignment.path_bytes.clone())
                .or_default()
                .push(assignment);
        }

        // Collect files that have hunks assigned to this stack
        // These are the "uncommitted" files for the stack
        let mut unassigned: Vec<String> = Vec::new();
        for (path, assignments) in &by_file {
            let has_stack_assignment = assignments.iter().any(|a| a.stack_id == Some(stack_id));
            if has_stack_assignment {
                unassigned.push(path.to_str_lossy().to_string());
            }
        }

        Ok(unassigned)
    } else {
        // Branch is not in workspace, so no unassigned files
        Ok(vec![])
    }
}

#[derive(Debug, serde::Serialize)]
struct FileChange {
    path: String,
    status: String, // "modified", "added", "deleted"
    insertions: usize,
    deletions: usize,
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

#[instrument(skip(commits))]
async fn generate_branch_summary(
    branch_name: &str,
    commits: &[CommitInfo],
) -> anyhow::Result<String> {
    use async_openai::types::chat::{
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    };
    use but_action::OpenAiProvider;

    // Get OpenAI provider (tries GitButler proxied, own key, then env var)
    let provider = OpenAiProvider::with(None).ok_or_else(|| {
        anyhow::anyhow!(
            "No AI credentials found. Configure in GitButler settings or set OPENAI_API_KEY environment variable."
        )
    })?;

    // Build the prompt with commit information
    let mut prompt = format!(
        "Please provide a concise summary (2-3 sentences) of what this branch '{}' accomplishes based on the following commits:\n\n",
        branch_name
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

    // Create OpenAI client and make request
    let client = provider.client()?;

    let messages = vec![
        ChatCompletionRequestSystemMessage::from(
            "You are a helpful assistant that summarizes Git branch changes.",
        )
        .into(),
        ChatCompletionRequestUserMessageArgs::default()
            .content(prompt)
            .build()?
            .into(),
    ];

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-5-mini")
        .messages(messages)
        .max_completion_tokens(500u32)
        .build()?;

    let response = client.chat().create(request).await?;

    // Extract the summary from the response
    let summary = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("No response content from AI"))?;

    Ok(summary)
}

fn output_json(
    branch_name: &str,
    commits: &[CommitInfo],
    unassigned_files: &[String],
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
        "unassignedFiles": unassigned_files,
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
    unassigned_files: &[String],
    reviews: &[but_forge::ForgeReview],
    ai_summary: Option<&str>,
    merge_check: Option<&MergeCheck>,
    out: &mut dyn std::fmt::Write,
) -> anyhow::Result<()> {
    use std::fmt::Write;
    // Build output as a string first
    let mut buf = String::new();

    // Show branch name with review info if available
    let reviews_str = if !reviews.is_empty() {
        let review_numbers = reviews
            .iter()
            .map(|r| format!("{}{}", r.unit_symbol, r.number))
            .collect::<Vec<String>>()
            .join(", ");
        format!(" ({})", review_numbers).blue().to_string()
    } else {
        String::new()
    };

    writeln!(
        buf,
        "{} {}{} ({} commits ahead)",
        "Branch:".bold(),
        branch_name.green(),
        reviews_str,
        commits.len().to_string().cyan()
    )?;
    writeln!(buf)?;

    if commits.is_empty() {
        writeln!(buf, "No commits ahead of base branch.")?;
    } else {
        for (i, commit) in commits.iter().enumerate() {
            writeln!(buf, "{} {}", commit.short_sha.yellow(), commit.message)?;
            writeln!(
                buf,
                "    {} by {}",
                format_timestamp(commit.timestamp).dimmed(),
                commit.author_name.dimmed()
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
            writeln!(buf, "    {}", stats_str.dimmed())?;

            // Show per-file changes if available
            if !commit.files.is_empty() {
                writeln!(buf)?;
                for file in &commit.files {
                    let status_color = match file.status.as_str() {
                        "added" => file.path.green(),
                        "deleted" => file.path.red(),
                        "modified" => file.path.yellow(),
                        _ => file.path.normal(),
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

                    writeln!(buf, "{}", change_str)?;
                }
            }

            // Add blank line between commits (but not after the last one)
            if i < commits.len() - 1 {
                writeln!(buf)?;
            }
        }
    }

    // Display unassigned files if any exist
    if !unassigned_files.is_empty() {
        writeln!(buf)?;
        writeln!(buf)?;
        writeln!(buf, "{}", "Unassigned Files:".bold())?;
        for file in unassigned_files {
            writeln!(buf, "  {}", file.yellow())?;
        }
    }

    // Display review details if available
    if !reviews.is_empty() {
        writeln!(buf)?;
        writeln!(buf)?;
        writeln!(buf, "{}", "Reviews:".bold())?;
        for review in reviews {
            writeln!(buf)?;
            writeln!(
                buf,
                "  {} {}{}",
                "PR/MR:".dimmed(),
                review.unit_symbol,
                review.number
            )?;
            writeln!(buf, "  {} {}", "Title:".dimmed(), review.title)?;
            writeln!(buf, "  {} {}", "URL:".dimmed(), review.html_url.cyan())?;

            if let Some(body) = &review.body
                && !body.is_empty()
            {
                writeln!(buf, "  {}", "Description:".dimmed())?;
                // Indent each line of the description
                for line in body.lines() {
                    writeln!(buf, "    {}", line)?;
                }
            }

            if review.draft {
                writeln!(buf, "  {} {}", "Status:".dimmed(), "Draft".yellow())?;
            }
        }
    }

    // Display AI summary if available
    if let Some(summary) = ai_summary {
        writeln!(buf)?;
        writeln!(buf)?;
        writeln!(buf, "{}", "AI Summary:".bold().cyan())?;
        writeln!(buf, "{}", summary)?;
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
                "Merge Check:".bold(),
                "Merges cleanly into upstream".green()
            )?;
        } else {
            writeln!(
                buf,
                "{} {}",
                "Merge Check:".bold(),
                "Conflicts detected".red().bold()
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
                writeln!(buf, "  {}", file.path.yellow().bold())?;

                // Show branch commits that modified this file
                if !file.branch_commits.is_empty() {
                    writeln!(buf, "    Modified by this branch:")?;
                    for commit in &file.branch_commits {
                        writeln!(buf, "      {} {}", commit.short_sha.cyan(), commit.message)?;
                        writeln!(
                            buf,
                            "        {} by {}",
                            format_timestamp(commit.timestamp).dimmed(),
                            commit.author_name.dimmed()
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
                            commit.short_sha.magenta(),
                            commit.message
                        )?;
                        writeln!(
                            buf,
                            "        {} by {}",
                            format_timestamp(commit.timestamp).dimmed(),
                            commit.author_name.dimmed()
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

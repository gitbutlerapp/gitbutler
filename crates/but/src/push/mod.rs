use but_core::RepositoryExt;
use but_settings::AppSettings;
use but_workspace::StackId;
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_repo::RepoCommands;

/// Generate branch URL for GitHub remote
fn generate_github_branch_url(remote_url: &str, branch_name: &str) -> Option<String> {
    if let Some(repo_path) = extract_github_repo_path(remote_url) {
        Some(format!(
            "https://github.com/{}/tree/{}",
            repo_path, branch_name
        ))
    } else {
        None
    }
}

/// Generate branch URL for GitLab remote
fn generate_gitlab_branch_url(remote_url: &str, branch_name: &str) -> Option<String> {
    if let Some((host, repo_path)) = extract_gitlab_info(remote_url) {
        Some(format!(
            "https://{}/{}/-/tree/{}",
            host, repo_path, branch_name
        ))
    } else {
        None
    }
}

/// Generate change URL for Gerrit remote
fn generate_gerrit_change_url(remote_url: &str) -> Option<String> {
    if let Some(base_url) = extract_gerrit_base_url(remote_url) {
        Some(format!("{}/dashboard/self", base_url))
    } else {
        None
    }
}

/// Extract GitHub repository path from various URL formats
fn extract_github_repo_path(url: &str) -> Option<String> {
    // Handle GitHub URLs: git@github.com:user/repo.git or https://github.com/user/repo.git
    if !url.contains("github.com") {
        return None;
    }

    // Find the part after github.com
    let after_github = if let Some(pos) = url.find("github.com:") {
        &url[pos + 11..] // Skip "github.com:"
    } else if let Some(pos) = url.find("github.com/") {
        &url[pos + 11..] // Skip "github.com/"
    } else {
        return None;
    };

    // Extract user/repo, removing .git suffix if present
    let repo_path = after_github.trim_end_matches(".git");
    let parts: Vec<&str> = repo_path.splitn(3, '/').collect();
    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// Extract GitLab host and repository path from various URL formats
fn extract_gitlab_info(url: &str) -> Option<(String, String)> {
    // Handle GitLab URLs: git@gitlab.example.com:user/repo.git or https://gitlab.example.com/user/repo.git
    if !url.contains("gitlab") {
        return None;
    }

    // Extract host and path
    if let Some(colon_pos) = url.rfind(':') {
        if let Some(at_pos) = url[..colon_pos].rfind('@') {
            // SSH format: git@gitlab.example.com:user/repo.git
            let host = &url[at_pos + 1..colon_pos];
            let path_part = &url[colon_pos + 1..];
            let repo_path = path_part.trim_end_matches(".git");
            let parts: Vec<&str> = repo_path.splitn(3, '/').collect();
            if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                return Some((host.to_string(), format!("{}/{}", parts[0], parts[1])));
            }
        }
    } else if url.starts_with("https://") {
        // HTTPS format: https://gitlab.example.com/user/repo.git
        let after_https = &url[8..]; // Skip "https://"
        if let Some(slash_pos) = after_https.find('/') {
            let host = &after_https[..slash_pos];
            let path_part = &after_https[slash_pos + 1..];
            let repo_path = path_part.trim_end_matches(".git");
            let parts: Vec<&str> = repo_path.splitn(3, '/').collect();
            if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                return Some((host.to_string(), format!("{}/{}", parts[0], parts[1])));
            }
        }
    }
    None
}

/// Extract Gerrit base URL from remote URL
fn extract_gerrit_base_url(url: &str) -> Option<String> {
    // Handle Gerrit URLs: ssh://user@gerrit.example.com:29418/project.git or https://gerrit.example.com/a/project.git
    if url.contains("gerrit") || url.contains(":29418") {
        if url.starts_with("ssh://") {
            // SSH format: ssh://user@gerrit.example.com:29418/project.git
            let after_ssh = &url[6..]; // Skip "ssh://"
            if let Some(at_pos) = after_ssh.find('@') {
                let after_at = &after_ssh[at_pos + 1..];
                if let Some(colon_pos) = after_at.find(':') {
                    let host = &after_at[..colon_pos];
                    return Some(format!("https://{}", host));
                }
            }
        } else if url.starts_with("https://") {
            // HTTPS format: https://gerrit.example.com/a/project.git
            let after_https = &url[8..]; // Skip "https://"
            if let Some(slash_pos) = after_https.find('/') {
                let host = &after_https[..slash_pos];
                return Some(format!("https://{}", host));
            }
        }
    }
    None
}

/// Generate appropriate URL for the remote and branch
fn generate_branch_url(remote_url: &str, branch_name: &str, is_gerrit: bool) -> Option<String> {
    if is_gerrit {
        generate_gerrit_change_url(remote_url)
    } else if remote_url.contains("github.com") {
        generate_github_branch_url(remote_url, branch_name)
    } else if remote_url.contains("gitlab") {
        generate_gitlab_branch_url(remote_url, branch_name)
    } else {
        None
    }
}

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// Branch name or CLI ID to push
    pub branch_id: String,
    /// Force push even if it's not fast-forward
    #[clap(long, short = 'f', default_value_t = true)]
    pub with_force: bool,
    /// Skip force push protection checks
    #[clap(long, short = 's')]
    pub skip_force_push_protection: bool,
    /// Run pre-push hooks
    #[clap(long, short = 'r', default_value_t = true)]
    pub run_hooks: bool,
    /// Mark change as work-in-progress (Gerrit)
    #[clap(long, short = 'w', group = "gerrit", hide = true)]
    pub wip: bool,
    /// Mark change as ready for review (Gerrit)  
    #[clap(long, short = 'y', group = "gerrit", hide = true)]
    pub ready: bool,
    /// Add hashtag to change (Gerrit)
    #[clap(long, short = 'a', group = "gerrit", value_name = "TAG", hide = true)]
    pub hashtag: Option<String>,
    /// Add custom topic to change (Gerrit)
    #[clap(long, short = 't', group = "gerrit", value_name = "TOPIC", hide = true)]
    pub topic: Option<String>,
    /// Use branch name as topic (Gerrit)
    #[clap(
        long = "tb",
        alias = "topic-from-branch",
        group = "gerrit",
        hide = true
    )]
    pub topic_from_branch: bool,
    /// Use branch name as hashtag (Gerrit)
    #[clap(
        long = "ab",
        alias = "hashtag-from-branch",
        group = "gerrit",
        hide = true
    )]
    pub hashtag_from_branch: bool,
}

fn get_gerrit_flag(
    args: &Args,
    branch_name: &str,
    gerrit_mode: bool,
) -> anyhow::Result<Option<but_gerrit::PushFlag>> {
    let has_gerrit_flag = args.wip
        || args.ready
        || args.hashtag.is_some()
        || args.topic.is_some()
        || args.topic_from_branch
        || args.hashtag_from_branch;

    if has_gerrit_flag && !gerrit_mode {
        return Err(anyhow::anyhow!(
            "Gerrit push flags (--wip, --ready, --hashtag, --topic, --topic-from-branch, --hashtag-from-branch) can only be used when gerrit_mode is enabled for this repository"
        ));
    }

    if args.wip {
        Ok(Some(but_gerrit::PushFlag::Wip))
    } else if args.ready {
        Ok(Some(but_gerrit::PushFlag::Ready))
    } else if let Some(hashtag) = &args.hashtag {
        if hashtag.trim().is_empty() {
            return Err(anyhow::anyhow!("Hashtag cannot be empty"));
        }
        Ok(Some(but_gerrit::PushFlag::Hashtag(hashtag.clone())))
    } else if let Some(topic) = &args.topic {
        if topic.trim().is_empty() {
            return Err(anyhow::anyhow!("Topic cannot be empty"));
        }
        Ok(Some(but_gerrit::PushFlag::Topic(topic.clone())))
    } else if args.topic_from_branch {
        Ok(Some(but_gerrit::PushFlag::Topic(branch_name.to_string())))
    } else if args.hashtag_from_branch {
        Ok(Some(but_gerrit::PushFlag::Hashtag(branch_name.to_string())))
    } else {
        Ok(None)
    }
}

pub fn handle(args: &Args, project: &Project, _json: bool) -> anyhow::Result<()> {
    let mut ctx = CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    // Check gerrit mode early
    let gerrit_mode = ctx
        .gix_repo()?
        .git_settings()?
        .gitbutler_gerrit_mode
        .unwrap_or(false);

    // Resolve branch_id to actual branch name
    let branch_name = resolve_branch_name(&mut ctx, &args.branch_id)?;

    // Find stack_id from branch name
    let stack_id = find_stack_id_by_branch_name(project, &branch_name)?;

    // Convert CLI args to gerrit flag with validation
    let gerrit_flag = get_gerrit_flag(args, &branch_name, gerrit_mode)?;

    println!("Pushing branch '{}'...", branch_name);

    // Call push_stack
    let result: PushResult = but_api::stack::push_stack(
        project.id,
        stack_id,
        args.with_force,
        args.skip_force_push_protection,
        branch_name.clone(),
        args.run_hooks,
        gerrit_flag,
    )?;

    dbg!(&result);

    println!("Push completed successfully");
    println!("Pushed to remote: {}", result.remote);
    if !gerrit_mode && !result.branch_to_remote.is_empty() {
        for (branch, remote_ref) in &result.branch_to_remote {
            println!("  {} -> {}", branch, remote_ref);
        }
    }

    // Get remote URL and generate branch URL if possible
    if let Ok(remotes) = project.remotes() {
        if let Some(remote) = remotes.iter().find(|r| r.name.as_ref() == Some(&result.remote)) {
            if let Some(remote_url) = &remote.url {
                if let Some(branch_url) = generate_branch_url(remote_url, &branch_name, gerrit_mode) {
                    if gerrit_mode {
                        println!("View changes: {}", branch_url);
                    } else {
                        println!("Branch URL: {}", branch_url);
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn print_help() {
    // Print basic push help
    println!("Push a branch/stack to remote");
    println!();
    println!("Usage: but push [OPTIONS] <BRANCH_ID>");
    println!();
    println!("Arguments:");
    println!("  <BRANCH_ID>  Branch name or CLI ID to push");
    println!();
    println!("Options:");
    println!("  -f, --with-force                  Force push even if it's not fast-forward");
    println!("  -s, --skip-force-push-protection  Skip force push protection checks");
    println!("  -r, --run-hooks                   Run pre-push hooks");

    // Check if gerrit mode is enabled and show gerrit options
    if is_gerrit_enabled_for_help() {
        println!();
        println!("Gerrit Options:");
        println!("  -w, --wip                         Mark change as work-in-progress (Gerrit)");
        println!("  -y, --ready                       Mark change as ready for review (Gerrit)");
        println!("  -a, --hashtag <TAG>               Add hashtag to change (Gerrit)");
        println!("      --ab, --hashtag-from-branch   Use branch name as hashtag (Gerrit)");
        println!("  -t, --topic <TOPIC>               Add custom topic to change (Gerrit)");
        println!("      --tb, --topic-from-branch     Use branch name as topic (Gerrit)");
        println!();
        println!("Note: Only one Gerrit option can be used at a time.");
    }

    println!("  -h, --help                        Print help");
}

fn is_gerrit_enabled_for_help() -> bool {
    // Parse the -C flag from command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut current_dir = std::path::Path::new(".");

    // Look for -C flag
    for (i, arg) in args.iter().enumerate() {
        if arg == "-C" && i + 1 < args.len() {
            current_dir = std::path::Path::new(&args[i + 1]);
            break;
        }
    }

    // Try to check if we're in a gerrit-enabled repository for help display
    if let Ok(repo) = gix::discover(current_dir)
        && let Ok(settings) = repo.git_settings()
    {
        return settings.gitbutler_gerrit_mode.unwrap_or(false);
    }
    false
}

fn resolve_branch_name(ctx: &mut CommandContext, branch_id: &str) -> anyhow::Result<String> {
    // Try to resolve as CliId first
    let cli_ids = crate::id::CliId::from_str(ctx, branch_id)?;

    if cli_ids.is_empty() {
        // If no CliId matches, treat as literal branch name but validate it exists
        let available_branches = get_available_branch_names(ctx)?;
        if !available_branches.contains(&branch_id.to_string()) {
            return Err(anyhow::anyhow!(
                "Branch '{}' not found. Available branches:\n{}",
                branch_id,
                format_branch_suggestions(&available_branches)
            ));
        }
        return Ok(branch_id.to_string());
    }

    if cli_ids.len() > 1 {
        let branch_names: Vec<String> = cli_ids
            .iter()
            .filter_map(|id| match id {
                crate::id::CliId::Branch { name } => Some(name.clone()),
                _ => None,
            })
            .collect();

        if !branch_names.is_empty() {
            return Err(anyhow::anyhow!(
                "Ambiguous branch identifier '{}'. Did you mean one of:\n{}",
                branch_id,
                format_branch_suggestions(&branch_names)
            ));
        } else {
            return Err(anyhow::anyhow!(
                "Identifier '{}' matches multiple non-branch items. Please use a branch name or branch CLI ID.",
                branch_id
            ));
        }
    }

    match &cli_ids[0] {
        crate::id::CliId::Branch { name } => Ok(name.clone()),
        _ => Err(anyhow::anyhow!(
            "Expected branch identifier, got {}. Please use a branch name or branch CLI ID.",
            cli_ids[0].kind()
        )),
    }
}

fn get_available_branch_names(ctx: &CommandContext) -> anyhow::Result<Vec<String>> {
    let stacks = crate::log::stacks(ctx)?;
    let mut branch_names = Vec::new();

    for stack in stacks {
        for head in &stack.heads {
            branch_names.push(head.name.to_string());
        }
    }

    branch_names.sort();
    branch_names.dedup();
    Ok(branch_names)
}

fn format_branch_suggestions(branches: &[String]) -> String {
    if branches.is_empty() {
        return "  (no branches available)".to_string();
    }

    branches
        .iter()
        .map(|name| format!("  - {}", name))
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_stack_id_by_branch_name(project: &Project, branch_name: &str) -> anyhow::Result<StackId> {
    let stacks =
        but_api::workspace::stacks(project.id, Some(but_workspace::StacksFilter::InWorkspace))?;

    // Find which stack this branch belongs to
    for stack_entry in &stacks {
        if stack_entry.heads.iter().any(|b| b.name == branch_name) && stack_entry.id.is_some() {
            return Ok(stack_entry.id.unwrap());
        }
    }

    // This should rarely happen now since we validate branch existence earlier,
    // but provide a helpful error just in case
    let available_branches: Vec<String> = stacks
        .iter()
        .flat_map(|s| s.heads.iter().map(|h| h.name.to_string()))
        .collect();

    Err(anyhow::anyhow!(
        "Branch '{}' not found in any stack. Available branches:\n{}",
        branch_name,
        format_branch_suggestions(&available_branches)
    ))
}

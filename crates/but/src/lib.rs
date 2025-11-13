use std::{ffi::OsString, io::Write, path::Path};

use anyhow::{Context, Result};

mod args;
use args::{Args, CommandName, Subcommands, actions, claude, cursor};
use but_claude::hooks::OutputAsJson;
use but_settings::AppSettings;
use colored::Colorize;
use gix::date::time::CustomFormat;
use metrics::{Event, Metrics, Props, metrics_if_configured};

mod absorb;
mod base;
mod branch;
mod command;
mod commit;
mod completions;
mod describe;
mod editor;
mod forge;
mod gui;
mod id;
mod init;
mod log;
mod mark;
mod mcp;
mod mcp_internal;
mod metrics;
mod oplog;
mod push;
mod rub;
mod status;
mod ui;
mod worktree;

const CLI_DATE: CustomFormat = gix::date::time::format::ISO8601;
/// A utility to clearly mark the old project type to get away from.
type LegacyProject = gitbutler_project::Project;

/// Handle `args` which must be what's passed by `std::env::args_os()`.
pub async fn handle_args(args: impl Iterator<Item = OsString>) -> Result<()> {
    let args: Vec<_> = args.collect();

    // Check if help is requested with no subcommand
    if args.len() == 1 || args.iter().any(|arg| arg == "--help" || arg == "-h") && args.len() == 2 {
        print_grouped_help().ok();
        return Ok(());
    }

    // The but push --help output is different if gerrit mode is enabled, hence the special handling
    let args_vec: Vec<String> = std::env::args().collect();
    if args_vec.iter().any(|arg| arg == "push")
        && args_vec.iter().any(|arg| arg == "--help" || arg == "-h")
    {
        push::print_help().ok();
        return Ok(());
    }

    let mut args: Args = clap::Parser::parse_from(args);
    let app_settings = AppSettings::load_from_default_path_creating()?;

    if args.trace > 0 {
        trace::init(args.trace)?;
    }

    let namespace = option_env!("IDENTIFIER").unwrap_or("com.gitbutler.app");
    but_secret::secret::set_application_namespace(namespace);
    let start = std::time::Instant::now();

    // If no subcommand is provided but we have source and target, default to rub
    match args.cmd.take() {
        None if args.source_or_path.is_some() && args.target.is_some() => {
            // Default to rub when two arguments are provided without a subcommand
            let source = args
                .source_or_path
                .as_ref()
                .expect("source is checked to be Some in match guard");
            let target = args
                .target
                .as_ref()
                .expect("target is checked to be Some in match guard");
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result =
                rub::handle(&project, args.json, source, target).context("Rubbed the wrong way.");
            if let Err(e) = &result {
                let mut stderr = std::io::stderr();
                writeln!(stderr, "{} {}", e, e.root_cause()).ok();
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            result
        }
        None if args.source_or_path.is_some() && args.target.is_none() => {
            // If only one arguments is provided without a subcommand, check if this is a valid path.
            let maybe_path = args
                .source_or_path
                .as_ref()
                .expect("path is checked to be Some in match guard");
            let path = std::path::Path::new(args.current_dir.as_path()).join(maybe_path);
            gui::open(&path)?;
            Ok(())
        }
        None => {
            // No subcommand and no source/target means help was requested
            print_grouped_help().ok();
            Ok(())
        }
        Some(cmd) => match_subcommand(cmd, args, app_settings, start).await,
    }
}

async fn match_subcommand(
    cmd: Subcommands,
    args: Args,
    app_settings: AppSettings,
    start: std::time::Instant,
) -> Result<()> {
    match cmd {
        Subcommands::Mcp { internal } => {
            if internal {
                mcp_internal::start(app_settings).await
            } else {
                mcp::start(app_settings).await
            }
        }
        Subcommands::Actions(actions::Platform { cmd }) => match cmd {
            Some(actions::Subcommands::HandleChanges {
                description,
                handler,
            }) => {
                let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
                command::handle_changes(&project, args.json, handler, &description)
            }
            None => {
                let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
                command::list_actions(&project, args.json, 0, 10)
            }
        },
        Subcommands::Metrics {
            command_name,
            props,
        } => {
            let event = &mut Event::new(command_name.into());
            if let Ok(props) = Props::from_json_string(&props) {
                props.update_event(event);
            }
            Metrics::capture_blocking(&app_settings, event.clone()).await;
            Ok(())
        }
        Subcommands::Claude(claude::Platform { cmd }) => match cmd {
            claude::Subcommands::PreTool => {
                let result = but_claude::hooks::handle_pre_tool_call();
                let p = props(start, &result);
                result.out_json();
                metrics_if_configured(app_settings, CommandName::ClaudePreTool, p).ok();
                Ok(())
            }
            claude::Subcommands::PostTool => {
                let result = but_claude::hooks::handle_post_tool_call();
                let p = props(start, &result);
                result.out_json();
                metrics_if_configured(app_settings, CommandName::ClaudePostTool, p).ok();
                Ok(())
            }
            claude::Subcommands::Stop => {
                let result = but_claude::hooks::handle_stop().await;
                let p = props(start, &result);
                result.out_json();
                metrics_if_configured(app_settings, CommandName::ClaudeStop, p).ok();
                Ok(())
            }
            claude::Subcommands::PermissionPromptMcp { session_id } => {
                but_claude::mcp::start(&args.current_dir, &session_id).await
            }
            claude::Subcommands::Last { offset } => {
                let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
                let mut ctx = gitbutler_command_context::CommandContext::open(
                    &project,
                    app_settings.clone(),
                )?;
                let message = but_claude::db::get_user_message(&mut ctx, Some(offset as i64))?;
                match message {
                    Some(msg) => {
                        if args.json {
                            // For JSON output, include timestamp and message
                            let output = serde_json::json!({
                                "timestamp": msg.created_at().format("%Y-%m-%d %H:%M:%S").to_string(),
                                "message": match msg.content() {
                                    but_claude::MessagePayload::User(input) => &input.message,
                                    _ => "",
                                }
                            });
                            println!("{}", serde_json::to_string_pretty(&output)?);
                        } else {
                            // For human-readable output, show timestamp and message
                            println!(
                                "{} {}",
                                "Timestamp:".bold(),
                                msg.created_at()
                                    .format("%Y-%m-%d %H:%M:%S")
                                    .to_string()
                                    .cyan()
                            );
                            match msg.content() {
                                but_claude::MessagePayload::User(input) => {
                                    println!("{}", input.message);
                                }
                                _ => {
                                    println!("{}", "Not a user input message".red());
                                }
                            }
                        }
                    }
                    None => {
                        if args.json {
                            println!("null");
                        } else {
                            println!("No user message found at offset {}", offset);
                        }
                    }
                }
                Ok(())
            }
        },
        Subcommands::Cursor(cursor::Platform { cmd }) => match cmd {
            cursor::Subcommands::AfterEdit => {
                let mut stdout = std::io::stdout();
                let result = but_cursor::handle_after_edit().await;
                let p = props(start, &result);
                writeln!(stdout, "{}", serde_json::to_string(&result?)?).ok();
                metrics_if_configured(app_settings, CommandName::CursorStop, p).ok();
                Ok(())
            }
            cursor::Subcommands::Stop { nightly } => {
                let mut stdout = std::io::stdout();
                let result = but_cursor::handle_stop(nightly).await;
                let p = props(start, &result);
                writeln!(stdout, "{}", serde_json::to_string(&result?)?).ok();
                metrics_if_configured(app_settings, CommandName::CursorStop, p).ok();
                Ok(())
            }
        },
        Subcommands::Base(base::Platform { cmd }) => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let metrics_cmd = match cmd {
                base::Subcommands::Check => CommandName::BaseCheck,
                base::Subcommands::Update => CommandName::BaseUpdate,
            };
            let result = base::handle(cmd, &project, args.json);
            metrics_if_configured(app_settings, metrics_cmd, props(start, &result)).ok();
            Ok(())
        }
        Subcommands::Branch(branch::Platform { cmd }) => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let metrics_command = match cmd {
                None | Some(branch::Subcommands::List { .. }) => CommandName::BranchList,
                Some(branch::Subcommands::New { .. }) => CommandName::BranchNew,
                Some(branch::Subcommands::Delete { .. }) => CommandName::BranchDelete,
                Some(branch::Subcommands::Unapply { .. }) => CommandName::BranchUnapply,
                Some(branch::Subcommands::Apply { .. }) => CommandName::BranchApply,
            };
            let result = branch::handle(cmd, &project, args.json).await;
            metrics_if_configured(app_settings, metrics_command, props(start, &result)).ok();
            result
        }
        Subcommands::Worktree(worktree::Platform { cmd }) => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = worktree::handle(cmd, &project, args.json);
            metrics_if_configured(app_settings, CommandName::Worktree, props(start, &result)).ok();
            result
        }
        Subcommands::Log => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = log::commit_graph(&project, args.json);
            metrics_if_configured(app_settings, CommandName::Log, props(start, &result)).ok();
            result?;
            Ok(())
        }
        Subcommands::Status {
            show_files,
            verbose,
            review,
        } => {
            let project = get_or_init_context_with_legacy_support(&args.current_dir)?;
            let result = status::worktree(&project, args.json, show_files, verbose, review).await;
            metrics_if_configured(app_settings, CommandName::Status, props(start, &result)).ok();
            result
        }
        Subcommands::Stf { verbose, review } => {
            let project = get_or_init_context_with_legacy_support(&args.current_dir)?;
            let result = status::worktree(&project, args.json, true, verbose, review).await;
            metrics_if_configured(app_settings, CommandName::Stf, props(start, &result)).ok();
            result
        }
        Subcommands::Rub { source, target } => {
            let mut stderr = std::io::stderr();
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result =
                rub::handle(&project, args.json, &source, &target).context("Rubbed the wrong way.");
            if let Err(e) = &result {
                writeln!(stderr, "{} {}", e, e.root_cause()).ok();
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            result
        }
        Subcommands::Mark { target, delete } => {
            let mut stderr = std::io::stderr();
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = mark::handle(&project, args.json, &target, delete)
                .context("Can't mark this. Taaaa-na-na-na. Can't mark this.");
            if let Err(e) = &result {
                writeln!(stderr, "{} {}", e, e.root_cause()).ok();
            }
            metrics_if_configured(app_settings, CommandName::Mark, props(start, &result)).ok();
            result
        }
        Subcommands::Unmark => {
            let mut stderr = std::io::stderr();
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = mark::unmark(&project, args.json)
                .context("Can't unmark this. Taaaa-na-na-na. Can't unmark this.");
            if let Err(e) = &result {
                writeln!(stderr, "{} {}", e, e.root_cause()).ok();
            }
            metrics_if_configured(app_settings, CommandName::Unmark, props(start, &result)).ok();
            result
        }
        Subcommands::Gui => {
            let result = gui::open(&args.current_dir);
            metrics_if_configured(app_settings, CommandName::Gui, props(start, &result)).ok();
            result
        }
        Subcommands::Commit {
            message,
            branch,
            create,
            only,
        } => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = commit::commit(
                &project,
                args.json,
                message.as_deref(),
                branch.as_deref(),
                only,
                create,
            );
            metrics_if_configured(app_settings, CommandName::Commit, props(start, &result)).ok();
            result
        }
        Subcommands::Push(push_args) => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = push::handle(push_args, &project, args.json);
            metrics_if_configured(app_settings, CommandName::Push, props(start, &result)).ok();
            result
        }
        Subcommands::New { target } => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = commit::insert_blank_commit(&project, args.json, &target);
            metrics_if_configured(app_settings, CommandName::New, props(start, &result)).ok();
            result
        }
        Subcommands::Describe { target } => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = describe::describe_target(&project, args.json, &target);
            metrics_if_configured(app_settings, CommandName::Describe, props(start, &result)).ok();
            result
        }
        Subcommands::Oplog { since } => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = oplog::show_oplog(&project, args.json, since.as_deref());
            metrics_if_configured(app_settings, CommandName::Oplog, props(start, &result)).ok();
            result
        }
        Subcommands::Restore { oplog_sha, force } => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = oplog::restore_to_oplog(&project, args.json, &oplog_sha, force);
            metrics_if_configured(app_settings, CommandName::Restore, props(start, &result)).ok();
            result
        }
        Subcommands::Undo => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = oplog::undo_last_operation(&project, args.json);
            metrics_if_configured(app_settings, CommandName::Undo, props(start, &result)).ok();
            result
        }
        Subcommands::Snapshot { message } => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = oplog::create_snapshot(&project, args.json, message.as_deref());
            metrics_if_configured(app_settings, CommandName::Snapshot, props(start, &result)).ok();
            result
        }
        Subcommands::Absorb { source } => {
            let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
            let result = absorb::handle(&project, args.json, source.as_deref());
            metrics_if_configured(app_settings, CommandName::Absorb, props(start, &result)).ok();
            result
        }
        Subcommands::Init { repo } => init::repo(&args.current_dir, args.json, repo)
            .context("Failed to initialize GitButler project."),
        Subcommands::Forge(forge::integration::Platform { cmd }) => {
            let metrics_cmd = match cmd {
                forge::integration::Subcommands::Auth => CommandName::ForgeAuth,
                forge::integration::Subcommands::ListUsers => CommandName::ForgeListUsers,
                forge::integration::Subcommands::Forget { .. } => CommandName::ForgeForget,
            };
            let result = forge::integration::handle(cmd).await;
            metrics_if_configured(app_settings, metrics_cmd, props(start, &result)).ok();
            result
        }
        Subcommands::Review(forge::review::Platform { cmd }) => match cmd {
            forge::review::Subcommands::Publish {
                branch,
                skip_force_push_protection,
                with_force,
                run_hooks,
                default,
            } => {
                let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
                let result = forge::review::publish_reviews(
                    &project,
                    branch,
                    skip_force_push_protection,
                    with_force,
                    run_hooks,
                    default,
                    args.json,
                )
                .await
                .context("Failed to publish reviews for branches.");
                metrics_if_configured(
                    app_settings,
                    CommandName::PublishReview,
                    props(start, &result),
                )
                .ok();
                result
            }
            forge::review::Subcommands::Template { template_path } => {
                let project = get_or_init_legacy_non_bare_project(&args.current_dir)?;
                let result = forge::review::set_review_template(&project, template_path, args.json)
                    .context("Failed to set review template.");
                metrics_if_configured(
                    app_settings,
                    CommandName::ReviewTemplate,
                    props(start, &result),
                )
                .ok();
                result
            }
        },
        Subcommands::Completions { shell } => completions::generate_completions(shell),
    }
}

fn get_or_init_legacy_non_bare_project(
    current_dir: &std::path::Path,
) -> anyhow::Result<LegacyProject> {
    let repo = gix::discover(current_dir)?;
    if let Some(path) = repo.workdir() {
        let project = match LegacyProject::find_by_worktree_dir(path) {
            Ok(p) => Ok(p),
            Err(_e) => {
                crate::init::repo(path, false, false)?;
                LegacyProject::find_by_worktree_dir(path)
            }
        }?;
        Ok(project)
    } else {
        let mut stdout = std::io::stdout();
        let error_desc = "Bare repositories are not supported.";
        writeln!(stdout, "{error_desc}").ok();
        anyhow::bail!(error_desc);
    }
}

/// Legacy - none of this should be kept.
/// Turn this instance into a project, which knows about the Git repository discovered from `directory`
/// and which can derive all other information from there.
pub fn get_or_init_context_with_legacy_support(
    directory: impl AsRef<Path>,
) -> anyhow::Result<but_ctx::Context> {
    let directory = directory.as_ref();
    let repo = gix::discover(directory)?;
    let worktree_dir = repo
        .workdir()
        .context("Bare repositories are not yet supported.")?;
    let project = LegacyProject::find_by_worktree_dir_opt(worktree_dir)?
        .map(anyhow::Ok)
        .unwrap_or_else(|| {
            init::repo(directory, false, false)
                .and_then(|()| LegacyProject::find_by_worktree_dir(directory))
        })?;
    Ok(but_ctx::Context {
        settings: AppSettings::load_from_default_path_creating()?,
        legacy_project: project,
        repo,
    })
}

/// Discover the Git repository in `directory` and return it,
pub fn get_context_with_legacy_support(
    directory: impl AsRef<Path>,
) -> anyhow::Result<but_ctx::Context> {
    let directory = directory.as_ref();
    let repo = gix::discover(directory)?;
    let worktree_dir = repo
        .workdir()
        .context("Bare repositories are not yet supported.")?;
    let project = LegacyProject::find_by_worktree_dir(worktree_dir)?;
    Ok(but_ctx::Context {
        settings: AppSettings::load_from_default_path_creating()?,
        legacy_project: project,
        repo,
    })
}

pub(crate) fn props<E, T, R>(start: std::time::Instant, result: R) -> Props
where
    R: std::ops::Deref<Target = Result<T, E>>,
    E: std::fmt::Display,
{
    let error = result.as_ref().err().map(|e| e.to_string());
    let mut props = Props::new();
    props.insert("durationMs", start.elapsed().as_millis());
    props.insert("error", error);
    props
}

fn print_grouped_help() -> std::io::Result<()> {
    use std::collections::HashSet;

    use clap::CommandFactory;
    use terminal_size::{Width, terminal_size};

    let mut stdout = std::io::stdout();

    // Get terminal width, default to 80 if detection fails
    let terminal_width = if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        80
    };

    // Helper function to truncate text to fit within available width
    let truncate_text = |text: &str, available_width: usize| -> String {
        const ELLIPSIS_LEN: usize = 1;
        if text.len() <= available_width {
            text.to_string()
        } else if available_width > ELLIPSIS_LEN {
            format!("{}…", &text[..available_width.saturating_sub(ELLIPSIS_LEN)])
        } else {
            text.chars().take(available_width).collect()
        }
    };

    let cmd = Args::command();
    let subcommands: Vec<_> = cmd.get_subcommands().collect();

    // Define command groupings and their order (excluding MISC)
    let groups = [
        ("Inspection".yellow(), vec!["status"]),
        (
            "Branching and Committing".yellow(),
            vec!["commit", "new", "branch", "base", "mark", "unmark"],
        ),
        (
            "Server Interactions".yellow(),
            vec!["push", "review", "forge"],
        ),
        (
            "Editing Commits".yellow(),
            vec!["rub", "describe", "absorb"],
        ),
        (
            "Operation History".yellow(),
            vec!["oplog", "undo", "restore", "snapshot"],
        ),
    ];

    writeln!(
        stdout,
        "{}",
        "The GitButler CLI change control system".red()
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "Usage: but [OPTIONS] <COMMAND>")?;
    writeln!(stdout, "       but [OPTIONS] [RUB-SOURCE] [RUB-TARGET]")?;
    writeln!(stdout)?;
    writeln!(
        stdout,
        "The GitButler CLI can be used to do nearly anything the desktop client can do (and more)."
    )?;
    writeln!(
        stdout,
        "It is a drop in replacement for most of the Git commands you would normally use, but Git"
    )?;
    writeln!(
        stdout,
        "commands (blame, log, etc) can also be used, as GitButler is fully Git compatible."
    )?;
    writeln!(stdout)?;
    writeln!(
        stdout,
        "Checkout the full docs here: https://docs.gitbutler.com/cli-overview"
    )?;
    writeln!(stdout)?;

    // Keep track of which commands we've already printed
    let mut printed_commands = HashSet::new();
    const LONGEST_COMMAND_LEN: usize = 13;
    const LONGEST_COMMAND_LEN_AND_ELLIPSIS: usize = LONGEST_COMMAND_LEN + 3;

    // Print grouped commands
    for (group_name, command_names) in &groups {
        writeln!(stdout, "{group_name}:")?;
        for cmd_name in command_names {
            if let Some(subcmd) = subcommands.iter().find(|c| c.get_name() == *cmd_name) {
                let about = subcmd.get_about().unwrap_or_default().to_string();
                // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
                let available_width =
                    terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
                let truncated_about = truncate_text(&about, available_width);
                writeln!(
                    stdout,
                    "  {:<LONGEST_COMMAND_LEN$}{}",
                    cmd_name.green(),
                    truncated_about,
                )?;
                printed_commands.insert(cmd_name.to_string());
            }
        }
        writeln!(stdout)?;
    }

    // Collect any remaining commands not in the explicit groups
    let misc_commands: Vec<_> = subcommands
        .iter()
        .filter(|subcmd| !printed_commands.contains(subcmd.get_name()) && !subcmd.is_hide_set())
        .collect();

    // Print MISC section if there are any ungrouped commands
    if !misc_commands.is_empty() {
        writeln!(stdout, "{}:", "Other Commands".yellow())?;
        for subcmd in misc_commands {
            let about = subcmd.get_about().unwrap_or_default().to_string();
            // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            writeln!(
                stdout,
                "  {:<LONGEST_COMMAND_LEN$}{}",
                subcmd.get_name().green(),
                truncated_about
            )?;
        }
        writeln!(stdout)?;
    }

    // Add command completion instructions
    writeln!(
        stdout,
        "To add command completion, add this to your shell rc: (for example ~/.zshrc)"
    )?;
    writeln!(stdout, "  eval \"$(but completions zsh)\"")?;
    writeln!(stdout)?;

    writeln!(
        stdout,
        "To use the GitButler CLI with coding agents (Claude Code hooks, Cursor hooks, MCP), see:"
    )?;
    writeln!(
        stdout,
        "  https://docs.gitbutler.com/features/ai-integration/ai-overview"
    )?;
    writeln!(stdout)?;

    writeln!(stdout, "{}:", "Options".yellow())?;
    // Truncate long option descriptions if needed
    let option_descriptions = [
        (
            "  -C, --current-dir <PATH>",
            "Run as if but was started in PATH instead of the current working directory [default: .]",
        ),
        ("  -j, --json", "Whether to use JSON output format"),
        ("  -h, --help", "Print help"),
    ];

    for (flag, desc) in option_descriptions {
        let available_width = terminal_width.saturating_sub(flag.len() + 2);
        let truncated_desc = truncate_text(desc, available_width);
        writeln!(stdout, "{}  {}", flag, truncated_desc)?;
    }

    Ok(())
}

mod trace {
    use tracing::metadata::LevelFilter;
    use tracing_subscriber::{
        Layer, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
    };

    pub fn init(level: u8) -> anyhow::Result<()> {
        let filter = match level {
            1 => LevelFilter::INFO,
            2 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        };
        if level >= 4 {
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .compact()
                        .with_span_events(FmtSpan::CLOSE)
                        .with_writer(std::io::stderr),
                )
                .with(
                    tracing_forest::ForestLayer::from(
                        tracing_forest::printer::PrettyPrinter::new().writer(std::io::stderr),
                    )
                    .with_filter(filter),
                )
                .init()
        } else {
            tracing_subscriber::registry()
                .with(
                    tracing_forest::ForestLayer::from(
                        tracing_forest::printer::PrettyPrinter::new().writer(std::io::stderr),
                    )
                    .with_filter(filter),
                )
                .init();
        }
        Ok(())
    }
}

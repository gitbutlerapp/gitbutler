use std::ffi::OsString;

use anyhow::{Context, Result};

mod args;
use args::{Args, CommandName, Subcommands, actions, claude, cursor};
use but_claude::hooks::OutputAsJson;
use but_settings::AppSettings;
use colored::Colorize;
use gix::date::time::CustomFormat;
use metrics::{Event, Metrics, Props, metrics_if_configured};

mod base;
mod branch;
mod command;
mod commit;
mod completions;
mod describe;
mod editor;
mod forge;
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

/// Handle `args` which must be what's passed by `std::env::args_os()`.
pub async fn handle_args(args: impl Iterator<Item = OsString>) -> Result<()> {
    let args: Vec<_> = args.collect();

    // Check if help is requested with no subcommand
    if args.len() == 1 || args.iter().any(|arg| arg == "--help" || arg == "-h") && args.len() == 2 {
        print_grouped_help();
        return Ok(());
    }

    // The but push --help output is different if gerrit mode is enabled, hence the special handling
    let args_vec: Vec<String> = std::env::args().collect();
    if args_vec.iter().any(|arg| arg == "push")
        && args_vec.iter().any(|arg| arg == "--help" || arg == "-h")
    {
        push::print_help();
        return Ok(());
    }

    let args: Args = clap::Parser::parse_from(args);
    let app_settings = AppSettings::load_from_default_path_creating()?;

    if args.trace > 0 {
        trace::init(args.trace)?;
    }

    let namespace = option_env!("IDENTIFIER").unwrap_or("com.gitbutler.app");
    but_secret::secret::set_application_namespace(namespace);
    let start = std::time::Instant::now();

    match &args.cmd {
        Subcommands::Mcp { internal } => {
            if *internal {
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
                let handler = *handler;
                let project = get_or_init_project(&args.current_dir)?;
                command::handle_changes(&project, args.json, handler, description)
            }
            None => {
                let project = get_or_init_project(&args.current_dir)?;
                command::list_actions(&project, args.json, 0, 10)
            }
        },
        Subcommands::Metrics {
            command_name,
            props,
        } => {
            let event = &mut Event::new((*command_name).into());
            if let Ok(props) = Props::from_json_string(props) {
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
            claude::Subcommands::PermissionPromptMcp => {
                but_claude::mcp::start(&args.current_dir).await
            }
        },
        Subcommands::Cursor(cursor::Platform { cmd }) => match cmd {
            cursor::Subcommands::AfterEdit => {
                let result = but_cursor::handle_after_edit().await;
                let p = props(start, &result);
                println!("{}", serde_json::to_string(&result?)?);
                metrics_if_configured(app_settings, CommandName::CursorStop, p).ok();
                Ok(())
            }
            cursor::Subcommands::Stop { nightly } => {
                let result = but_cursor::handle_stop(*nightly).await;
                let p = props(start, &result);
                println!("{}", serde_json::to_string(&result?)?);
                metrics_if_configured(app_settings, CommandName::CursorStop, p).ok();
                Ok(())
            }
        },
        Subcommands::Base(base::Platform { cmd }) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = base::handle(cmd, &project, args.json);
            metrics_if_configured(
                app_settings,
                match cmd {
                    base::Subcommands::Check => CommandName::BaseCheck,
                    base::Subcommands::Update => CommandName::BaseUpdate,
                },
                props(start, &result),
            )
            .ok();
            Ok(())
        }
        Subcommands::Branch(branch::Platform { cmd }) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = branch::handle(cmd, &project, args.json).await;
            let metrics_command = match cmd {
                None | Some(branch::Subcommands::List { .. }) => CommandName::BranchList,
                Some(branch::Subcommands::New { .. }) => CommandName::BranchNew,
                Some(branch::Subcommands::Delete { .. }) => CommandName::BranchDelete,
                Some(branch::Subcommands::Unapply { .. }) => CommandName::BranchUnapply,
            };
            metrics_if_configured(app_settings, metrics_command, props(start, &result)).ok();
            result
        }
        Subcommands::Worktree(worktree::Platform { cmd }) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = worktree::handle(cmd, &project, args.json);
            metrics_if_configured(app_settings, CommandName::Worktree, props(start, &result)).ok();
            result
        }
        Subcommands::Log => {
            let project = get_or_init_project(&args.current_dir)?;
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
            let project = get_or_init_project(&args.current_dir)?;
            let result =
                status::worktree(&project, args.json, *show_files, *verbose, *review).await;
            metrics_if_configured(app_settings, CommandName::Status, props(start, &result)).ok();
            result
        }
        Subcommands::Stf { verbose, review } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = status::worktree(&project, args.json, true, *verbose, *review).await;
            metrics_if_configured(app_settings, CommandName::Stf, props(start, &result)).ok();
            result
        }
        Subcommands::Rub { source, target } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result =
                rub::handle(&project, args.json, source, target).context("Rubbed the wrong way.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            result
        }
        Subcommands::Mark { target, delete } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = mark::handle(&project, args.json, target, *delete)
                .context("Can't mark this. Taaaa-na-na-na. Can't mark this.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            result
        }
        Subcommands::Unmark => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = mark::unmark(&project, args.json)
                .context("Can't unmark this. Taaaa-na-na-na. Can't unmark this.");
            if let Err(e) = &result {
                eprintln!("{} {}", e, e.root_cause());
            }
            metrics_if_configured(app_settings, CommandName::Rub, props(start, &result)).ok();
            result
        }
        Subcommands::Commit {
            message,
            branch,
            create,
            only,
        } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = commit::commit(
                &project,
                args.json,
                message.as_deref(),
                branch.as_deref(),
                *only,
                *create,
            );
            metrics_if_configured(app_settings, CommandName::Commit, props(start, &result)).ok();
            result
        }
        Subcommands::Push(push_args) => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = push::handle(push_args, &project, args.json);
            metrics_if_configured(app_settings, CommandName::Push, props(start, &result)).ok();
            result
        }
        Subcommands::New { target } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = commit::insert_blank_commit(&project, args.json, target);
            metrics_if_configured(app_settings, CommandName::New, props(start, &result)).ok();
            result
        }
        Subcommands::Describe { target } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = describe::describe_target(&project, args.json, target);
            metrics_if_configured(app_settings, CommandName::Describe, props(start, &result)).ok();
            result
        }
        Subcommands::Oplog { since } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::show_oplog(&project, args.json, since.as_deref());
            metrics_if_configured(app_settings, CommandName::Oplog, props(start, &result)).ok();
            result
        }
        Subcommands::Restore { oplog_sha, force } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::restore_to_oplog(&project, args.json, oplog_sha, *force);
            metrics_if_configured(app_settings, CommandName::Restore, props(start, &result)).ok();
            result
        }
        Subcommands::Undo => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::undo_last_operation(&project, args.json);
            metrics_if_configured(app_settings, CommandName::Undo, props(start, &result)).ok();
            result
        }
        Subcommands::Snapshot { message } => {
            let project = get_or_init_project(&args.current_dir)?;
            let result = oplog::create_snapshot(&project, args.json, message.as_deref());
            metrics_if_configured(app_settings, CommandName::Snapshot, props(start, &result)).ok();
            result
        }
        Subcommands::Init { repo } => init::repo(&args.current_dir, args.json, *repo)
            .context("Failed to initialize GitButler project."),
        Subcommands::Forge(forge::integration::Platform { cmd }) => {
            let result = forge::integration::handle(cmd).await;
            let metrics_cmd = match cmd {
                forge::integration::Subcommands::Auth => CommandName::ForgeAuth,
                forge::integration::Subcommands::ListUsers => CommandName::ForgeListUsers,
                forge::integration::Subcommands::Forget { .. } => CommandName::ForgeForget,
            };
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
                let project = get_or_init_project(&args.current_dir)?;
                let result = forge::review::publish_reviews(
                    &project,
                    branch,
                    *skip_force_push_protection,
                    *with_force,
                    *run_hooks,
                    *default,
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
        },
        Subcommands::Completions { shell } => completions::generate_completions(*shell),
    }
}

fn get_or_init_project(
    current_dir: &std::path::Path,
) -> anyhow::Result<gitbutler_project::Project> {
    let repo = gix::discover(current_dir)?;
    if let Some(path) = repo.workdir() {
        let project = match gitbutler_project::Project::find_by_worktree_dir(path) {
            Ok(p) => Ok(p),
            Err(_e) => {
                crate::init::repo(path, false, false)?;
                gitbutler_project::Project::find_by_worktree_dir(path)
            }
        }?;
        Ok(project)
    } else {
        let error_desc = "Bare repositories are not supported.";
        println!("{error_desc}");
        anyhow::bail!(error_desc);
    }
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

fn print_grouped_help() {
    use std::collections::HashSet;

    use clap::CommandFactory;
    use terminal_size::{Width, terminal_size};

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
        ("Inspection".yellow(), vec!["status", "log"]),
        (
            "Branching and Committing".yellow(),
            vec!["commit", "push", "new", "branch", "base", "mark", "unmark"],
        ),
        ("Editing Commits".yellow(), vec!["rub", "describe"]),
        (
            "Operation History".yellow(),
            vec!["oplog", "undo", "restore", "snapshot"],
        ),
    ];

    println!("{}", "The GitButler CLI change control system".red());
    println!();
    println!("Usage: but [OPTIONS] <COMMAND>");
    println!();

    // Keep track of which commands we've already printed
    let mut printed_commands = HashSet::new();
    const LONGEST_COMMAND_LEN: usize = 13;
    const LONGEST_COMMAND_LEN_AND_ELLIPSIS: usize = LONGEST_COMMAND_LEN + 3;

    // Print grouped commands
    for (group_name, command_names) in &groups {
        println!("{group_name}:");
        for cmd_name in command_names {
            if let Some(subcmd) = subcommands.iter().find(|c| c.get_name() == *cmd_name) {
                let about = subcmd.get_about().unwrap_or_default().to_string();
                // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
                let available_width =
                    terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
                let truncated_about = truncate_text(&about, available_width);
                println!(
                    "  {:<LONGEST_COMMAND_LEN$}{}",
                    cmd_name.green(),
                    truncated_about,
                );
                printed_commands.insert(cmd_name.to_string());
            }
        }
        println!();
    }

    // Collect any remaining commands not in the explicit groups
    let misc_commands: Vec<_> = subcommands
        .iter()
        .filter(|subcmd| !printed_commands.contains(subcmd.get_name()) && !subcmd.is_hide_set())
        .collect();

    // Print MISC section if there are any ungrouped commands
    if !misc_commands.is_empty() {
        println!("{}:", "Other Commands".yellow());
        for subcmd in misc_commands {
            let about = subcmd.get_about().unwrap_or_default().to_string();
            // Calculate available width: terminal_width - indent (2) - command column (10) - buffer (1)
            let available_width = terminal_width.saturating_sub(LONGEST_COMMAND_LEN_AND_ELLIPSIS);
            let truncated_about = truncate_text(&about, available_width);
            println!(
                "  {:<LONGEST_COMMAND_LEN$}{}",
                subcmd.get_name().green(),
                truncated_about
            );
        }
        println!();
    }

    println!("{}:", "Options".yellow());
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
        println!("{}  {}", flag, truncated_desc);
    }
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

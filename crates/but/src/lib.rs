#![deny(unsafe_code)]

use std::{ffi::OsString, path::Path};

use anyhow::{Context, Result};

mod args;
use args::{Args, Subcommands, actions, claude, cursor};
use but_claude::hooks::OutputAsJson;
use but_settings::AppSettings;
use colored::Colorize;
use gix::date::time::CustomFormat;
use metrics::{Event, Metrics, Props};

use crate::{
    args::CommandName,
    metrics::MetricsContext,
    utils::{
        OutputChannel, OutputFormat, ResultErrorExt, ResultJsonExt, ResultMetricsExt,
        print_grouped_help,
    },
};

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
        let mut out = OutputChannel::new_with_pager(OutputFormat::Human);
        print_grouped_help(&mut out)?;
        return Ok(());
    }

    // The `but push --help` output is different if gerrit mode is enabled, hence the special handling
    let args_vec: Vec<String> = std::env::args().collect();
    // TODO: handle this as part of clap, it can be told to not generate all help.
    if args_vec.iter().any(|arg| arg == "push")
        && args_vec.iter().any(|arg| arg == "--help" || arg == "-h")
    {
        let mut out = OutputChannel::new_with_pager(OutputFormat::Human);
        push::print_help(&mut out)?;
        return Ok(());
    }

    let mut args: Args = clap::Parser::parse_from(args);
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let output_format = if args.json {
        OutputFormat::Json
    } else {
        args.format
    };
    // Set it so code past this point can assume it's set.;
    let mut out = OutputChannel::new_with_pager(output_format);

    if args.trace > 0 {
        trace::init(args.trace)?;
    }

    let namespace = option_env!("IDENTIFIER").unwrap_or("com.gitbutler.app");
    but_secret::secret::set_application_namespace(namespace);

    // If no subcommand is provided, but we have source and target, default to rub
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
            let project = get_or_init_legacy_non_bare_project(&args)?;
            rub::handle(&project, &mut out, source, target)
                .context("Rubbed the wrong way.")
                .emit_metrics(MetricsContext::new_if_enabled(
                    &app_settings,
                    CommandName::Rub,
                ))
                .show_root_cause_error_then_exit_without_destructors(out)
        }
        None if args.source_or_path.is_some() && args.target.is_none() => {
            // If only one argument is provided without a subcommand, check if this is a valid path.
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
            print_grouped_help(&mut out)?;
            Ok(())
        }
        Some(cmd) => match_subcommand(cmd, args, app_settings, out).await,
    }
}

async fn match_subcommand(
    cmd: Subcommands,
    args: Args,
    app_settings: AppSettings,
    mut output: OutputChannel,
) -> Result<()> {
    let out = &mut output;
    let metrics_ctx = cmd.to_metrics_context(&app_settings);

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
                let project = get_or_init_legacy_non_bare_project(&args)?;
                command::handle_changes(&project, out, handler, &description)
            }
            None => {
                let project = get_or_init_legacy_non_bare_project(&args)?;
                command::list_actions(&project, out, 0, 10)
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
            claude::Subcommands::PreTool => but_claude::hooks::handle_pre_tool_call()
                .output_claude_json()
                .emit_metrics(metrics_ctx),
            claude::Subcommands::PostTool => but_claude::hooks::handle_post_tool_call()
                .output_claude_json()
                .emit_metrics(metrics_ctx),
            claude::Subcommands::Stop => but_claude::hooks::handle_stop()
                .await
                .output_claude_json()
                .emit_metrics(metrics_ctx),
            claude::Subcommands::PermissionPromptMcp { session_id } => {
                but_claude::mcp::start(&args.current_dir, &session_id).await
            }
            claude::Subcommands::Last { offset } => {
                let project = get_or_init_legacy_non_bare_project(&args)?;
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
            cursor::Subcommands::AfterEdit => but_cursor::handle_after_edit()
                .await
                .output_json(true)
                .emit_metrics(metrics_ctx),
            cursor::Subcommands::Stop { nightly } => but_cursor::handle_stop(nightly)
                .await
                .output_json(true)
                .emit_metrics(metrics_ctx),
        },
        Subcommands::Base(base::Platform { cmd }) => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            base::handle(cmd, &project, out)
                .await
                .emit_metrics(metrics_ctx)
        }
        Subcommands::Branch(branch::Platform { cmd }) => {
            let ctx = get_or_init_context_with_legacy_support(&args)?;
            branch::handle(cmd, &ctx, out)
                .await
                .emit_metrics(metrics_ctx)
        }
        Subcommands::Worktree(worktree::Platform { cmd }) => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            worktree::handle(cmd, &project, out)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        Subcommands::Log => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            log::commit_graph(&project, out).emit_metrics(metrics_ctx)
        }
        Subcommands::Status {
            show_files,
            verbose,
            review,
        } => {
            let project = get_or_init_context_with_legacy_support(&args)?;
            status::worktree(&project, out, show_files, verbose, review)
                .await
                .emit_metrics(metrics_ctx)
        }
        Subcommands::Stf { verbose, review } => {
            let project = get_or_init_context_with_legacy_support(&args)?;
            status::worktree(&project, out, true, verbose, review)
                .await
                .emit_metrics(metrics_ctx)
        }
        Subcommands::Rub { source, target } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            rub::handle(&project, out, &source, &target)
                .context("Rubbed the wrong way.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        Subcommands::Mark { target, delete } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            mark::handle(&project, out, &target, delete)
                .context("Can't mark this. Taaaa-na-na-na. Can't mark this.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        Subcommands::Unmark => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            mark::unmark(&project, out)
                .context("Can't unmark this. Taaaa-na-na-na. Can't unmark this.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        Subcommands::Gui => gui::open(&args.current_dir).emit_metrics(metrics_ctx),
        Subcommands::Commit {
            message,
            branch,
            create,
            only,
        } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            commit::commit(
                &project,
                out,
                message.as_deref(),
                branch.as_deref(),
                only,
                create,
            )
            .emit_metrics(metrics_ctx)
        }
        Subcommands::Push(push_args) => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            push::handle(push_args, &project, out).emit_metrics(metrics_ctx)
        }
        Subcommands::New { target } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            commit::insert_blank_commit(&project, out, &target).emit_metrics(metrics_ctx)
        }
        Subcommands::Describe { target } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            describe::describe_target(&project, out, &target).emit_metrics(metrics_ctx)
        }
        Subcommands::Oplog { since } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            oplog::show_oplog(&project, out, since.as_deref()).emit_metrics(metrics_ctx)
        }
        Subcommands::Restore { oplog_sha, force } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            oplog::restore_to_oplog(&project, out, &oplog_sha, force).emit_metrics(metrics_ctx)
        }
        Subcommands::Undo => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            oplog::undo_last_operation(&project, out).emit_metrics(metrics_ctx)
        }
        Subcommands::Snapshot { message } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            oplog::create_snapshot(&project, out, message.as_deref()).emit_metrics(metrics_ctx)
        }
        Subcommands::Absorb { source } => {
            let project = get_or_init_legacy_non_bare_project(&args)?;
            absorb::handle(&project, out, source.as_deref()).emit_metrics(metrics_ctx)
        }
        Subcommands::Init { repo } => init::repo(&args.current_dir, out, repo)
            .context("Failed to initialize GitButler project.")
            .emit_metrics(metrics_ctx),
        Subcommands::Forge(forge::integration::Platform { cmd }) => {
            forge::integration::handle(cmd, out)
                .await
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        Subcommands::Review(forge::review::Platform { cmd }) => match cmd {
            forge::review::Subcommands::Publish {
                branch,
                skip_force_push_protection,
                with_force,
                run_hooks,
                default,
            } => {
                let project = get_or_init_legacy_non_bare_project(&args)?;
                forge::review::publish_reviews(
                    &project,
                    branch,
                    skip_force_push_protection,
                    with_force,
                    run_hooks,
                    default,
                    out,
                )
                .await
                .context("Failed to publish reviews for branches.")
                .emit_metrics(metrics_ctx)
            }
            forge::review::Subcommands::Template { template_path } => {
                let project = get_or_init_legacy_non_bare_project(&args)?;
                forge::review::set_review_template(&project, template_path, out)
                    .context("Failed to set review template.")
                    .emit_metrics(metrics_ctx)
            }
        },
        Subcommands::Completions { shell } => {
            completions::generate_completions(shell).emit_metrics(metrics_ctx)
        }
    }
}

fn get_or_init_legacy_non_bare_project(args: &Args) -> anyhow::Result<LegacyProject> {
    let repo = gix::discover(&args.current_dir)?;
    if let Some(path) = repo.workdir() {
        let project = match LegacyProject::find_by_worktree_dir(path) {
            Ok(p) => Ok(p),
            Err(_e) => {
                init::repo(
                    path,
                    &mut OutputChannel::new_without_pager_non_json(args.format),
                    false,
                )?;
                LegacyProject::find_by_worktree_dir(path)
            }
        }?;
        Ok(project)
    } else {
        anyhow::bail!("Bare repositories are not supported.");
    }
}

/// Legacy - none of this should be kept.
/// Turn this instance into a project, which knows about the Git repository discovered from `directory`
/// and which can derive all other information from there.
pub fn get_or_init_context_with_legacy_support(args: &Args) -> anyhow::Result<but_ctx::Context> {
    let directory = &args.current_dir;
    let repo = gix::discover(directory)?;
    let worktree_dir = repo
        .workdir()
        .context("Bare repositories are not yet supported.")?;
    let project = LegacyProject::find_by_worktree_dir_opt(worktree_dir)?
        .map(anyhow::Ok)
        .unwrap_or_else(|| {
            init::repo(
                directory,
                &mut OutputChannel::new_without_pager_non_json(args.format),
                false,
            )
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

mod utils;

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

/// Get the clap Command for documentation generation
pub fn get_command() -> clap::Command {
    use clap::CommandFactory;
    Args::command()
}

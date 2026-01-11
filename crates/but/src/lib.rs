//! ### Organisation
//!
//! * `args`
//!     - `clap` argument structure where the CLI parser is defined.
//! * `command`
//!     - implementations of everything that is ultimately called.
//! * `tui`
//!     - interactive and static components for terminals
//! * `*legacy/`
//!     - code that depends on `gitbutler-*` crates or `but-*` crates with `legacy` feature enabled.
//!
//! ### Testing
//!
//! #### Legacy builds
//!
//! Run `cargo test -p but`, legacy features are the default.
//!
//! #### Non-Legacy builds
//!
//! Tests aren't available in this mode yet, but one can compile it with `cargo check -p but --no-default-features`.
//!
#![deny(unsafe_code)]
#![cfg_attr(not(feature = "legacy"), expect(unused))]

use std::ffi::OsString;

use anyhow::{Context as _, Result};
use cfg_if::cfg_if;

pub mod args;
use args::{
    Args, OutputFormat, Subcommands, actions, alias as alias_args, branch, claude, cursor, forge,
    metrics, worktree,
};
use but_settings::AppSettings;
use colored::Colorize;
use gix::date::time::CustomFormat;

use crate::{
    init::Fetch,
    utils::{
        OneshotMetricsContext, OutputChannel, ResultErrorExt, ResultJsonExt, ResultMetricsExt,
    },
};

mod id;
pub use id::{CliId, IdMap};

mod alias;
/// A place for all command implementations.
pub(crate) mod command;
mod tui;

const CLI_DATE: CustomFormat = gix::date::time::format::ISO8601;

/// Handle `args` which must be what's passed by `std::env::args_os()`.
pub async fn handle_args(args: impl Iterator<Item = OsString>) -> Result<()> {
    let args: Vec<_> = args.collect();

    // Check if version is requested
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        let version = option_env!("VERSION").unwrap_or("dev");
        println!("but {}", version);
        return Ok(());
    }

    // Check if help is requested and show grouped help instead of clap's default
    // Only intercept top-level help (but -h or but --help), not subcommand help
    let has_help_flag = args.iter().any(|arg| arg == "--help" || arg == "-h");
    let has_subcommand = args.len() > 2 && args[1] != "--help" && args[1] != "-h";
    if has_help_flag && !has_subcommand {
        let mut out = OutputChannel::new_without_pager_non_json(OutputFormat::Human);
        command::help::print_grouped(&mut out)?;
        return Ok(());
    }

    // Expand aliases before parsing arguments
    let args = alias::expand_aliases(args)?;

    // The `but push --help` output is different if gerrit mode is enabled, hence the special handling
    let args_vec: Vec<String> = std::env::args().collect();
    // TODO: handle this as part of clap, it can be told to not generate all help.
    if args_vec.iter().any(|arg| arg == "push")
        && args_vec.iter().any(|arg| arg == "--help" || arg == "-h")
    {
        let mut out = OutputChannel::new_without_pager_non_json(OutputFormat::Human);
        command::push::help::print(&mut out)?;
        return Ok(());
    }

    // Handle `but help -h` and `but help --help` to show the grouped help output
    if args_vec.iter().any(|arg| arg == "help")
        && args_vec.iter().any(|arg| arg == "--help" || arg == "-h")
    {
        let mut out = OutputChannel::new_without_pager_non_json(OutputFormat::Human);
        command::help::print_grouped(&mut out)?;
        return Ok(());
    }

    let mut args: Args = clap::Parser::parse_from(args);
    let app_settings = AppSettings::load_from_default_path_creating_without_customization()?;
    let output_format = if args.json {
        OutputFormat::Json
    } else {
        args.format
    };
    // Determine if pager should be used based on the command
    let use_pager = match args.cmd {
        #[cfg(feature = "legacy")]
        Some(Subcommands::Status { .. }) | Some(Subcommands::Oplog(..)) => false,
        Some(Subcommands::Help) => false,
        _ => true,
    };
    let mut out = OutputChannel::new_with_optional_pager(output_format, use_pager);

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
            #[cfg(feature = "legacy")]
            {
                let mut ctx = init::init_ctx(&args, Fetch::None, &mut out)?;
                command::legacy::rub::handle(&mut ctx, &mut out, source, target)
                    .context("Rubbed the wrong way.")
                    .emit_metrics(OneshotMetricsContext::new_if_enabled(
                        &app_settings,
                        metrics::CommandName::Rub,
                    ))
                    .show_root_cause_error_then_exit_without_destructors(out)
            }
            #[cfg(not(feature = "legacy"))]
            todo!("Non-legacy rub isn't implemented yet")
        }
        None if args.source_or_path.is_some() && args.target.is_none() => {
            // If only one argument is provided without a subcommand, check if this is a valid path.
            let maybe_path = args
                .source_or_path
                .as_ref()
                .expect("path is checked to be Some in match guard");
            let path = std::path::Path::new(args.current_dir.as_path()).join(maybe_path);

            // Check if the path exists before trying to open the GUI
            if !path.exists() {
                anyhow::bail!(
                    "\"but {}\" is not a command. Type \"but --help\" to see all available commands.",
                    maybe_path
                );
            }

            command::gui::open(&path)?;
            Ok(())
        }
        None => {
            // No subcommand and no source/target means run the default alias
            // The default alias expands to "status --hint" which provides a helpful entry point
            let default_args = vec![OsString::from("but"), OsString::from("default")];
            let expanded = alias::expand_aliases(default_args)?;
            let mut new_args: Args = clap::Parser::parse_from(expanded);

            // Take the command from the newly parsed args and execute it
            match new_args.cmd.take() {
                Some(cmd) => match_subcommand(cmd, new_args, app_settings, out).await,
                None => {
                    // Fallback to help if default alias somehow doesn't resolve
                    command::help::print_grouped(&mut out)?;
                    Ok(())
                }
            }
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
        Subcommands::Metrics {
            command_name,
            props,
        } => {
            let mut event = utils::metrics::Event::new(command_name.into());
            if let Ok(props) = utils::metrics::Props::from_json_string(&props) {
                props.update_event(&mut event);
            }
            utils::metrics::capture_event_blocking(&app_settings, event).await;
            Ok(())
        }
        Subcommands::Forge(forge::integration::Platform { cmd }) => {
            command::forge::integration::handle(cmd, out)
                .await
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        Subcommands::Gui => command::gui::open(&args.current_dir).emit_metrics(metrics_ctx),
        Subcommands::Completions { shell } => {
            command::completions::generate_completions(shell).emit_metrics(metrics_ctx)
        }
        Subcommands::Help => {
            command::help::print_grouped(out)?;
            Ok(())
        }
        #[cfg(feature = "legacy")]
        Subcommands::Alias(alias_args::Platform { cmd }) => match cmd {
            Some(alias_args::Subcommands::List) | None => {
                command::alias::list(out).emit_metrics(metrics_ctx)
            }
            Some(alias_args::Subcommands::Add {
                name,
                value,
                global,
            }) => {
                let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
                command::alias::add(&mut ctx, out, &name, &value, global).emit_metrics(metrics_ctx)
            }
            Some(alias_args::Subcommands::Remove { name, global }) => {
                let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
                command::alias::remove(&mut ctx, out, &name, global).emit_metrics(metrics_ctx)
            }
        },
        Subcommands::Branch(branch::Platform { cmd }) => {
            cfg_if! {
                if #[cfg(feature = "legacy")]  {
                    let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
                    command::legacy::branch::handle(cmd, &mut ctx, out)
                        .emit_metrics(metrics_ctx)
                } else {
                    let ctx = but_ctx::Context::discover(&args.current_dir)?;
                    command::branch::handle(cmd, ctx, out)
                }
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Mcp { internal } => {
            if internal {
                command::legacy::mcp_internal::start(app_settings).await
            } else {
                command::legacy::mcp::start(app_settings).await
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Actions(actions::Platform { cmd }) => match cmd {
            Some(actions::Subcommands::HandleChanges {
                description,
                handler,
            }) => {
                let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
                command::legacy::actions::handle_changes(&mut ctx, out, handler, &description)
            }
            None => {
                let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
                command::legacy::actions::list_actions(&mut ctx, out, 0, 10)
            }
        },
        #[cfg(feature = "legacy")]
        Subcommands::Claude(claude::Platform { cmd }) => {
            use but_claude::hooks::OutputClaudeJson;
            match cmd {
                claude::Subcommands::PreTool => {
                    but_claude::hooks::handle_pre_tool_call(std::io::stdin().lock())
                        .output_claude_json()
                        .emit_metrics(metrics_ctx)
                }
                claude::Subcommands::PostTool => {
                    but_claude::hooks::handle_post_tool_call(std::io::stdin().lock())
                        .output_claude_json()
                        .emit_metrics(metrics_ctx)
                }
                claude::Subcommands::Stop => {
                    but_claude::hooks::handle_stop(std::io::stdin().lock())
                        .await
                        .output_claude_json()
                        .emit_metrics(metrics_ctx)
                }
                claude::Subcommands::PermissionPromptMcp { session_id } => {
                    but_claude::mcp::start(&args.current_dir, &session_id).await
                }
                claude::Subcommands::Last { offset } => {
                    let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
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
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Cursor(cursor::Platform { cmd }) => match cmd {
            cursor::Subcommands::AfterEdit => {
                but_cursor::handle_after_edit(std::io::stdin().lock())
                    .await
                    .output_json(true)
                    .emit_metrics(metrics_ctx)
            }
            cursor::Subcommands::Stop { nightly } => {
                but_cursor::handle_stop(nightly, std::io::stdin().lock())
                    .await
                    .output_json(true)
                    .emit_metrics(metrics_ctx)
            }
        },
        #[cfg(feature = "legacy")]
        Subcommands::Pull { check } => {
            let ctx = init::init_ctx(&args, Fetch::None, out)?;
            command::legacy::pull::handle(&ctx, out, check)
                .await
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Fetch => {
            if out.for_human().is_some() {
                use std::fmt::Write;
                let mut progress = out.progress_channel();
                writeln!(
                    progress,
                    "{}",
                    "Assuming you meant to check for upstream work, running `but pull --check`"
                        .yellow()
                )?;
            }
            let ctx = init::init_ctx(&args, Fetch::None, out)?;
            command::legacy::pull::handle(&ctx, out, true)
                .await
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Worktree(worktree::Platform { cmd }) => {
            let ctx = init::init_ctx(&args, Fetch::None, out)?;
            command::legacy::worktree::handle(cmd, &ctx, out)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Status {
            show_files,
            verbose,
            refresh_prs: sync_prs,
            upstream,
            hint,
        } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::status::worktree(
                &mut ctx, out, show_files, verbose, sync_prs, upstream, hint,
            )
            .await
            .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Rub { source, target } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::rub::handle(&mut ctx, out, &source, &target)
                .context("Rubbed the wrong way.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Diff { target } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::diff::handle(&mut ctx, out, target.as_deref())
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Show { commit } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::show::show_commit(&mut ctx, out, &commit)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Mark { target, delete } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::mark::handle(&mut ctx, out, &target, delete)
                .context("Can't mark this. Taaaa-na-na-na. Can't mark this.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Unmark => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::mark::unmark(&mut ctx, out)
                .context("Can't unmark this. Taaaa-na-na-na. Can't unmark this.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Commit {
            message,
            file,
            branch,
            create,
            only,
        } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            // Read message from file if provided, otherwise use message option
            let commit_message = match file {
                Some(path) => Some(std::fs::read_to_string(&path).with_context(|| {
                    format!(
                        "Failed to read commit message from file: {}",
                        path.display()
                    )
                })?),
                None => message,
            };
            command::legacy::commit::commit(
                &mut ctx,
                out,
                commit_message.as_deref(),
                branch.as_deref(),
                only,
                create,
            )
            .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Push(push_args) => {
            let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
            command::legacy::push::handle(push_args, &mut ctx, out).emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::New { target } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::commit::insert_blank_commit(&mut ctx, out, &target)
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Reword { target, message } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::reword::reword_target(&mut ctx, out, &target, message.as_deref())
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Oplog(args::oplog::Platform { cmd }) => {
            let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
            match cmd {
                Some(args::oplog::Subcommands::List { since, snapshot }) => {
                    let filter = if snapshot {
                        Some(command::legacy::oplog::OplogFilter::Snapshot)
                    } else {
                        None
                    };
                    command::legacy::oplog::show_oplog(&mut ctx, out, since.as_deref(), filter)
                        .emit_metrics(metrics_ctx)
                }
                Some(args::oplog::Subcommands::Snapshot { message }) => {
                    command::legacy::oplog::create_snapshot(&mut ctx, out, message.as_deref())
                        .emit_metrics(metrics_ctx)
                }
                None => {
                    // Default to list when no subcommand is provided
                    command::legacy::oplog::show_oplog(&mut ctx, out, None, None)
                        .emit_metrics(metrics_ctx)
                }
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Restore { oplog_sha, force } => {
            let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
            command::legacy::oplog::restore_to_oplog(&mut ctx, out, &oplog_sha, force)
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Undo => {
            let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
            command::legacy::oplog::undo_last_operation(&mut ctx, out).emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Absorb { source } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::absorb::handle(&mut ctx, out, source.as_deref())
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Discard { id } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::discard::handle(&mut ctx, out, &id).emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Init { repo } => command::legacy::init::repo(&args.current_dir, out, repo)
            .context("Failed to initialize GitButler project.")
            .emit_metrics(metrics_ctx),
        #[cfg(feature = "legacy")]
        Subcommands::Pr(forge::pr::Platform { cmd }) => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            match cmd {
                Some(forge::pr::Subcommands::New {
                    branch,
                    skip_force_push_protection,
                    with_force,
                    run_hooks,
                    default,
                }) => command::legacy::forge::review::create_pr(
                    &mut ctx,
                    branch,
                    skip_force_push_protection,
                    with_force,
                    run_hooks,
                    default,
                    out,
                )
                .await
                .context("Failed to create PR for branch.")
                .emit_metrics(metrics_ctx),
                Some(forge::pr::Subcommands::Template { template_path }) => {
                    command::legacy::forge::review::set_review_template(
                        &mut ctx,
                        template_path,
                        out,
                    )
                    .context("Failed to set PR template.")
                    .emit_metrics(metrics_ctx)
                }
                None => {
                    // Default to `pr new` when no subcommand is provided
                    command::legacy::forge::review::create_pr(
                        &mut ctx, None, false, true, true, false, out,
                    )
                    .await
                    .context("Failed to create PR for branch.")
                    .emit_metrics(metrics_ctx)
                }
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::RefreshRemoteData { fetch, pr: prs, ci } => {
            let mut ctx = init::init_ctx(&args, Fetch::None, out)?;
            command::legacy::refresh::handle(&mut ctx, out, fetch, prs, ci)
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Resolve { cmd, commit } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::resolve::handle(&mut ctx, out, cmd, commit)
                .context("Failed to handle conflict resolution.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Uncommit { source } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::rub::handle_uncommit(&mut ctx, out, &source)
                .context("Failed to uncommit.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Amend { file, commit } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::rub::handle_amend(&mut ctx, out, &file, &commit)
                .context("Failed to amend.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Stage {
            file_or_hunk,
            branch,
        } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::rub::handle_stage(&mut ctx, out, &file_or_hunk, &branch)
                .context("Failed to stage.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Unstage {
            file_or_hunk,
            branch,
        } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::rub::handle_unstage(&mut ctx, out, &file_or_hunk, branch.as_deref())
                .context("Failed to unstage.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Squash {
            commit1,
            commit2,
            drop_message,
        } => {
            let mut ctx = init::init_ctx(&args, Fetch::Auto, out)?;
            command::legacy::rub::handle_squash(&mut ctx, out, &commit1, &commit2, drop_message)
                .context("Failed to squash commits.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
    }
}

#[cfg(feature = "legacy")]
mod legacy;

mod init;
mod trace;
mod utils;

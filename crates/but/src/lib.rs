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
use clap::Parser;

pub mod args;
use args::{
    Args, OutputFormat, Subcommands, actions, alias as alias_args, branch, claude, cursor, forge, metrics,
    update as update_args, worktree,
};
use but_settings::AppSettings;
use colored::Colorize;
use gix::date::time::CustomFormat;

use crate::{
    setup::{BackgroundSync, InitCtxOptions},
    utils::{OneshotMetricsContext, OutputChannel, ResultErrorExt, ResultJsonExt, ResultMetricsExt},
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
    if args_vec.iter().any(|arg| arg == "push") && args_vec.iter().any(|arg| arg == "--help" || arg == "-h") {
        let mut out = OutputChannel::new_without_pager_non_json(OutputFormat::Human);
        command::push::help::print(&mut out)?;
        return Ok(());
    }

    // Handle `but help -h` and `but help --help` to show the grouped help output
    if args_vec.iter().any(|arg| arg == "help") && args_vec.iter().any(|arg| arg == "--help" || arg == "-h") {
        let mut out = OutputChannel::new_without_pager_non_json(OutputFormat::Human);
        command::help::print_grouped(&mut out)?;
        return Ok(());
    }

    let mut args: Args = Args::parse_from(args);
    let app_settings = AppSettings::load_from_default_path_creating_without_customization()?;
    let output_format = if args.json { OutputFormat::Json } else { args.format };
    // Determine if pager should be used based on the command
    let use_pager = match args.cmd {
        #[cfg(feature = "legacy")]
        Some(Subcommands::Diff { tui, .. }) => !tui,
        #[cfg(feature = "legacy")]
        Some(Subcommands::Stage { ref file_or_hunk, .. }) => file_or_hunk.is_some(),
        _ => false,
    };
    let mut out = OutputChannel::new_with_optional_pager(output_format, use_pager);

    if args.trace > 0 {
        trace::init(args.trace)?;
    }
    let _span = tracing::info_span!("CLI").entered();

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
                let status_after = args.status_after;
                let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), &mut out)?;
                out.begin_status_after(status_after);
                let result = command::legacy::rub::handle(&mut ctx, &mut out, source, target)
                    .context("Rubbed the wrong way.")
                    .emit_metrics(OneshotMetricsContext::new_if_enabled(
                        &app_settings,
                        metrics::CommandName::Rub,
                    ));
                maybe_run_status_after(status_after, &result, &mut ctx, &mut out).await;
                result.show_root_cause_error_then_exit_without_destructors(out)
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
            // The default alias expands to "status" which provides a helpful entry point
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
        Subcommands::Metrics { command_name, props } => {
            let mut event = utils::metrics::Event::new(command_name.into());
            if let Ok(props) = utils::metrics::Props::from_json_string(&props) {
                props.update_event(&mut event);
            }
            utils::metrics::capture_event_blocking(&app_settings, event).await;
            Ok(())
        }
        Subcommands::Gui => command::gui::open(&args.current_dir).emit_metrics(metrics_ctx),
        Subcommands::Completions { shell } => {
            command::completions::generate_completions(shell).emit_metrics(metrics_ctx)
        }
        Subcommands::Update(update_args::Platform { cmd }) => {
            command::update::handle(cmd, out, &app_settings).emit_metrics(metrics_ctx)
        }
        Subcommands::Help => {
            command::help::print_grouped(out)?;
            Ok(())
        }
        Subcommands::Onboarding => command::onboarding::handle(out),
        Subcommands::EvalHook => {
            command::eval_hook::execute();
            Ok(())
        }
        Subcommands::Alias(alias_args::Platform { cmd }) => {
            let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
            match cmd {
                Some(alias_args::Subcommands::List) | None => {
                    command::alias::list(&*ctx.repo.get()?, out).emit_metrics(metrics_ctx)
                }
                Some(alias_args::Subcommands::Add { name, value, global }) => {
                    command::alias::add(&mut ctx, out, &name, &value, global).emit_metrics(metrics_ctx)
                }
                Some(alias_args::Subcommands::Remove { name, global }) => {
                    command::alias::remove(&mut ctx, out, &name, global).emit_metrics(metrics_ctx)
                }
            }
        }
        Subcommands::Config(args::config::Platform { cmd }) => {
            // Handle subcommands that don't require a repo context
            match &cmd {
                Some(args::config::Subcommands::Metrics { status }) => command::config::metrics_config(out, *status)
                    .await
                    .emit_metrics(metrics_ctx),
                Some(args::config::Subcommands::Forge { cmd: forge_cmd }) => {
                    command::config::forge_config(out, forge_cmd.clone())
                        .await
                        .emit_metrics(metrics_ctx)
                }
                _ => {
                    // Other subcommands need a repo context
                    cfg_if! {
                        if #[cfg(feature = "legacy")] {
                            let mut ctx = setup::init_ctx(&args, InitCtxOptions { background_sync: BackgroundSync::Disabled, ..Default::default() }, out)?;
                            command::config::exec(&mut ctx, out, cmd)
                                .await
                                .emit_metrics(metrics_ctx)
                        } else {
                            let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
                            command::config::exec(&mut ctx, out, cmd)
                                .await
                                .emit_metrics(metrics_ctx)
                        }
                    }
                }
            }
        }
        Subcommands::Skill(args::skill::Platform { cmd }) => {
            // Skill commands use repository context when available, but can run
            // without one. Subcommand handlers produce tailored guidance when a
            // local repository is actually required.
            let ctx = but_ctx::Context::discover(&args.current_dir);
            let mut ctx = match ctx {
                Ok(ctx) => Some(ctx),
                Err(err) if is_not_in_git_repository_error(&err) => None,
                Err(err) => return Err(err),
            };
            let result = command::skill::handle(ctx.as_mut(), out, cmd);

            // Handle user cancellation gracefully (exit 0 instead of error)
            if let Err(ref e) = result
                && e.downcast_ref::<command::skill::UserCancelled>().is_some()
            {
                return Ok(());
            }

            result.emit_metrics(metrics_ctx)
        }
        Subcommands::Branch(branch::Platform { cmd }) => {
            cfg_if! {
                if #[cfg(feature = "legacy")]  {
                    let mut ctx = setup::init_ctx(&args, InitCtxOptions { background_sync: BackgroundSync::Enabled, ..Default::default() }, out)?;
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
            Some(actions::Subcommands::HandleChanges { description, handler }) => {
                let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
                command::legacy::actions::handle_changes(&mut ctx, out, handler, &description)
            }
            None => {
                let ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
                command::legacy::actions::list_actions(&ctx, out, 0, 10)
            }
        },
        #[cfg(feature = "legacy")]
        Subcommands::Claude(claude::Platform { cmd }) => {
            use but_claude::hooks::OutputClaudeJson;
            match cmd {
                claude::Subcommands::PreTool => but_claude::hooks::handle_pre_tool_call(std::io::stdin().lock())
                    .output_claude_json()
                    .emit_metrics(metrics_ctx),
                claude::Subcommands::PostTool => but_claude::hooks::handle_post_tool_call(std::io::stdin().lock())
                    .output_claude_json()
                    .emit_metrics(metrics_ctx),
                claude::Subcommands::Stop => but_claude::hooks::handle_stop(std::io::stdin().lock())
                    .output_claude_json()
                    .emit_metrics(metrics_ctx),
                claude::Subcommands::Last { offset } => {
                    let ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
                    let message = but_claude::db::get_user_message(&ctx, Some(offset as i64))?;
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
                                    msg.created_at().format("%Y-%m-%d %H:%M:%S").to_string().cyan()
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
            cursor::Subcommands::AfterEdit => but_cursor::handle_after_edit(std::io::stdin().lock())
                .await
                .output_json(true)
                .emit_metrics(metrics_ctx),
            cursor::Subcommands::Stop { nightly } => but_cursor::handle_stop(nightly, std::io::stdin().lock())
                .await
                .output_json(true)
                .emit_metrics(metrics_ctx),
        },
        #[cfg(feature = "legacy")]
        Subcommands::Pull { check } => {
            let ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
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
                    "Assuming you meant to check for upstream work, running `but pull --check`".yellow()
                )?;
            }
            let ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::pull::handle(&ctx, out, true)
                .await
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Worktree(worktree::Platform { cmd }) => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::worktree::handle(cmd, &mut ctx, out)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Status {
            show_files,
            verbose,
            refresh_prs: sync_prs,
            upstream,
            no_hint,
        } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::status::worktree(&mut ctx, out, show_files, verbose, sync_prs, upstream, !no_hint)
                .await
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Rub { source, target } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::rub::handle(&mut ctx, out, &source, &target)
                .context("Rubbed the wrong way.")
                .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Diff { target, tui, no_tui } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            let use_tui = if tui {
                true
            } else if no_tui {
                false
            } else {
                // Check git config for but.ui.tui
                ctx.git2_repo
                    .get()
                    .ok()
                    .and_then(|repo| repo.config().ok())
                    .map(|config| command::config::get_tui_enabled(&config))
                    .unwrap_or(false)
            };
            if use_tui {
                command::legacy::diff::handle_tui(&mut ctx, target.as_deref())
                    .emit_metrics(metrics_ctx)
                    .show_root_cause_error_then_exit_without_destructors(output)
            } else {
                command::legacy::diff::handle(&mut ctx, out, target.as_deref())
                    .emit_metrics(metrics_ctx)
                    .show_root_cause_error_then_exit_without_destructors(output)
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Show { commit, verbose } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::show::show_commit(&mut ctx, out, &commit, verbose)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Mark { target, delete } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::mark::handle(&mut ctx, out, &target, delete)
                .context("Can't mark this. Taaaa-na-na-na. Can't mark this.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Unmark => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::mark::unmark(&mut ctx, out)
                .context("Can't unmark this. Taaaa-na-na-na. Can't unmark this.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Commit(commit_args) => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);

            let result = match commit_args.cmd {
                Some(crate::args::commit::Subcommands::Empty {
                    ref target,
                    ref before,
                    ref after,
                }) => {
                    use but_rebase::graph_rebase::mutate::InsertSide;

                    // Validate that no regular commit options are specified with the empty subcommand
                    if commit_args.message.is_some() {
                        anyhow::bail!(
                            "--message cannot be used with 'commit empty'. Empty commits have no message by default."
                        );
                    }
                    if commit_args.message_file.is_some() {
                        anyhow::bail!(
                            "--message-file cannot be used with 'commit empty'. Empty commits have no message by default."
                        );
                    }
                    if commit_args.branch.is_some() {
                        anyhow::bail!(
                            "branch argument cannot be used with 'commit empty'. Use the target positional argument or --before/--after flags."
                        );
                    }
                    if commit_args.create {
                        anyhow::bail!("--create cannot be used with 'commit empty'.");
                    }
                    if commit_args.only {
                        anyhow::bail!("--only cannot be used with 'commit empty'.");
                    }
                    if commit_args.no_hooks {
                        anyhow::bail!("--no-hooks cannot be used with 'commit empty'.");
                    }
                    if commit_args.ai.is_some() {
                        anyhow::bail!("--ai cannot be used with 'commit empty'.");
                    }
                    // Note: --paths with commit empty is rejected by clap at parse time
                    // because --paths is not a flag on the empty subcommand

                    // Handle the `but commit empty` subcommand
                    // Determine target and insert side based on which argument was provided
                    // Note: InsertSide::Above inserts as a child (after in time),
                    // InsertSide::Below inserts as a parent (before in time)

                    // Compute the target string and insert side, possibly storing a String
                    // we own if we need to create the default branch name
                    enum TargetSpec<'a> {
                        Borrowed(&'a str, InsertSide),
                        Owned(String, InsertSide),
                    }

                    let target_spec = if let Some(t) = before {
                        TargetSpec::Borrowed(t.as_str(), InsertSide::Below)
                    } else if let Some(t) = after {
                        TargetSpec::Borrowed(t.as_str(), InsertSide::Above)
                    } else if let Some(t) = target {
                        // Default to --before behavior when using positional argument
                        TargetSpec::Borrowed(t.as_str(), InsertSide::Below)
                    } else {
                        // No arguments provided - default to inserting at top of first branch
                        use but_api::legacy::workspace;

                        let stack_entries = workspace::stacks(&ctx, None)?;
                        let stacks: Vec<(but_core::ref_metadata::StackId, but_workspace::ui::StackDetails)> =
                            stack_entries
                                .iter()
                                .filter_map(|s| {
                                    s.id.and_then(|id| {
                                        workspace::stack_details(&ctx, Some(id))
                                            .ok()
                                            .map(|details| (id, details))
                                    })
                                })
                                .collect();

                        // Find the first stack with branches and convert BString to String
                        let branch_name = stacks
                            .iter()
                            .find_map(|(_, stack_details)| {
                                stack_details.branch_details.first().map(|b| b.name.to_string())
                            })
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "No branches found. Create a branch first or specify a target explicitly."
                                )
                            })?;

                        TargetSpec::Owned(branch_name, InsertSide::Above)
                    };

                    let (target_str, insert_side) = match &target_spec {
                        TargetSpec::Borrowed(s, side) => (*s, *side),
                        TargetSpec::Owned(s, side) => (s.as_str(), *side),
                    };

                    command::legacy::commit::insert_blank_commit(&mut ctx, out, target_str, insert_side)
                        .emit_metrics(metrics_ctx)
                }
                None => {
                    // Handle the regular `but commit` command

                    // In JSON mode, require either -m, --message-file, or --ai to be specified
                    if args.json
                        && commit_args.message.is_none()
                        && commit_args.message_file.is_none()
                        && commit_args.ai.is_none()
                    {
                        anyhow::bail!(
                            "In JSON mode, either --message (-m), --message-file, or --ai (-i) must be specified"
                        );
                    }

                    // Read message from file if provided, otherwise use message option
                    let commit_message =
                        match &commit_args.message_file {
                            Some(path) => Some(std::fs::read_to_string(path).with_context(|| {
                                format!("Failed to read commit message from file: {}", path.display())
                            })?),
                            None => commit_args.message.clone(),
                        };
                    command::legacy::commit::commit(
                        &mut ctx,
                        out,
                        commit_message.as_deref(),
                        commit_args.branch.as_deref(),
                        &commit_args.changes,
                        commit_args.only,
                        commit_args.create,
                        commit_args.no_hooks,
                        commit_args.ai.clone(),
                    )
                    .emit_metrics(metrics_ctx)
                }
            };

            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result
        }
        #[cfg(feature = "legacy")]
        Subcommands::Push(push_args) => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::push::handle(push_args, &mut ctx, out).emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Reword {
            target,
            message,
            format,
        } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::reword::reword_target(&mut ctx, out, &target, message.as_deref(), format)
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Oplog(args::oplog::Platform { cmd }) => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
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
                    command::legacy::oplog::create_snapshot(&mut ctx, out, message.as_deref()).emit_metrics(metrics_ctx)
                }
                Some(args::oplog::Subcommands::Restore { oplog_sha, force }) => {
                    command::legacy::oplog::restore_to_oplog(&mut ctx, out, &oplog_sha, force).emit_metrics(metrics_ctx)
                }
                None => {
                    // Default to list when no subcommand is provided
                    command::legacy::oplog::show_oplog(&mut ctx, out, None, None).emit_metrics(metrics_ctx)
                }
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Undo => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::oplog::undo_last_operation(&mut ctx, out).emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Absorb { source, dry_run, new } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::absorb::handle(&mut ctx, out, source.as_deref(), dry_run, new)
                .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result
        }
        #[cfg(feature = "legacy")]
        Subcommands::Discard { id } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::discard::handle(&mut ctx, out, &id).emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Setup { init } => {
            let repo = match but_api::legacy::projects::add_project_best_effort(args.current_dir.clone())? {
                gitbutler_project::AddProjectOutcome::Added(project)
                | gitbutler_project::AddProjectOutcome::AlreadyExists(project) => gix::open(project.git_dir())?,
                _ => command::legacy::setup::find_or_initialize_repo(&args.current_dir, out, init)?,
            };
            let mut ctx = but_ctx::Context::from_repo(repo)?;
            let mut guard = ctx.exclusive_worktree_access();
            command::legacy::setup::repo(&mut ctx, &args.current_dir, out, guard.write_permission())
                .context("Failed to set up GitButler project.")
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Teardown => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    workspace_check: setup::WorkspaceCheck::Disabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::teardown::teardown(&mut ctx, out)
                .context("Failed to teardown GitButler project.")
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Pr(forge::pr::Platform { cmd }) => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            match cmd {
                Some(forge::pr::Subcommands::New {
                    branch,
                    message,
                    file,
                    skip_force_push_protection,
                    with_force,
                    run_hooks,
                    default,
                }) => {
                    // Read message content from file or inline
                    let message_content = match &file {
                        Some(path) => Some(std::fs::read_to_string(path).with_context(|| {
                            format!("Failed to read forge review message from file: {}", path.display())
                        })?),
                        None => message.clone(),
                    };
                    // Parse early to fail fast on invalid content
                    let review_message = match message_content {
                        Some(content) => Some(command::legacy::forge::review::parse_review_message(&content)?),
                        None => None,
                    };
                    // Check for non-interactive environment
                    if !out.can_prompt() {
                        if branch.is_none() {
                            anyhow::bail!("Non-interactive environment detected. Please specify a branch.");
                        }
                        if review_message.is_none() && !default {
                            anyhow::bail!(
                                "Non-interactive environment detected. Provide one of: --message (-m), --file (-F), or --default (-t)."
                            );
                        }
                    }
                    command::legacy::forge::review::create_review(
                        &mut ctx,
                        branch,
                        skip_force_push_protection,
                        with_force,
                        run_hooks,
                        default,
                        review_message,
                        out,
                    )
                    .await
                    .context("Failed to create forge review for branch.")
                    .emit_metrics(metrics_ctx)
                }
                Some(forge::pr::Subcommands::Template { template_path }) => {
                    command::legacy::forge::review::set_review_template(&mut ctx, template_path, out)
                        .context("Failed to set forge review template.")
                        .emit_metrics(metrics_ctx)
                }
                None => {
                    // Default to `pr new` when no subcommand is provided
                    command::legacy::forge::review::create_review(&mut ctx, None, false, true, true, false, None, out)
                        .await
                        .context("Failed to create forge review for branch.")
                        .emit_metrics(metrics_ctx)
                }
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::RefreshRemoteData {
            fetch,
            pr: prs,
            ci,
            updates,
        } => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::refresh::handle(&mut ctx, out, fetch, prs, ci, updates, &app_settings)
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Resolve { cmd, commit } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::resolve::handle(&mut ctx, out, cmd, commit)
                .context("Failed to handle conflict resolution.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Uncommit { source } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::rub::handle_uncommit(&mut ctx, out, &source)
                .context("Failed to uncommit.")
                .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Amend { file, commit } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::rub::handle_amend(&mut ctx, out, &file, &commit)
                .context("Failed to amend.")
                .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Stage {
            file_or_hunk,
            branch_pos,
            branch,
        } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = if let Some(file_or_hunk) = file_or_hunk {
                // Direct mode: but stage <file_or_hunk> <branch>
                let branch = branch.or(branch_pos).ok_or_else(|| {
                    anyhow::anyhow!("Missing required argument: <branch>. Usage: but stage <file_or_hunk> <branch>")
                })?;
                command::legacy::rub::handle_stage(&mut ctx, out, &file_or_hunk, &branch)
                    .context("Failed to stage.")
                    .emit_metrics(metrics_ctx)
            } else {
                // Interactive mode: but stage [--branch <branch>]
                use std::io::IsTerminal;
                if !std::io::stdout().is_terminal() {
                    anyhow::bail!("Interactive stage requires a terminal. Use: but stage <file_or_hunk> <branch>");
                }
                command::legacy::rub::handle_stage_tui(&mut ctx, out, branch.as_deref())
                    .context("Failed to stage.")
                    .emit_metrics(metrics_ctx)
            };
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Unstage { file_or_hunk, branch } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::rub::handle_unstage(&mut ctx, out, &file_or_hunk, branch.as_deref())
                .context("Failed to unstage.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Squash {
            commits,
            drop_message,
            message,
            ai,
        } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::rub::squash::handle(
                &mut ctx,
                out,
                &commits,
                drop_message,
                message.as_deref(),
                ai.clone(),
            )
            .context("Failed to squash commits.")
            .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Merge { branch } => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::merge::handle(&mut ctx, out, &branch)
                .await
                .context("Failed to merge branch.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Move {
            source_commit,
            target,
            after,
        } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::rub::r#move::handle(&mut ctx, out, &source_commit, &target, after)
                .context("Failed to move commit.")
                .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Pick { source, target_branch } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::pick::handle(&mut ctx, out, &source, target_branch.as_deref())
                .context("Failed to pick commit.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Unapply { identifier, force } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::unapply::handle(&mut ctx, out, &identifier, force)
                .context("Failed to unapply branch.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Apply { branch_name } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::branch::apply::apply(&mut ctx, &branch_name, out)
                .context("Failed to apply branch.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
    }
}

fn is_not_in_git_repository_error(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<gix::discover::Error>(),
        Some(gix::discover::Error::Discover(
            gix::discover::upwards::Error::NoGitRepository { .. }
                | gix::discover::upwards::Error::NoGitRepositoryWithinCeiling { .. }
                | gix::discover::upwards::Error::NoGitRepositoryWithinFs { .. }
        ))
    )
}

/// If `--status-after` was requested, appends workspace status to the output.
///
/// Call `out.begin_status_after(status_after)` *before* the mutation to set up
/// JSON buffering, then call this *after* to conditionally emit the combined output.
///
/// When the mutation succeeded, runs status and combines the output.
/// When the mutation failed, the buffer is left intact — `OutputChannel::drop`
/// will flush any buffered error JSON (e.g. structured illegal_move details) to stdout.
/// Errors from the status query itself are logged to stderr but never mask
/// the mutation's success.
#[cfg(feature = "legacy")]
async fn maybe_run_status_after(
    status_after: bool,
    result: &anyhow::Result<()>,
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) {
    if !status_after {
        return;
    }
    if result.is_ok() {
        let mutation_json = out.take_json_buffer();
        run_status_after(ctx, out, mutation_json).await;
    } else {
        // Mutation failed — don't drain the buffer here. OutputChannel::drop
        // will flush any buffered JSON (e.g. structured illegal_move details)
        // to stdout, so the mutation result is never silently lost.
    }
}

/// Run workspace status output after a mutation command completes.
///
/// In human mode, prints a blank line then full status.
/// In JSON mode, combines the mutation's buffered JSON with status JSON into
/// `{"result": <mutation_output>, "status": <workspace_status>}`.
///
/// Status errors are handled gracefully: in JSON mode the mutation result is
/// always emitted (with a `"status_error"` field on failure); in human mode
/// a warning is printed to stderr.
#[cfg(feature = "legacy")]
async fn run_status_after(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    mutation_json: Option<serde_json::Value>,
) {
    if out.is_json() {
        out.start_json_buffering();
        let status_result = command::legacy::status::worktree(ctx, out, false, false, false, false, false).await;
        let status_json = out.take_json_buffer().unwrap_or(serde_json::Value::Null);

        let combined = match status_result {
            Ok(()) => serde_json::json!({
                "result": mutation_json.unwrap_or(serde_json::Value::Null),
                "status": status_json,
            }),
            Err(err) => {
                eprintln!(
                    "warning: --status-after failed: {err:#}. Run 'but status' separately to check workspace state."
                );
                serde_json::json!({
                    "result": mutation_json.unwrap_or(serde_json::Value::Null),
                    "status_error": format!("{err:#}"),
                })
            }
        };
        if let Err(err) = out.write_value(combined) {
            eprintln!("warning: failed to write --status-after output: {err}");
        }
    } else {
        if let Some(human) = out.for_human() {
            writeln!(human).ok();
        }
        if let Err(err) = command::legacy::status::worktree(ctx, out, false, false, false, false, true).await {
            eprintln!("warning: --status-after failed: {err:#}. Run 'but status' separately to check workspace state.");
        }
    }
}

#[cfg(feature = "legacy")]
mod legacy;

mod setup;
pub mod trace;
mod utils;

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
#[cfg(unix)]
use clap::CommandFactory;
use clap::Parser;

pub mod args;
use args::{
    Args, OutputFormat, Subcommands, actions, alias as alias_args, branch, forge,
    update as update_args, worktree,
};
use but_settings::AppSettings;
use gix::date::time::CustomFormat;
use theme::Paint;

#[cfg(feature = "legacy")]
use crate::command::legacy::ShowDiffInEditor;
use crate::{
    setup::{BackgroundSync, InitCtxOptions},
    utils::{OutputChannel, ResultErrorExt, ResultMetricsExt, envs},
};

mod error;
pub(crate) use error::{CliError, CliResult, bad_input};

mod id;
pub use id::{CliId, IdMap};

pub use utils::binary_path::is_executed_as_but;

mod alias;
/// A place for all command implementations.
pub(crate) mod command;
pub mod theme;
mod tui;

const CLI_DATE: CustomFormat = gix::date::time::format::ISO8601;

/// Handle `args` which must be what's passed by `std::env::args_os()`.
pub async fn handle_args(args: impl Iterator<Item = OsString>) -> Result<()> {
    let theme_preset_from_env: anyhow::Result<theme::ThemePreset> =
        if let Some(theme_name) = std::env::var_os(envs::BUT_THEME) {
            theme_name.to_string_lossy().parse()
        } else {
            Ok(theme::ThemePreset::Dark)
        };

    {
        let theme_preset = match &theme_preset_from_env {
            Ok(theme_preset) => theme_preset.clone(),
            Err(_) => {
                // ignore for now, we print a warning once the output channel has been initialized
                theme::ThemePreset::Dark
            }
        };

        // Note: Overrides in but-theme.json are hardwired to apply to the Dark theme at present.
        // This is only for internal testing at the moment so it's not worthwhile to go through the
        // motions of merging overrides with a configurable theme.
        let theme = dirs::config_dir()
            .map(|dir| dir.join("gitbutler").join("but-theme.json"))
            .filter(|p| p.exists())
            .and_then(|p| theme::load(&p).ok())
            .unwrap_or_else(|| theme::Theme::default_for(theme_preset));
        theme::init(theme);
    }

    let args: Vec<_> = args.collect();

    // Check if version is requested
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        let version = option_env!("VERSION").unwrap_or("dev");
        println!("but {version}");
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

    let mut args: Args = Args::parse_from(args);
    let output_format = if args.json {
        OutputFormat::Json
    } else {
        args.format
    };
    // Determine if pager should be used based on the command
    let use_pager = match args.cmd {
        #[cfg(feature = "legacy")]
        Some(Subcommands::Status { .. }) => true,
        #[cfg(feature = "legacy")]
        Some(Subcommands::Diff { tui, .. }) => !tui,
        #[cfg(feature = "legacy")]
        Some(Subcommands::Stage {
            ref file_or_hunk, ..
        }) => file_or_hunk.is_some(),
        _ => false,
    };
    let _tracing_appender_worker_guard = if args.trace > 0 {
        trace::init(args.trace, args.log_file.as_deref())?
    } else {
        None
    };
    let _span =
        tracing::info_span!("CLI", cmd = ?args.cmd.as_ref().map(|cmd| cmd.to_metrics_command()))
            .entered();

    let namespace = option_env!("IDENTIFIER").unwrap_or("com.gitbutler.app");
    but_secret::secret::set_application_namespace(namespace);

    let mut out = OutputChannel::new_with_optional_pager(output_format, use_pager);

    if let (Err(theme_preset_err), Some(out)) = (theme_preset_from_env, out.for_human()) {
        writeln!(
            out,
            "{}: {theme_preset_err}",
            theme::get().attention.paint("Failed to set theme")
        )?;
    }

    if let Some(Subcommands::AgentLog { .. }) = &args.cmd {
        let Some(Subcommands::AgentLog { cmd }) = args.cmd.take() else {
            unreachable!("agentlog command was checked above")
        };
        return run_agentlog_command(&args.current_dir, cmd, &mut out);
    }
    let app_settings = AppSettings::load_from_default_path_creating_without_customization()?;

    let result = match args.cmd.take() {
        #[cfg(unix)]
        Some(Subcommands::External(extra)) => {
            command::external::dispatch(&args.current_dir, &extra)
        }
        None => {
            // No arguments means run the default alias
            // The default alias expands to "status" which provides a helpful entry point
            let default_args = vec![OsString::from("but"), OsString::from("default")];
            let expanded = alias::expand_aliases(default_args)?;
            let mut default_alias_args: Args = clap::Parser::parse_from(expanded);

            // Preserve globals from the default alias, while letting explicit user globals
            // take precedence (e.g. `but -C <dir>` without a subcommand).
            if args.trace > 0 {
                default_alias_args.trace = args.trace;
            }
            if args.current_dir != std::path::Path::new(".") {
                default_alias_args.current_dir = args.current_dir.clone();
            }
            if !matches!(args.format, OutputFormat::Human) {
                default_alias_args.format = args.format;
            }
            if args.json {
                default_alias_args.json = true;
            }
            if args.status_after {
                default_alias_args.status_after = true;
            }

            match default_alias_args.cmd.take() {
                Some(cmd) => match_subcommand(cmd, default_alias_args, app_settings, out).await,
                None => {
                    // Fallback to help if default alias somehow doesn't resolve
                    command::help::print_grouped(&mut out)?;
                    Ok(())
                }
            }
        }
        Some(cmd) => match_subcommand(cmd, args, app_settings, out).await,
    };

    match result {
        Err(CliError::Internal(err)) => Err(err),
        Err(CliError::BadInput(bad_input)) => print_and_exit_non_zero(bad_input),
        #[cfg(unix)]
        Err(CliError::ExternalCommandNotFound(command_name)) => {
            // We reparse without external subcommands allowed, which _should_ result in a proper
            // clap error, including suggestions for "near matches". This gives richer error
            // information than the plain ExternalCommandNotFound error.
            let cmd = Args::command();
            let argv = [OsString::from(cmd.get_name()), command_name.clone()];

            // This should fail to parse, print a nicely formatted Clap error and exit on its own.
            let _ = cmd
                .external_subcommand_value_parser(None)
                .allow_external_subcommands(false)
                .get_matches_from(argv);

            // If for some reason we succeeded to parse now, we'll print the original error.
            // This shouldn't happen in practice but logically it could.
            print_and_exit_non_zero(CliError::ExternalCommandNotFound(command_name))
        }
        Ok(()) => Ok(()),
    }
}

fn print_and_exit_non_zero<T: std::fmt::Display>(err: T) -> ! {
    use std::io::Write;
    // We swallow this error, there is nothing more to do at this point
    let _ = write!(std::io::stderr(), "{err}");
    std::process::exit(1)
}

async fn match_subcommand(
    cmd: Subcommands,
    args: Args,
    app_settings: AppSettings,
    mut output: OutputChannel,
) -> CliResult<()> {
    let out = &mut output;

    let cmd = match cmd {
        Subcommands::AgentLog { cmd } => {
            return Ok(run_agentlog_command(&args.current_dir, cmd, out)?);
        }
        cmd => cmd,
    };

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
        Subcommands::Gui { path } => {
            let path = path
                .as_ref()
                .map(|path| args.current_dir.join(path))
                .unwrap_or_else(|| args.current_dir.clone());
            command::gui::open(&path)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        Subcommands::Completions { shell } => command::completions::generate_completions(shell)
            .emit_metrics(metrics_ctx)
            .map_err(CliError::from),
        Subcommands::Update(update_args::Platform { cmd }) => {
            command::update::handle(cmd, out, &app_settings)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        Subcommands::Help => {
            command::help::print_grouped(out)?;
            Ok(())
        }
        Subcommands::Onboarding => command::onboarding::handle(out).map_err(CliError::from),
        Subcommands::EvalHook => {
            command::eval_hook::execute();
            Ok(())
        }
        Subcommands::Alias(alias_args::Platform { cmd }) => {
            let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
            match cmd {
                Some(alias_args::Subcommands::List) | None => {
                    command::alias::list(&*ctx.repo.get()?, out)
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                Some(alias_args::Subcommands::Add {
                    name,
                    value,
                    global,
                }) => command::alias::add(&mut ctx, out, &name, &value, global.into())
                    .emit_metrics(metrics_ctx)
                    .map_err(CliError::from),
                Some(alias_args::Subcommands::Remove { name, global }) => {
                    command::alias::remove(&mut ctx, out, &name, global.into())
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
            }
        }
        Subcommands::Config(args::config::Platform { cmd }) => {
            // Handle subcommands that don't require a repo context
            match &cmd {
                Some(args::config::Subcommands::Metrics { status }) => {
                    command::config::metrics_config(out, *status)
                        .await
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                Some(args::config::Subcommands::Forge { cmd: forge_cmd }) => {
                    command::config::forge_config(out, forge_cmd.clone())
                        .await
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                Some(args::config::Subcommands::Ai {
                    cmd: ai_cmd,
                    local,
                    global,
                }) if !local => command::config::ai_config(out, ai_cmd.clone(), *local, *global)
                    .emit_metrics(metrics_ctx)
                    .map_err(CliError::from),
                _ => {
                    // Other subcommands need a repo context
                    cfg_if! {
                        if #[cfg(feature = "legacy")] {
                            let mut ctx = setup::init_ctx(&args, InitCtxOptions { background_sync: BackgroundSync::Disabled, ..Default::default() }, out)?;
                            command::config::exec(&mut ctx, out, cmd)
                                .await
                                .emit_metrics(metrics_ctx).map_err(CliError::from)
                        } else {
                            let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
                            command::config::exec(&mut ctx, out, cmd)
                                .await
                                .emit_metrics(metrics_ctx).map_err(CliError::from)
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
                Err(err) => return Err(CliError::Internal(err)),
            };
            let result = command::skill::handle(ctx.as_mut(), out, cmd);

            // Handle user cancellation gracefully (exit 0 instead of error)
            if let Err(ref e) = result
                && e.downcast_ref::<command::skill::UserCancelled>().is_some()
            {
                return Ok(());
            }

            result.emit_metrics(metrics_ctx).map_err(CliError::from)
        }
        Subcommands::Branch(branch::Platform { cmd }) => {
            let result = match cmd {
                #[cfg(not(feature = "legacy"))]
                None => todo!("implement list and call recursively"),
                #[cfg(feature = "legacy")]
                None => {
                    let mut ctx = setup::init_ctx(
                        &args,
                        InitCtxOptions {
                            background_sync: BackgroundSync::Enabled { silent: false },
                            ..Default::default()
                        },
                        out,
                    )?;
                    command::legacy::branch::handle_no_subcommand(&mut ctx, out)
                        .map_err(CliError::from)
                }
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::List {
                    filter,
                    local,
                    remote,
                    all,
                    no_ahead,
                    review,
                    no_check,
                    empty,
                }) => {
                    let mut ctx = setup::init_ctx(
                        &args,
                        InitCtxOptions {
                            background_sync: BackgroundSync::Enabled { silent: false },
                            ..Default::default()
                        },
                        out,
                    )?;
                    command::legacy::branch::list_branches(
                        &mut ctx, out, filter, local, remote, all, no_ahead, review, no_check,
                        empty,
                    )
                    .map_err(CliError::from)
                }
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::Show {
                    branch,
                    review,
                    files,
                    ai,
                    check,
                }) => {
                    let mut ctx = setup::init_ctx(
                        &args,
                        InitCtxOptions {
                            background_sync: BackgroundSync::Enabled { silent: false },
                            ..Default::default()
                        },
                        out,
                    )?;
                    command::legacy::branch::show_branches(
                        &mut ctx, out, branch, review, files, ai, check,
                    )
                }
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::New {
                    branch_name,
                    anchor,
                }) => {
                    let mut ctx = setup::init_ctx(
                        &args,
                        InitCtxOptions {
                            background_sync: BackgroundSync::Enabled { silent: false },
                            ..Default::default()
                        },
                        out,
                    )?;
                    command::legacy::branch::new(&mut ctx, out, branch_name, anchor)
                }
                #[cfg(feature = "legacy")]
                Some(branch::Subcommands::Delete { branch_name }) => {
                    let mut ctx = setup::init_ctx(
                        &args,
                        InitCtxOptions {
                            background_sync: BackgroundSync::Enabled { silent: false },
                            ..Default::default()
                        },
                        out,
                    )?;
                    command::legacy::branch::delete(&mut ctx, out, branch_name)
                }
                #[cfg(not(feature = "legacy"))]
                Some(branch::Subcommands::Apply { branch_name }) => {
                    let ctx = but_ctx::Context::discover(&args.current_dir)?;
                    command::branch::apply(ctx, &branch_name, out).map_err(CliError::from)
                }
                Some(branch::Subcommands::Move { .. }) => Err(bad_input(
                    "`but branch move` has been removed. Use `but move` instead.",
                )
                .into()),
            };
            result.emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Mcp => command::legacy::mcp::start(app_settings)
            .await
            .map_err(CliError::from),
        #[cfg(feature = "legacy")]
        Subcommands::Actions(actions::Platform { cmd }) => match cmd {
            Some(actions::Subcommands::HandleChanges {
                description,
                handler,
            }) => {
                let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
                command::legacy::actions::handle_changes(&mut ctx, out, handler, &description)
                    .map_err(CliError::from)
            }
            None => {
                let ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
                command::legacy::actions::list_actions(&ctx, out, 0, 10).map_err(CliError::from)
            }
        },
        #[cfg(feature = "legacy")]
        Subcommands::Pull { check } => {
            let ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::pull::handle(&ctx, out, check)
                .await
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Fetch => {
            use std::fmt::Write;
            let mut progress = out.progress_channel();
            writeln!(
                progress,
                "{}",
                theme::get().attention.paint(
                    "Assuming you meant to check for upstream work, running `but pull --check`"
                )
            )?;
            let ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::pull::handle(&ctx, out, true)
                .await
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Clean {
            dry_run,
            pull,
            include_upstream,
        } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            if pull {
                use std::fmt::Write;
                let mut progress = out.progress_channel();
                writeln!(progress, "Pulling latest...")?;
                let mut pull_out =
                    OutputChannel::new_with_optional_pager(OutputFormat::None, false);
                command::legacy::pull::handle(&ctx, &mut pull_out, false).await?;
                writeln!(progress, "Pull complete.")?;
            }
            out.begin_status_after(status_after);
            let result = command::legacy::clean::handle(
                &mut ctx,
                out,
                command::legacy::clean::CleanOptions {
                    dry_run,
                    include_upstream,
                },
            )
            .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.map_err(CliError::from)
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
            use crate::command::legacy::status::FilesStatusFlag;
            use crate::command::legacy::status::StatusFlags;

            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            let show_files = if show_files {
                FilesStatusFlag::All
            } else {
                FilesStatusFlag::None
            };
            let flags = StatusFlags {
                show_files,
                verbose,
                refresh_prs: sync_prs,
                show_upstream: upstream,
                hint: !no_hint,
            };
            command::legacy::status::worktree(
                &mut ctx,
                out,
                flags,
                command::legacy::status::StatusRenderMode::Oneshot,
            )
            .await
            .emit_metrics(metrics_ctx)
            .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Tui {
            #[cfg(feature = "tui-profiling")]
            debug,
            #[cfg(feature = "tui-profiling")]
            quit_after,
            #[cfg(feature = "tui-profiling")]
            headless,
            #[cfg(feature = "tui-profiling")]
            skip_status_after,
            #[cfg(feature = "tui-profiling")]
            diff,
            #[cfg(feature = "tui-profiling")]
            select_commit,
            #[cfg(feature = "tui-profiling")]
            quit_after_rendering_full_diff,
        } => {
            use crate::command::legacy::status::{StatusFlags, StatusRenderMode, TuiLaunchOptions};

            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: true },
                    ..Default::default()
                },
                out,
            )?;
            #[cfg(feature = "tui-profiling")]
            let _options = TuiLaunchOptions {
                debug,
                quit_after,
                headless,
                skip_status_after,
                show_diff: if quit_after_rendering_full_diff {
                    true
                } else {
                    diff
                },
                select_commit,
                quit_after_rendering_full_diff,
            };
            #[cfg(not(feature = "tui-profiling"))]
            let _options = TuiLaunchOptions::default();
            command::legacy::status::worktree(
                &mut ctx,
                out,
                StatusFlags::for_tui(),
                StatusRenderMode::Tui(_options),
            )
            .await
            .emit_metrics(metrics_ctx)
            .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Rub { source, target } => {
            use but_workspace::commit::squash_commits::MessageCombinationStrategy;

            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::rub::handle(
                &mut ctx,
                out,
                &source,
                &target,
                MessageCombinationStrategy::KeepBoth,
            )
            .context("Rubbed the wrong way.")
            .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Diff {
            target,
            tui,
            no_tui,
        } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
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
                ctx.repo
                    .get()
                    .ok()
                    .map(|repo| command::config::get_tui_enabled(&repo.config_snapshot()))
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
        Subcommands::Edit { file } => {
            let path = args.current_dir.join(&file);
            tui::editor::edit_file(&path)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Show { commit, verbose } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
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
                    background_sync: BackgroundSync::Enabled { silent: false },
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
            let ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::mark::unmark(&ctx, out)
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
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);

            let result = match commit_args.cmd {
                Some(crate::args::commit::Subcommands::Empty {
                    target,
                    before,
                    after,
                }) => {
                    // Validate that no regular commit options are specified with the empty subcommand
                    if commit_args.message.is_some() {
                        return Err(bad_input(
                            "--message cannot be used with 'commit empty'. Empty commits have no message by default."
                        ).into());
                    }
                    if commit_args.message_file.is_some() {
                        return Err(bad_input(
                            "--message-file cannot be used with 'commit empty'. Empty commits have no message by default."
                        ).into());
                    }
                    if commit_args.branch.is_some() {
                        return Err(bad_input(
                            "branch argument cannot be used with 'commit empty'. Use the target positional argument or --before/--after flags."
                        ).into());
                    }
                    if commit_args.create {
                        return Err(
                            bad_input("--create cannot be used with 'commit empty'.").into()
                        );
                    }
                    if commit_args.only {
                        return Err(bad_input("--only cannot be used with 'commit empty'.").into());
                    }
                    if commit_args.all {
                        return Err(bad_input("--all cannot be used with 'commit empty'.").into());
                    }
                    if commit_args.no_hooks {
                        return Err(
                            bad_input("--no-hooks cannot be used with 'commit empty'.").into()
                        );
                    }
                    if commit_args.ai.is_some() {
                        return Err(bad_input("--ai cannot be used with 'commit empty'.").into());
                    }
                    if commit_args.diff {
                        return Err(bad_input("--diff cannot be used with 'commit empty'.").into());
                    }
                    if commit_args.no_diff {
                        return Err(
                            bad_input("--no-diff cannot be used with 'commit empty'.").into()
                        );
                    }
                    // Note: --paths with commit empty is rejected by clap at parse time
                    // because --paths is not a flag on the empty subcommand

                    command::legacy::commit::insert_blank_commit(
                        &mut ctx, out, target, before, after,
                    )
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
                        return Err(bad_input(
                            "In JSON mode, either --message (-m), --message-file, or --ai (-i) must be specified"
                        ).into());
                    }

                    // Read message from file if provided, otherwise use message option
                    let commit_message = match &commit_args.message_file {
                        Some(path) => Some(std::fs::read_to_string(path).with_context(|| {
                            format!(
                                "Failed to read commit message from file: {}",
                                path.display()
                            )
                        })?),
                        None => commit_args.message.clone(),
                    };
                    command::legacy::commit::commit(
                        &mut ctx,
                        out,
                        commit_message.as_deref(),
                        commit_args.branch,
                        &commit_args.changes,
                        commit_args.only,
                        commit_args.all,
                        commit_args.create,
                        commit_args.no_hooks,
                        commit_args.ai.clone(),
                        ShowDiffInEditor::from_args(commit_args.diff, commit_args.no_diff)
                            .unwrap_or(ShowDiffInEditor::Unspecified),
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
            command::legacy::push::handle(push_args, &mut ctx, out)
                .await
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Reword {
            target,
            message,
            format,
            diff,
            no_diff,
        } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::reword::reword_target(
                &mut ctx,
                out,
                target,
                message.as_deref(),
                format,
                // clap's `conflicts_with` should prevent this being `None` but better safe than
                // sorry
                ShowDiffInEditor::from_args(diff, no_diff).unwrap_or(ShowDiffInEditor::Unspecified),
            )
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
                        .map_err(CliError::from)
                }
                Some(args::oplog::Subcommands::Snapshot { message }) => {
                    command::legacy::oplog::create_snapshot(&mut ctx, out, message.as_deref())
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                Some(args::oplog::Subcommands::Restore { oplog_sha }) => {
                    command::legacy::oplog::restore_to_oplog(&mut ctx, out, &oplog_sha)
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                None => {
                    // Default to list when no subcommand is provided
                    command::legacy::oplog::show_oplog(&mut ctx, out, None, None)
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Undo => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::oplog::handle_undo(&mut ctx, out)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Redo => {
            let mut ctx = setup::init_ctx(&args, InitCtxOptions::default(), out)?;
            command::legacy::oplog::handle_redo(&mut ctx, out)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Absorb { source, dry_run } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::absorb::handle(&mut ctx, out, source.as_deref(), dry_run)
                .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Discard { id } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::discard::handle(&mut ctx, out, &id)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Setup { init } => {
            let repo =
                match but_api::legacy::projects::add_project_best_effort(args.current_dir.clone())?
                {
                    gitbutler_project::AddProjectOutcome::Added(project)
                    | gitbutler_project::AddProjectOutcome::AlreadyExists(project) => {
                        gix::open(project.git_dir())?
                    }
                    _ => command::legacy::setup::find_or_initialize_repo(
                        &args.current_dir,
                        out,
                        init,
                    )?,
                };
            let mut ctx = but_ctx::Context::from_repo(repo)?;
            let mut guard = ctx.exclusive_worktree_access();
            command::legacy::setup::repo(&mut ctx, &args.current_dir, out, guard.write_permission())
                .context("Failed to set up GitButler project.")
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Teardown { checkout_to } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    workspace_check: setup::WorkspaceCheck::Disabled,
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::teardown::teardown(&mut ctx, checkout_to, out)
                .map_err(|err| err.context("Failed to teardown GitButler project."))
                .emit_metrics(metrics_ctx)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Pr(forge::pr::Platform {
            cmd,
            draft: top_level_draft,
        }) => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
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
                    no_hooks,
                    default,
                    draft,
                }) => {
                    let draft = top_level_draft || draft;
                    // Read message content from file or inline
                    let message_content = match &file {
                        Some(path) => Some(std::fs::read_to_string(path).with_context(|| {
                            format!(
                                "Failed to read forge review message from file: {}",
                                path.display()
                            )
                        })?),
                        None => message.clone(),
                    };
                    // Parse early to fail fast on invalid content
                    let review_message = match message_content {
                        Some(content) => Some(
                            command::legacy::forge::review::parse_review_message(&content)?,
                        ),
                        None => None,
                    };
                    // Check for non-interactive environment
                    if !out.can_prompt() {
                        if branch.is_none() {
                            return Err(bad_input(
                                "Non-interactive environment detected. Please specify a branch.",
                            )
                            .into());
                        }
                        if review_message.is_none() && !default {
                            return Err(bad_input(
                                "Non-interactive environment detected. Provide one of: --message (-m), --file (-F), or --default (-t)."
                            ).into());
                        }
                    }
                    command::legacy::forge::review::create_review(
                        &mut ctx,
                        branch,
                        skip_force_push_protection,
                        with_force,
                        !no_hooks,
                        default,
                        draft,
                        review_message,
                        out,
                    )
                    .await
                    .context("Failed to create forge review for branch.")
                    .emit_metrics(metrics_ctx)
                    .map_err(CliError::from)
                }
                Some(forge::pr::Subcommands::Template { template_path }) => {
                    command::legacy::forge::review::set_review_template(
                        &mut ctx,
                        template_path,
                        out,
                    )
                    .context("Failed to set forge review template.")
                    .emit_metrics(metrics_ctx)
                    .map_err(CliError::from)
                }
                Some(forge::pr::Subcommands::AutoMerge { selector, off }) => {
                    command::legacy::forge::review::enable_auto_merge(&mut ctx, selector, off, out)
                        .await
                        .context("Failed to set the auto-merge state.")
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                Some(forge::pr::Subcommands::SetDraft { selector }) => {
                    command::legacy::forge::review::set_draftiness(&mut ctx, selector, true, out)
                        .await
                        .context("Failed to set reviews as draft.")
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                Some(forge::pr::Subcommands::SetReady { selector }) => {
                    command::legacy::forge::review::set_draftiness(&mut ctx, selector, false, out)
                        .await
                        .context("Failed to set reviews as ready-for-review.")
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                None => {
                    // Default to `pr new` when no subcommand is provided
                    command::legacy::forge::review::create_review(
                        &mut ctx,
                        None,
                        false,
                        true,
                        true,
                        false,
                        top_level_draft,
                        None,
                        out,
                    )
                    .await
                    .context("Failed to create forge review for branch.")
                    .emit_metrics(metrics_ctx)
                    .map_err(CliError::from)
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
                .map_err(CliError::from)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Resolve { cmd, commit } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
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
        Subcommands::Uncommit { source, discard } => {
            let status_after = args.status_after;
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = command::legacy::rub::handle_uncommit(&mut ctx, out, &source, discard)
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
                    background_sync: BackgroundSync::Enabled { silent: false },
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
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            out.begin_status_after(status_after);
            let result = if let Some(file_or_hunk) = file_or_hunk.as_deref() {
                // Direct mode: but stage <file_or_hunk> <branch>
                let branch = branch.as_deref().or(branch_pos.as_deref()).ok_or_else(|| {
                    bad_input("Missing required argument: <branch>. Usage: but stage <file_or_hunk> <branch>")
                        .arg_name("<BRANCH>")
                })?;
                command::legacy::rub::handle_stage(&mut ctx, out, file_or_hunk, branch)
                    .context("Failed to stage.")
                    .emit_metrics(metrics_ctx)
            } else {
                // Interactive mode: but stage [--branch <branch>]
                use std::io::IsTerminal;
                if !std::io::stdout().is_terminal() {
                    return Err(bad_input(
                        "Interactive stage requires a terminal. Use: but stage <file_or_hunk> <branch>"
                    ).into());
                }
                command::legacy::rub::handle_stage_tui(&mut ctx, out, branch.as_deref())
                    .context("Failed to stage.")
                    .emit_metrics(metrics_ctx)
            };
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.map_err(command::legacy::rub::stage_cli_error)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Unstage {
            file_or_hunk,
            branch,
        } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
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
                    background_sync: BackgroundSync::Enabled { silent: false },
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
        Subcommands::Move {
            source,
            target,
            after,
        } => {
            let status_after = args.status_after;
            let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
            out.begin_status_after(status_after);
            let result = command::r#move::handle(&mut ctx, out, &source, &target, after)
                .emit_metrics(metrics_ctx);
            maybe_run_status_after(status_after, &result, &mut ctx, out).await;
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Pick {
            source,
            target_branch,
        } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
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
        Subcommands::Unapply { identifier } => {
            let mut ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            command::legacy::unapply::handle(&mut ctx, out, &identifier)
                .context("Failed to unapply branch.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Apply { branch_name } => {
            let ctx = setup::init_ctx(
                &args,
                InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
                out,
            )?;
            let branch_name = resolve_legacy_top_level_apply_branch_name(&ctx, &branch_name)?;
            command::branch::apply(ctx, &branch_name, out)
                .context("Failed to apply branch.")
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        Subcommands::AgentLog { .. } => {
            unreachable!("agentlog command is handled before metrics setup")
        }
        #[cfg(unix)]
        Subcommands::External(_) => {
            unreachable!("external commands are delegated before reaching match_subcommand")
        }
    }
}

fn run_agentlog_command(
    current_dir: &std::path::Path,
    mut cmd: but_agentlog::Command,
    out: &mut OutputChannel,
) -> Result<()> {
    let quiet = matches!(cmd, but_agentlog::Command::Hook { .. });
    match &mut cmd {
        but_agentlog::Command::Hook { agent, .. } if agent.is_none() => {
            use utils::detect_agent::Agent as DetectedAgent;

            *agent = match utils::detect_agent::detect() {
                Some(DetectedAgent::Codex) => Some(but_agentlog::Agent::Codex),
                Some(DetectedAgent::ClaudeCode | DetectedAgent::ClaudeCodeCowork) => {
                    Some(but_agentlog::Agent::Claude)
                }
                _ => None,
            };
        }
        _ => {}
    }

    let report = but_agentlog::run_from_dir(current_dir, cmd)?;
    if quiet {
        return Ok(());
    }
    if let Some(writer) = out.for_human_or_shell() {
        writeln!(writer, "{report}")?;
    } else if let Some(json_out) = out.for_json() {
        json_out.write_value(&report)?;
    }
    Ok(())
}

/// Resolve a legacy top-level `but apply` branch name to the narrowest directly applicable ref.
///
/// This preserves exact-name behavior while restoring the removed alias that lets a bare branch
/// name map to a unique remote-tracking branch. When multiple remotes provide the same branch
/// identity, the original input is preserved so the shared apply command keeps its current error.
#[cfg(feature = "legacy")]
fn resolve_legacy_top_level_apply_branch_name(
    ctx: &but_ctx::Context,
    branch_name: &str,
) -> Result<String> {
    let repo = ctx.repo.get()?;
    if repo.try_find_reference(branch_name)?.is_some() {
        return Ok(branch_name.to_owned());
    }

    let mut remote_matches = repo
        .remote_names()
        .iter()
        .filter_map(|remote_name| {
            let full_name = format!("refs/remotes/{remote_name}/{branch_name}");
            repo.try_find_reference(&full_name)
                .transpose()
                .map(|reference| reference.map(|_| full_name))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if remote_matches.len() == 1 {
        return Ok(remote_matches
            .pop()
            .expect("exactly one remote match exists"));
    }

    Ok(branch_name.to_owned())
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
async fn maybe_run_status_after<T, E>(
    status_after: bool,
    result: &Result<T, E>,
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

/// Ignore `--status-after` in non-legacy builds until a non-legacy status command exists.
#[cfg(not(feature = "legacy"))]
async fn maybe_run_status_after(
    _status_after: bool,
    _result: &anyhow::Result<()>,
    _ctx: &mut but_ctx::Context,
    _out: &mut OutputChannel,
) {
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
    use crate::command::legacy::status::StatusFlags;

    if out.is_json() {
        out.start_json_buffering();
        let status_result = command::legacy::status::worktree(
            ctx,
            out,
            StatusFlags::all_false(),
            command::legacy::status::StatusRenderMode::Oneshot,
        )
        .await;
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
        if let Err(err) = command::legacy::status::worktree(
            ctx,
            out,
            StatusFlags {
                show_files: crate::command::legacy::status::FilesStatusFlag::All,
                verbose: true,
                hint: true,
                ..StatusFlags::all_false()
            },
            command::legacy::status::StatusRenderMode::Oneshot,
        )
        .await
        {
            eprintln!(
                "warning: --status-after failed: {err:#}. Run 'but status' separately to check workspace state."
            );
        }
    }
}

#[cfg(feature = "legacy")]
mod legacy;

mod setup;
pub mod trace;
mod utils;

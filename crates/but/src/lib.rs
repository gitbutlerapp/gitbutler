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

use std::{
    ffi::{OsStr, OsString},
    io::IsTerminal as _,
};

use anyhow::{Context as _, Result};
use but_api::WorkspaceState;
use cfg_if::cfg_if;
use clap::{CommandFactory, FromArgMatches as _, Parser as _};

pub mod args;
use args::{
    Args, OutputFormat, Subcommands, actions, agent, alias as alias_args, branch, forge,
    update as update_args, worktree,
};
use but_settings::AppSettings;
use gix::date::time::CustomFormat;
use theme::Paint;

#[cfg(feature = "legacy")]
use crate::command::legacy::ShowDiffInEditor;
use crate::{
    setup::{BackgroundSync, InitCtxOptions, TargetRequirement},
    utils::{OutputChannel, ResultErrorExt, ResultMetricsExt, envs},
};

mod error;
pub(crate) use error::{CliError, CliResult, CliResultExt, bad_input};

mod id;
pub use id::{CliId, IdMap};

pub use utils::binary_path::is_executed_as_but;

mod alias;
/// A place for all command implementations.
pub(crate) mod command;
mod retired_syntax;
pub mod theme;
mod tui;

const CLI_DATE: CustomFormat = gix::date::time::format::ISO8601;

/// The format for help output printed before clap parses arguments.
fn early_help_format(_args: &[OsString], agent_detected: bool) -> OutputFormat {
    if let Some(output_format_from_env) = OutputFormat::try_from_env(agent_detected)
        && output_format_from_env.is_human_text()
    {
        return output_format_from_env;
    }

    OutputFormat::Human {
        agent: agent_detected,
    }
}

fn parse_args_and_output_format(args: Vec<OsString>, agent_detected: bool) -> (Args, OutputFormat) {
    let command = Args::command();
    let matches = match command.try_get_matches_from(&args) {
        Ok(matches) => matches,
        Err(err) => {
            // Fall back to translating retired syntax (the pre-revamp
            // `but commit <branch> -c --changes <id>,<id>` form) before giving
            // up. Help and version requests also arrive as `Err`; translation
            // rejects them, so they exit through `err.exit()` below.
            use retired_syntax::Translation;
            let retry = match retired_syntax::translate_commit(&args) {
                Translation::Translated(translated) => {
                    Args::command().try_get_matches_from(translated).ok()
                }
                Translation::Refused => None,
                Translation::NotRetired => {
                    // Help and version requests also arrive as `Err`; they
                    // print to stdout and must pass through untouched.
                    if !err.use_stderr() {
                        err.exit()
                    }
                    // The other revamped commands are never translated, but a
                    // rejected line that looks like their retired syntax gets
                    // a teaching hint before the error.
                    let retired_hint = retired_syntax::parse_failure_hint(&args, agent_detected);
                    if let Some((command, hint)) = &retired_hint {
                        print_err_infallible(hint);
                        utils::metrics::emit_retired_syntax_hint(*command);
                    }
                    let _ = err.print();
                    // For the commands with the trickiest grammar, follow the
                    // error with example invocations — unless the hint above
                    // already named a concrete rewrite. The usage-line check
                    // keeps root-level errors (e.g. a bad root flag before the
                    // subcommand) from getting examples for a grammar that
                    // never rejected anything.
                    let concrete_rewrite = retired_hint
                        .as_ref()
                        .is_some_and(|(_, hint)| retired_syntax::hint_is_concrete(hint));
                    if !concrete_rewrite
                        && let Some(examples) = args::find_subcommand(&args)
                            .and_then(|(_, name)| name.to_str())
                            .filter(|name| err.to_string().contains(&format!("but {name}")))
                            .and_then(args::error_examples)
                    {
                        print_err_infallible(format!("\n{examples}"));
                    }
                    std::process::exit(err.exit_code())
                }
            };
            match retry {
                Some(matches) => {
                    retired_syntax::mark_used();
                    matches
                }
                // Retired syntax we cannot safely translate (e.g.
                // `--no-hooks`): teach the new form, then report the
                // original error.
                None => {
                    print_err_infallible(retired_syntax::hint(agent_detected));
                    utils::metrics::emit_retired_syntax_hint(args::metrics::CommandName::Commit);
                    err.exit()
                }
            }
        }
    };
    let args = Args::from_arg_matches(&matches).unwrap_or_else(|err| err.exit());

    let output_format = if args.format.json {
        OutputFormat::Json
    } else if let Some(output_format_from_env) = OutputFormat::try_from_env(agent_detected) {
        output_format_from_env
    } else {
        OutputFormat::Human {
            agent: agent_detected,
        }
    };

    (args, output_format)
}

fn should_detect_terminal_theme(
    args: &[OsString],
    agent_detected: bool,
    stdout_is_terminal: bool,
) -> bool {
    stdout_is_terminal
        && !agent_detected
        && !args.iter().any(|arg| arg == "--json")
        && !matches!(
            OutputFormat::try_from_env(agent_detected),
            Some(OutputFormat::Json)
        )
}

fn resolve_theme_preset(
    theme_name_from_env: Option<&OsStr>,
    detected_preset: Option<theme::ThemePreset>,
) -> anyhow::Result<theme::ThemePreset> {
    match theme_name_from_env {
        Some(theme_name) => theme_name.to_string_lossy().parse(),
        None => Ok(detected_preset.unwrap_or(theme::ThemePreset::Dark)),
    }
}

static APP_SETTINGS: std::sync::OnceLock<AppSettings> = std::sync::OnceLock::new();

/// The application settings, loaded from the default path once per process.
///
/// Concurrent first calls may load twice with one winner - harmless, and avoids
/// the still-unstable `OnceLock::get_or_try_init`.
pub(crate) fn app_settings() -> Result<&'static AppSettings> {
    if let Some(settings) = APP_SETTINGS.get() {
        return Ok(settings);
    }
    match AppSettings::load_from_default_path_creating_without_customization() {
        Ok(settings) => Ok(APP_SETTINGS.get_or_init(|| settings)),
        // A concurrent caller may have initialized while our load failed.
        Err(err) => APP_SETTINGS.get().ok_or(err),
    }
}

/// Handle `args` which must be what's passed by `std::env::args_os()`.
pub async fn handle_args(args: impl Iterator<Item = OsString>) -> Result<()> {
    let args: Vec<_> = args.collect();

    // Check if version is requested
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        let version = option_env!("VERSION").unwrap_or("dev");
        println!("but {version}");
        return Ok(());
    }

    let agent_detected = utils::detect_agent::detect().is_some();
    let (args, alias_expansion_warning) = expand_aliases(args);
    let theme_name_from_env = std::env::var_os(envs::BUT_THEME);
    let detected_preset = if theme_name_from_env.is_none()
        && should_detect_terminal_theme(&args, agent_detected, std::io::stdout().is_terminal())
    {
        theme::detect_terminal_preset()
    } else {
        None
    };
    let theme_preset_result = resolve_theme_preset(theme_name_from_env.as_deref(), detected_preset);

    {
        let theme_preset = match &theme_preset_result {
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

    if let Some(warning) = alias_expansion_warning {
        print_err_infallible(theme::get().attention.paint(warning));
    }

    // Check if help is requested and show grouped help instead of clap's default
    // Only intercept top-level help (but -h or but --help), not subcommand help
    let has_help_flag = args.iter().any(|arg| arg == "--help" || arg == "-h");
    let has_subcommand = args.len() > 2 && args[1] != "--help" && args[1] != "-h";
    if has_help_flag && !has_subcommand {
        let mut out = OutputChannel::new(early_help_format(&args, agent_detected));
        command::help::print_grouped(&mut out)?;
        return Ok(());
    }

    // The `but push --help` output is different if gerrit mode is enabled, hence the special handling
    let args_vec: Vec<String> = std::env::args().collect();
    // TODO: handle this as part of clap, it can be told to not generate all help.
    if args_vec.iter().any(|arg| arg == "push")
        && args_vec.iter().any(|arg| arg == "--help" || arg == "-h")
    {
        let mut out = OutputChannel::new(early_help_format(&args, agent_detected));
        command::push::help::print(&mut out)?;
        return Ok(());
    }

    // Handle bare `but help -h` and `but help --help` to show the grouped help output.
    // Topic help flags, like `but help cli-ids --help`, are left to clap.
    if args_vec.len() == 3
        && args_vec[1] == "help"
        && matches!(args_vec[2].as_str(), "--help" | "-h")
    {
        let mut out = OutputChannel::new(early_help_format(&args, agent_detected));
        command::help::print_grouped(&mut out)?;
        return Ok(());
    }

    let (mut args, output_format) = parse_args_and_output_format(args, agent_detected);
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

    let mut out = OutputChannel::new(output_format);
    #[cfg(feature = "legacy")]
    if matches!(
        &args.cmd,
        Some(Subcommands::Status { .. }) | Some(Subcommands::Diff { tui: false, .. })
    ) {
        out.request_pager();
    }

    #[cfg(feature = "legacy")]
    if matches!(&args.cmd, Some(Subcommands::_Diff2(_))) {
        out.request_pager();
    }

    if let (Err(theme_preset_err), Some(out)) = (theme_preset_result, out.for_human_ui()) {
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
    let app_settings = app_settings()?.clone();

    let result = match args.cmd.take() {
        Some(cmd @ Subcommands::External(_)) => {
            let metrics_ctx = cmd.to_metrics_context(&app_settings, &args.current_dir);
            let Subcommands::External(extra) = cmd else {
                unreachable!("external command was matched above")
            };
            command::external::dispatch(&args.current_dir, &extra).emit_metrics(metrics_ctx)
        }
        None => {
            // No arguments means run the default alias
            // The default alias expands to "status" which provides a helpful entry point
            let mut default_args: Vec<OsString> = vec!["but".into()];
            let expanded_alias = alias::expand_alias("default")?;
            default_args.extend_from_slice(&expanded_alias);
            let mut default_alias_args: Args = clap::Parser::parse_from(default_args);

            // Preserve globals from the default alias, while letting explicit user globals
            // take precedence (e.g. `but -C <dir>` without a subcommand).
            if args.trace > 0 {
                default_alias_args.trace = args.trace;
            }
            if args.current_dir != std::path::Path::new(".") {
                default_alias_args.current_dir = args.current_dir.clone();
            }
            default_alias_args.format = args.format;
            default_alias_args.status_after = args.status_after;

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

    if retired_syntax::was_used() {
        print_err_infallible(retired_syntax::hint(agent_detected));
    }

    match result {
        Err(CliError::Internal(err)) => Err(err),
        Err(CliError::CommandRejection) => std::process::exit(1),
        Err(CliError::BadInput(bad_input)) => print_and_exit_non_zero(bad_input),
        Err(CliError::ExternalCommandNotFound(command_name)) => {
            // Commands removed by the revamp (`rub`) land here as unknown
            // external subcommands; teach their replacements before the error.
            if let Some(hint) = retired_syntax::removed_command_hint(&command_name, agent_detected)
            {
                print_err_infallible(hint);
            }
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

/// Expand aliases in the argument list.
///
/// The parser treats aliases in the same way as external subcommands, so they end up inside of
/// [`Subcommands::External`]. Anytime we find an external subcommand, we attempt to expand the
/// first word in the command string as an alias.
///
/// This also has the intended effect of allowing aliases to shadow external commands. For example,
/// if there is an external command `but-b` on the PATH and an alias `b=branch`, then `but b` will
/// expand to `but branch` rather than be executed as `but-b`.
///
/// Cargo considers this shadowing behavior to be a security concern due to the fact that Cargo
/// aliases can be defined in the worktree of a repository (see
/// https://github.com/rust-lang/cargo/issues/10049). We don't have that problem as `but` aliases
/// can only ever be defined in Git config, which is trusted and does not follow with clones.
///
/// Root options are consumed by the parser separately from [`Subcommands::External`], so e.g. `but
/// -C some-alias a -b c` resolves into `External(["some-alias", "a", "-b", "c"])`, and the root
/// options are correctly parsed into the [`Args`] struct.
///
/// Note that at present, aliases are resolved from the real working directory ("."). If you pass
/// `-C /repo`, aliases from `/repo` are _not_ resolved.
///
/// # Examples
///
/// ```bash
/// # Set up aliases
/// but alias add b branch
/// but alias add bl 'branch list --local'
///
/// # Use them
/// but b                       # Expands to: but branch
/// but bl                      # Expands to: but branch list --local
/// but bl --review             # Expands to: but branch list --local --review
/// but -C /repo bl --review    # Expands to: but -C /repo branch list --local --review
/// ```
///
/// This function never fails - any unexpected situation leads to the original args being returned.
/// An alias expansion failure also returns a warning to print after the theme is initialized.
fn expand_aliases(args: Vec<OsString>) -> (Vec<OsString>, Option<String>) {
    expand_aliases_with(args, alias::expand_alias)
}

fn expand_aliases_with(
    args: Vec<OsString>,
    alias_expander: impl FnOnce(&str) -> anyhow::Result<Vec<OsString>>,
) -> (Vec<OsString>, Option<String>) {
    let parsed_args = match Args::try_parse_from(&args) {
        Ok(parsed) => parsed,
        Err(_) => {
            // We let the core parsing logic handle hard parse errors as there is special handling
            // of e.g. help output. If we get rid of that bespoke parsing we can also get rid of
            // this early return and let Clap handle parse errors with [`Args::parse_from`].
            return (args, None);
        }
    };

    match &parsed_args.cmd {
        Some(Subcommands::External(subcommand_args))
            if let Some(command_name) = subcommand_args.first() =>
        {
            if let Some(command_name) = command_name.to_str() {
                let subcommand_start = args.len() - subcommand_args.len();

                let expanded = match alias_expander(command_name) {
                    Ok(expanded) => expanded,
                    Err(err) => {
                        let warning = format!(
                            "Failed to expand alias '{command_name}': {err}\nSkipping alias expansion\n",
                        );
                        return (args, Some(warning));
                    }
                };

                let expanded_args = Vec::<OsString>::new()
                    .iter()
                    .chain(args[..subcommand_start].iter())
                    .chain(expanded.iter())
                    .chain(args[subcommand_start + 1..].iter())
                    .cloned()
                    .collect();
                (expanded_args, None)
            } else {
                (args, None)
            }
        }
        _ => (args, None),
    }
}

/// Print to stderr, ignoring any errors in printing. Use this when printing to stderr is the only
/// reasonable thing to do and there are no other options left.
fn print_err_infallible<T: std::fmt::Display>(err: T) {
    use std::io::Write;
    // We swallow this error, there is nothing more to do at this point
    let _ = write!(std::io::stderr(), "{err}");
}

fn print_deprecation_warning(message: impl AsRef<str>) {
    use std::io::Write;
    let t = theme::get();
    let _ = writeln!(
        std::io::stderr(),
        "{} {}",
        t.sym().warning,
        t.hint.paint(message.as_ref())
    );
}

fn print_and_exit_non_zero<T: std::fmt::Display>(err: T) -> ! {
    print_err_infallible(format!("{}{err}", theme::get().error.paint("Error: ")));
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

    let is_expand = matches!(&cmd, Subcommands::_Expand { .. });
    // A non-interactive invocation of a regular command: the situation where an
    // agent-facing maintenance notice is worth printing.
    let notice_worthy_command = !out.can_prompt()
        && !is_expand
        && !matches!(
            &cmd,
            Subcommands::Skill(_)
                | Subcommands::Agent(_)
                | Subcommands::Mcp(_)
                | Subcommands::Help { .. }
                | Subcommands::Completions { .. }
                | Subcommands::Metrics { .. }
        );
    let show_agent_skill_notice = out.format().is_human_text() && notice_worthy_command;
    let agent_skill_notice = show_agent_skill_notice
        .then(|| command::skill::agent_skill_notice(&args.current_dir))
        .flatten();
    if let Some(notice) = agent_skill_notice.as_ref()
        && let Some(human) = out.for_human()
    {
        writeln!(human, "{}", notice.text()).ok();
        writeln!(human).ok();
    }
    // Unlike the skill notice, the policy cleanup also runs for JSON-driven
    // agents; its notice goes to stderr there so the JSON contract on stdout
    // stays intact.
    let retired_policy_notice = notice_worthy_command
        .then(command::agent::retired_policy_syntax_notice)
        .flatten();
    if let Some(notice) = retired_policy_notice.as_ref() {
        if let Some(human) = out.for_human() {
            writeln!(human, "{notice}").ok();
            writeln!(human).ok();
        } else if out.is_json() {
            print_err_infallible(format!("{notice}\n"));
        }
    }
    let mut metrics_ctx = cmd.to_metrics_context(&app_settings, &args.current_dir);
    if agent_skill_notice.is_some_and(|notice| notice.is_hint())
        && let Some(metrics_ctx) = metrics_ctx.as_mut()
    {
        metrics_ctx.push_extra_prop("agentSkillHintShown", true);
    }
    if retired_policy_notice.is_some()
        && let Some(metrics_ctx) = metrics_ctx.as_mut()
    {
        metrics_ctx.push_extra_prop("retiredPolicySyntaxCleaned", true);
    }
    if retired_syntax::was_used()
        && let Some(metrics_ctx) = metrics_ctx.as_mut()
    {
        metrics_ctx.push_extra_prop("retiredCommitSyntax", true);
    }

    let mut ctx = match cmd {
        Subcommands::Metrics {
            command_name,
            props,
        } => {
            use args::metrics::CommandName;
            let mut event = utils::metrics::Event::new(command_name.into());
            if let Ok(props) = utils::metrics::Props::from_json_string(&props) {
                props.update_event(&mut event);
            }
            if matches!(command_name, CommandName::Commit | CommandName::CommitEmpty) {
                utils::metrics::add_workspace_shape(&mut event, &args.current_dir);
            }
            utils::metrics::capture_event_blocking(&app_settings, event).await;
            return Ok(());
        }
        Subcommands::Gui { new_window, path } => {
            let path = path
                .as_ref()
                .map(|path| args.current_dir.join(path))
                .unwrap_or_else(|| args.current_dir.clone());
            return command::gui::open(&path, new_window)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from);
        }
        Subcommands::Completions { shell } => {
            return command::completions::generate_completions(shell)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from);
        }
        Subcommands::Update(update_args::Platform { cmd }) => {
            return command::update::handle(cmd, out, &app_settings)
                .emit_metrics(metrics_ctx)
                .map_err(CliError::from);
        }
        Subcommands::Help { topic } => {
            command::help::print(out, topic)?;
            return Ok(());
        }
        Subcommands::Onboarding => {
            return command::onboarding::handle(out).map_err(CliError::from);
        }
        Subcommands::Config(args::config::Platform { cmd }) => {
            // Handle subcommands that don't require a repo context
            return match &cmd {
                Some(args::config::Subcommands::Metrics { status }) => {
                    command::config::metrics_config(out, *status)
                        .await
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                Some(args::config::Subcommands::Feature { flag, status }) => {
                    command::config::feature_config(out, *flag, *status)
                        .emit_metrics(metrics_ctx)
                        .map_err(CliError::from)
                }
                #[cfg(feature = "legacy")]
                Some(args::config::Subcommands::Forge {
                    cmd: Some(args::config::ForgeSubcommand::GithubStacks { .. }),
                }) => {
                    // Repository-local setting; handled below after project setup.
                    let mut ctx = setup::init_ctx(
                        &args,
                        InitCtxOptions {
                            background_sync: BackgroundSync::Disabled,
                            target_requirement: TargetRequirement::Optional,
                            ..Default::default()
                        },
                        out,
                    )?;
                    command::config::exec(&mut ctx, out, cmd)
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
                            let mut ctx = setup::init_ctx(&args, InitCtxOptions {
                                background_sync: BackgroundSync::Disabled,
                                target_requirement: TargetRequirement::Optional,
                                ..Default::default()
                            }, out)?;
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
            };
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

            return result.emit_metrics(metrics_ctx).map_err(CliError::from);
        }
        Subcommands::Agent(agent::Platform { cmd }) => {
            let result = command::agent::handle(&args.current_dir, out, cmd);

            return result.emit_metrics(metrics_ctx).map_err(CliError::from);
        }
        Subcommands::Mcp(args::mcp::Platform { cmd }) => {
            return match cmd {
                args::mcp::Subcommands::Serve => {
                    command::mcp::serve().await.map_err(CliError::from)
                }
            };
        }
        Subcommands::Edit { file } => {
            let path = args.current_dir.join(&file);
            tui::editor::edit_file(&path)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Setup { init } => {
            let repo = match but_api::legacy::projects::add_project_best_effort(
                args.current_dir.clone(),
            )? {
                gitbutler_project::AddProjectOutcome::Added(project)
                | gitbutler_project::AddProjectOutcome::AlreadyExists(project) => {
                    gix::open(project.git_dir())?
                }
                gitbutler_project::AddProjectOutcome::ReftableRefFormatUnsupported => {
                    return Err(anyhow::anyhow!(
                            "The repository at {} uses the currently unsupported reftable reference format.",
                            args.current_dir.display()
                        )
                        .into());
                }
                _ => command::legacy::setup::find_or_initialize_repo(&args.current_dir, out, init)?,
            };
            let mut ctx = but_ctx::Context::from_repo_with_settings(repo, app_settings.clone())?;
            let mut guard = ctx.exclusive_worktree_access();
            return command::legacy::setup::repo(
                &mut ctx,
                &args.current_dir,
                out,
                guard.write_permission(),
            )
            .context("Failed to set up GitButler project.")
            .emit_metrics(metrics_ctx)
            .map_err(CliError::from);
        }
        Subcommands::_Open { .. } => setup::init_ctx(
            &args,
            InitCtxOptions {
                background_sync: BackgroundSync::Disabled,
                ..Default::default()
            },
            out,
        )?,
        Subcommands::_Expand { .. } | Subcommands::Alias(..) => {
            but_ctx::Context::discover(&args.current_dir)?
        }
        Subcommands::Branch(branch::Platform { ref cmd }) => setup::init_ctx(
            &args,
            match cmd {
                Some(branch::Subcommands::Update { .. }) => InitCtxOptions {
                    workspace_check: setup::WorkspaceCheck::Disabled,
                    ..Default::default()
                },
                _ => InitCtxOptions {
                    background_sync: BackgroundSync::Enabled { silent: false },
                    ..Default::default()
                },
            },
            out,
        )?,
        Subcommands::Switch { .. } => setup::init_ctx(
            &args,
            InitCtxOptions {
                workspace_check: setup::WorkspaceCheck::Disabled,
                ..Default::default()
            },
            out,
        )?,
        Subcommands::_Comment(..) => setup::init_ctx(&args, InitCtxOptions::default(), out)?,
        #[cfg(feature = "legacy")]
        Subcommands::Actions { .. }
        | Subcommands::Pull { .. }
        | Subcommands::Fetch
        | Subcommands::Worktree(..)
        | Subcommands::Push(..)
        | Subcommands::Oplog(..)
        | Subcommands::Undo(..)
        | Subcommands::Redo(..)
        | Subcommands::RefreshRemoteData { .. }
        | Subcommands::Land { .. } => setup::init_ctx(&args, InitCtxOptions::default(), out)?,
        #[cfg(feature = "legacy")]
        Subcommands::Clean { .. }
        | Subcommands::Status { .. }
        | Subcommands::Diff { .. }
        | Subcommands::Show { .. }
        | Subcommands::Commit(..)
        | Subcommands::Squash(..)
        | Subcommands::Move(..)
        | Subcommands::_Diff2(..)
        | Subcommands::_Reword2(..)
        | Subcommands::Reword { .. }
        | Subcommands::Absorb { .. }
        | Subcommands::Discard(..)
        | Subcommands::Pr(..)
        | Subcommands::Resolve { .. }
        | Subcommands::Uncommit(..)
        | Subcommands::Amend(..)
        | Subcommands::Pick(..)
        | Subcommands::Unapply(..)
        | Subcommands::Apply(..) => setup::init_ctx(
            &args,
            InitCtxOptions {
                background_sync: BackgroundSync::Enabled { silent: false },
                ..Default::default()
            },
            out,
        )?,
        #[cfg(feature = "legacy")]
        Subcommands::Tui(..) => setup::init_ctx(
            &args,
            InitCtxOptions {
                background_sync: BackgroundSync::Enabled { silent: true },
                ..Default::default()
            },
            out,
        )?,
        #[cfg(feature = "legacy")]
        Subcommands::Teardown { .. } => setup::init_ctx(
            &args,
            InitCtxOptions {
                workspace_check: setup::WorkspaceCheck::Disabled,
                target_requirement: TargetRequirement::Optional,
                ..Default::default()
            },
            out,
        )?,
        Subcommands::AgentLog { .. } => {
            unreachable!("agentlog command is handled before metrics setup")
        }
        Subcommands::External(_) => {
            unreachable!("external commands are delegated before reaching match_subcommand")
        }
    };

    // If `Some`, and if result is `Ok`, this is passed to
    // `report_newly_conflicted()`.
    #[cfg(feature = "legacy")]
    let mut newly_conflicted_data: Option<command::legacy::conflict_notice::ConflictSnapshot> =
        None;
    // If `Some`, and if result is `Ok`, this is passed to
    // `run_status_after_if_requested()`.
    let mut status_after_data: Option<bool> = None;
    let _ws: Option<WorkspaceState> = match cmd {
        Subcommands::Metrics { .. }
        | Subcommands::Gui { .. }
        | Subcommands::Completions { .. }
        | Subcommands::Update(..)
        | Subcommands::Help { .. }
        | Subcommands::Onboarding
        | Subcommands::Config(..)
        | Subcommands::Skill(..)
        | Subcommands::Agent(..)
        | Subcommands::Mcp(..)
        | Subcommands::Edit { .. }
        | Subcommands::AgentLog { .. }
        | Subcommands::External(..) => {
            unreachable!("handled above")
        }
        #[cfg(feature = "legacy")]
        Subcommands::Setup { .. } => {
            unreachable!("handled above")
        }
        Subcommands::_Open {
            sources,
            program_id,
        } => {
            use crate::utils::IntermediateChannel;

            let out = IntermediateChannel::new(out);
            command::open::open(&ctx, out, sources, program_id).emit_metrics(metrics_ctx)?;
            None
        }
        Subcommands::_Expand { cli_id } => {
            let outcome = command::expand::handle(&ctx, cli_id).emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            None
        }
        Subcommands::Alias(alias_args::Platform { cmd }) => match cmd {
            Some(alias_args::Subcommands::List) | None => {
                command::alias::list(&*ctx.repo.get()?, out).emit_metrics(metrics_ctx)?;
                None
            }
            Some(alias_args::Subcommands::Add {
                name,
                value,
                global,
            }) => {
                command::alias::add(&mut ctx, out, &name, &value, global.into())
                    .emit_metrics(metrics_ctx)?;
                None
            }
            Some(alias_args::Subcommands::Remove { name, global }) => {
                command::alias::remove(&mut ctx, out, &name, global.into())
                    .emit_metrics(metrics_ctx)?;
                None
            }
        },
        Subcommands::Branch(branch::Platform { cmd }) => match cmd {
            #[cfg(not(feature = "legacy"))]
            None => todo!("implement list and call recursively"),
            #[cfg(feature = "legacy")]
            None => {
                command::legacy::branch::handle_no_subcommand(&mut ctx, out)?;
                None
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
                command::legacy::branch::list_branches(
                    &mut ctx, out, filter, local, remote, all, no_ahead, review, no_check, empty,
                )
                .emit_metrics(metrics_ctx)?;
                None
            }
            #[cfg(feature = "legacy")]
            Some(branch::Subcommands::Show {
                branch,
                review,
                files,
                ai,
                check,
            }) => {
                command::legacy::branch::show_branches(
                    &mut ctx, out, branch, review, files, ai, check,
                )
                .emit_metrics(metrics_ctx)?;
                None
            }
            #[cfg(feature = "legacy")]
            Some(branch::Subcommands::New(new_args)) => {
                use crate::utils::IntermediateChannel;

                let status_after = args.status_after;
                out.begin_status_after(status_after);
                status_after_data = Some(status_after);

                newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
                let outcome = command::legacy::branch::new::new(
                    &mut ctx,
                    IntermediateChannel::new(out),
                    new_args,
                )
                .emit_metrics(metrics_ctx)?;
                out.print_cli_output(outcome)?;
                None
            }
            #[cfg(feature = "legacy")]
            Some(branch::Subcommands::Delete { branches }) => {
                use crate::utils::IntermediateChannel;

                let status_after = args.status_after;
                out.begin_status_after(status_after);
                status_after_data = Some(status_after);

                newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
                let (outcome, ws) = command::legacy::branch::delete(
                    &mut ctx,
                    IntermediateChannel::new(out),
                    branches,
                )
                .emit_metrics(metrics_ctx)?;
                out.print_cli_output(outcome)?;
                Some(ws)
            }
            Some(branch::Subcommands::Update {
                branch,
                strategy,
                dry_run,
                verbose,
                interactive,
            }) => {
                let status_after = args.status_after && !dry_run && !interactive;
                out.begin_status_after(status_after);
                status_after_data = Some(status_after);
                command::branch::update(
                    &mut ctx,
                    &branch,
                    strategy,
                    dry_run,
                    verbose,
                    interactive,
                    out,
                )
                .emit_metrics(metrics_ctx)?;
                None
            }
            Some(branch::Subcommands::Move { .. }) => {
                return Err(bad_input(
                    "`but branch move` has been removed. Use `but move` instead.",
                )
                .into());
            }
        },
        Subcommands::Switch {
            target,
            workspace,
            new,
        } => {
            command::r#switch::handle(&mut ctx, out, target, workspace, new)
                .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Actions(actions::Platform { cmd }) => {
            match cmd {
                Some(actions::Subcommands::HandleChanges {
                    description,
                    handler,
                }) => {
                    command::legacy::actions::handle_changes(&mut ctx, out, handler, &description)?;
                }
                None => {
                    command::legacy::actions::list_actions(&ctx, out, 0, 10)?;
                }
            }
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Pull { check } => {
            command::legacy::pull::handle(&mut ctx, out, check)
                .await
                .emit_metrics(metrics_ctx)?;
            None
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
            command::legacy::pull::handle(&mut ctx, out, true)
                .await
                .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Clean {
            dry_run,
            pull,
            include_upstream,
        } => {
            let status_after = args.status_after;
            if pull {
                use std::fmt::Write;
                let mut progress = out.progress_channel();
                writeln!(progress, "Pulling latest...")?;
                command::legacy::pull::handle(&mut ctx, out, false).await?;
                writeln!(progress, "Pull complete.")?;
            }
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);
            command::legacy::clean::handle(
                &mut ctx,
                out,
                command::legacy::clean::CleanOptions {
                    dry_run,
                    include_upstream,
                },
            )
            .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Worktree(worktree::Platform { cmd }) => {
            command::legacy::worktree::handle(cmd, &mut ctx, out).emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Status {
            show_files,
            verbose,
            refresh_prs: sync_prs,
            upstream,
            no_hint,
            short: _,
        } => {
            use crate::command::legacy::status::FilesStatusFlag;
            use crate::command::legacy::status::StatusFlags;

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
            .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Tui(tui_args) => {
            use crate::command::legacy::status::{StatusFlags, StatusRenderMode, TuiLaunchOptions};

            if !out.format().allows_human_ui() {
                return Err(bad_input(
                    "Interactive terminal UI is not available for this output format.",
                )
                .into());
            }

            let options = TuiLaunchOptions::resolve(tui_args);
            command::legacy::status::worktree(
                &mut ctx,
                out,
                StatusFlags::for_tui(),
                StatusRenderMode::Tui(options),
            )
            .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Diff {
            target,
            tui,
            no_tui,
        } => {
            if tui && !out.format().allows_human_ui() {
                return Err(bad_input(
                    "Interactive terminal UI is not available for this output format.",
                )
                .into());
            }
            let use_tui = if tui {
                true
            } else if no_tui || !out.format().allows_human_ui() {
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
                print_deprecation_warning(
                    "`--tui` and `but.ui.tui` are deprecated and will be removed in a future release. \
                        Use `but tui --diff ID` instead",
                );

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
            command::legacy::show::show_commit(&mut ctx, out, &commit, verbose)
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Commit(commit_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            let (outcome, ws) = command::legacy::commit::commit(
                &mut ctx,
                IntermediateChannel::new(out),
                commit_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            Some(ws)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Squash(squash_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            let (outcome, ws) = command::legacy::squash::squash(
                &mut ctx,
                IntermediateChannel::new(out),
                squash_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            ws
        }
        #[cfg(feature = "legacy")]
        Subcommands::Move(move_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            let (outcome, ws) =
                command::legacy::r#move::r#move(&mut ctx, IntermediateChannel::new(out), move_args)
                    .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            Some(ws)
        }
        #[cfg(feature = "legacy")]
        Subcommands::_Diff2(diff_args) => {
            use crate::utils::IntermediateChannel;

            let outcome =
                command::legacy::diff2::diff(&mut ctx, IntermediateChannel::new(out), diff_args)
                    .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Push(push_args) => {
            command::legacy::push::handle(push_args, &mut ctx, out)
                .await
                .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::_Reword2(reword_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            let (outcome, ws) = command::legacy::reword2::reword(
                &mut ctx,
                IntermediateChannel::new(out),
                reword_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            ws
        }
        #[cfg(feature = "legacy")]
        Subcommands::Reword {
            target,
            message,
            format,
            diff,
            no_diff,
            allow_merged,
        } => {
            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);
            command::legacy::reword::reword_target(
                &mut ctx,
                out,
                target,
                message.as_deref(),
                format,
                // clap's `conflicts_with` should prevent this being `None` but better safe than
                // sorry
                ShowDiffInEditor::from_args(diff, no_diff).unwrap_or(ShowDiffInEditor::Unspecified),
                allow_merged,
            )
            .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Oplog(args::oplog::Platform { cmd }) => {
            match cmd {
                Some(args::oplog::Subcommands::List { since, snapshot }) => {
                    let filter = if snapshot {
                        Some(command::legacy::oplog::OplogFilter::Snapshot)
                    } else {
                        None
                    };
                    command::legacy::oplog::show_oplog(&mut ctx, out, since.as_deref(), filter)
                        .emit_metrics(metrics_ctx)?;
                    None
                }
                Some(args::oplog::Subcommands::Snapshot { message }) => {
                    command::legacy::oplog::create_snapshot(&mut ctx, out, message.as_deref())
                        .emit_metrics(metrics_ctx)?;
                    None
                }
                Some(args::oplog::Subcommands::Restore { oplog_sha }) => {
                    command::legacy::oplog::restore_to_oplog(&mut ctx, out, &oplog_sha)
                        .emit_metrics(metrics_ctx)?;
                    None
                }
                None => {
                    // Default to list when no subcommand is provided
                    command::legacy::oplog::show_oplog(&mut ctx, out, None, None)
                        .emit_metrics(metrics_ctx)?;
                    None
                }
            }
        }
        #[cfg(feature = "legacy")]
        Subcommands::Undo(undo_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            let outcome = command::legacy::undo_redo::undo(
                &mut ctx,
                IntermediateChannel::new(out),
                undo_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Redo(redo_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            let outcome = command::legacy::undo_redo::redo(
                &mut ctx,
                IntermediateChannel::new(out),
                redo_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Absorb {
            source,
            dry_run,
            allow_merged,
        } => {
            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);
            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            command::legacy::absorb::handle(
                &mut ctx,
                out,
                source.as_deref(),
                dry_run,
                allow_merged,
            )
            .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Discard(discard_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            let (outcome, ws) = command::legacy::discard::discard(
                &mut ctx,
                IntermediateChannel::new(out),
                discard_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            Some(ws)
        }
        Subcommands::_Comment(comment_args) => {
            use crate::utils::IntermediateChannel;

            let outcome =
                command::comment::comment(&mut ctx, IntermediateChannel::new(out), comment_args)
                    .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Teardown { checkout_to } => {
            command::legacy::teardown::teardown(&mut ctx, checkout_to, out)
                .map_err(|err| err.context("Failed to teardown GitButler project."))
                .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Pr(forge::pr::Platform {
            cmd,
            draft: top_level_draft,
        }) => {
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
                    .emit_metrics(metrics_ctx)?;
                    None
                }
                Some(forge::pr::Subcommands::Template { template_path }) => {
                    command::legacy::forge::review::set_review_template(
                        &mut ctx,
                        template_path,
                        out,
                    )
                    .context("Failed to set forge review template.")
                    .emit_metrics(metrics_ctx)?;
                    None
                }
                Some(forge::pr::Subcommands::AutoMerge { selector, off }) => {
                    command::legacy::forge::review::enable_auto_merge(&mut ctx, selector, off, out)
                        .await
                        .context("Failed to set the auto-merge state.")
                        .emit_metrics(metrics_ctx)?;
                    None
                }
                Some(forge::pr::Subcommands::SetDraft { selector }) => {
                    command::legacy::forge::review::set_draftiness(&mut ctx, selector, true, out)
                        .await
                        .context("Failed to set reviews as draft.")
                        .emit_metrics(metrics_ctx)?;
                    None
                }
                Some(forge::pr::Subcommands::SetReady { selector }) => {
                    command::legacy::forge::review::set_draftiness(&mut ctx, selector, false, out)
                        .await
                        .context("Failed to set reviews as ready-for-review.")
                        .emit_metrics(metrics_ctx)?;
                    None
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
                    .emit_metrics(metrics_ctx)?;
                    None
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
            command::legacy::refresh::handle(&mut ctx, out, fetch, prs, ci, updates, &app_settings)
                .emit_metrics(metrics_ctx)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Resolve { cmd, commit, ai } => {
            let status_after = args.status_after
                && matches!(&cmd, Some(crate::args::resolve::Subcommands::Finish));
            out.begin_status_after(status_after);
            let result = command::legacy::resolve::handle(&mut ctx, out, cmd, commit, ai)
                .context("Failed to handle conflict resolution.");
            if result.is_ok() {
                run_status_after_if_requested(status_after, &mut ctx, out);
            }
            result
                .emit_metrics(metrics_ctx)
                .show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Uncommit(uncommit_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            let (outcome, ws) = command::legacy::uncommit::uncommit(
                &mut ctx,
                IntermediateChannel::new(out),
                uncommit_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            ws
        }
        #[cfg(feature = "legacy")]
        Subcommands::Amend(amend_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            let (outcome, ws) =
                command::legacy::amend::amend(&mut ctx, IntermediateChannel::new(out), amend_args)
                    .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            ws
        }
        #[cfg(feature = "legacy")]
        Subcommands::Land {
            branch,
            yes,
            no_ff,
            whole_stack,
        } => {
            let conflicts_before = command::legacy::conflict_notice::snapshot(&ctx);
            let result =
                command::legacy::land::handle(&mut ctx, out, &branch, yes, no_ff, whole_stack)
                    .context("Failed to land branch.")
                    .emit_metrics(metrics_ctx);
            if result.is_ok() {
                command::legacy::conflict_notice::report_newly_conflicted(
                    &ctx,
                    out,
                    conflicts_before,
                );
            }
            result.show_root_cause_error_then_exit_without_destructors(output)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Pick(pick_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            newly_conflicted_data = Some(command::legacy::conflict_notice::snapshot(&ctx));
            let (outcome, ws) =
                command::legacy::pick::pick(&mut ctx, IntermediateChannel::new(out), pick_args)
                    .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            Some(ws)
        }
        #[cfg(feature = "legacy")]
        Subcommands::Unapply(unapply_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            let outcome = command::legacy::unapply::unapply(
                &mut ctx,
                IntermediateChannel::new(out),
                unapply_args,
            )
            .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            None
        }
        #[cfg(feature = "legacy")]
        Subcommands::Apply(apply_args) => {
            use crate::utils::IntermediateChannel;

            let status_after = args.status_after;
            out.begin_status_after(status_after);
            status_after_data = Some(status_after);

            let outcome =
                command::legacy::apply::apply(&mut ctx, IntermediateChannel::new(out), apply_args)
                    .emit_metrics(metrics_ctx)?;
            out.print_cli_output(outcome)?;
            None
        }
    };

    #[cfg(feature = "legacy")]
    if let Some(conflicts_before) = newly_conflicted_data {
        command::legacy::conflict_notice::report_newly_conflicted(&ctx, out, conflicts_before);
    }
    #[cfg(feature = "legacy")]
    if let Some(status_after) = status_after_data {
        run_status_after_if_requested(status_after, &mut ctx, out);
    }

    Ok(())
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

#[cfg(feature = "legacy")]
fn run_status_after_if_requested(
    status_after: bool,
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) {
    if !status_after {
        if out.is_json()
            && let Some(notice) = command::skill::agent_skill_update_notice()
        {
            eprintln!("{notice}");
        }
        return;
    }
    let mutation_json = out.take_json_buffer();
    run_status_after(ctx, out, mutation_json);
}

/// Run workspace status output after a mutation command when explicitly requested.
///
/// In human mode, prints a blank line then full status.
/// In JSON mode, combines the mutation's buffered JSON with status JSON into
/// `{"result": <mutation_output>, "status": <workspace_status>}`.
/// For JSON commands, reconciles stale skill installations and includes an
/// update announcement or failure notice under `agent_skill_notice`.
/// The global `--status-after` flag controls whether this function runs.
///
/// Status errors are handled gracefully: in JSON mode the mutation result is
/// always emitted (with a `"status_error"` field on failure); in human mode
/// a warning is printed to stderr.
#[cfg(feature = "legacy")]
fn run_status_after(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    mutation_json: Option<serde_json::Value>,
) {
    use crate::command::legacy::status::StatusFlags;

    let agent_skill_notice = out
        .format()
        .is_json()
        .then(command::skill::agent_skill_update_notice)
        .flatten();

    if out.is_json() {
        out.start_json_buffering();
        let status_result = command::legacy::status::worktree(
            ctx,
            out,
            StatusFlags::all_false(),
            command::legacy::status::StatusRenderMode::Oneshot,
        );
        let status_json = out.take_json_buffer().unwrap_or(serde_json::Value::Null);

        let mut combined = match status_result {
            Ok(()) => serde_json::json!({
                "result": mutation_json.unwrap_or(serde_json::Value::Null),
                "status": status_json,
            }),
            Err(err) => {
                eprintln!(
                    "warning: status after mutation failed: {err:#}. Run 'but status' separately to check workspace state."
                );
                serde_json::json!({
                    "result": mutation_json.unwrap_or(serde_json::Value::Null),
                    "status_error": format!("{err:#}"),
                })
            }
        };
        if let Some(notice) = agent_skill_notice
            && let Some(object) = combined.as_object_mut()
        {
            object.insert(
                "agent_skill_notice".to_string(),
                serde_json::Value::String(notice),
            );
        }
        if let Err(err) = out.write_value(combined) {
            eprintln!("warning: failed to write status after mutation: {err}");
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
        ) {
            eprintln!(
                "warning: status after mutation failed: {err:#}. Run 'but status' separately to check workspace state."
            );
        }
    }
}

#[cfg(feature = "legacy")]
mod legacy;

mod setup;
pub mod trace;
mod utils;
#[doc(hidden)]
pub use utils::detect_agent::ENVIRONMENT_VARIABLES as AGENT_ENVIRONMENT_VARIABLES;

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(|arg| OsString::from(*arg)).collect()
    }

    #[test]
    fn expanded_alias_arguments_control_terminal_theme_detection() {
        let (expanded_args, warning) = expand_aliases_with(os_args(&["but", "js"]), |alias| {
            assert_eq!(alias, "js", "the external command should be expanded");
            Ok(os_args(&["status", "--json"]))
        });

        assert!(
            warning.is_none(),
            "successful alias expansion should not produce a warning"
        );
        temp_env::with_var(envs::BUT_OUTPUT_FORMAT, None::<&str>, || {
            assert!(
                !should_detect_terminal_theme(&expanded_args, false, true),
                "an alias that enables JSON output should not query the terminal"
            );
        });
    }

    #[test]
    fn terminal_theme_detection_is_limited_to_interactive_human_output() {
        temp_env::with_var(envs::BUT_OUTPUT_FORMAT, None::<&str>, || {
            assert!(
                should_detect_terminal_theme(&os_args(&["but", "status"]), false, true),
                "an interactive human command should detect the terminal theme"
            );
            assert!(
                !should_detect_terminal_theme(&os_args(&["but", "status"]), true, true),
                "an agent command should not query the terminal"
            );
            assert!(
                !should_detect_terminal_theme(&os_args(&["but", "status", "--json"]), false, true),
                "a JSON command should not query the terminal"
            );
            assert!(
                !should_detect_terminal_theme(&os_args(&["but", "status"]), false, false),
                "a redirected command should not query the terminal"
            );
        });

        temp_env::with_var(envs::BUT_OUTPUT_FORMAT, Some("json"), || {
            assert!(
                !should_detect_terminal_theme(&os_args(&["but", "status"]), false, true),
                "JSON output selected through the environment should not query the terminal"
            );
        });
    }

    #[test]
    fn explicit_theme_overrides_detection_and_detection_falls_back_to_dark() {
        assert_eq!(
            resolve_theme_preset(Some(OsStr::new("light")), Some(theme::ThemePreset::Dark))
                .unwrap(),
            theme::ThemePreset::Light,
            "BUT_THEME should override the detected terminal theme"
        );
        assert_eq!(
            resolve_theme_preset(None, Some(theme::ThemePreset::Light)).unwrap(),
            theme::ThemePreset::Light,
            "the detected terminal theme should be used without an override"
        );
        assert_eq!(
            resolve_theme_preset(None, None).unwrap(),
            theme::ThemePreset::Dark,
            "failed terminal detection should fall back to the dark preset"
        );
    }

    #[test]
    fn detected_agent_defaults_to_agent_output() {
        let (_, format) = temp_env::with_var(envs::BUT_OUTPUT_FORMAT, None::<&str>, || {
            parse_args_and_output_format(os_args(&["but", "status"]), true)
        });

        assert!(matches!(format, OutputFormat::Human { agent: true }));
    }

    #[test]
    fn detected_agent_preserves_environment_output_format() {
        let (_, format) = temp_env::with_var(envs::BUT_OUTPUT_FORMAT, Some("json"), || {
            parse_args_and_output_format(os_args(&["but", "status"]), true)
        });

        assert!(matches!(format, OutputFormat::Json));
    }

    #[test]
    #[cfg(feature = "legacy")]
    fn detected_agent_omits_status_after_mutation_by_default() {
        let (args, _) =
            parse_args_and_output_format(os_args(&["but", "commit", "--no-message"]), true);

        assert!(
            !args.status_after,
            "detected agents must not request mutation status implicitly"
        );
    }

    #[test]
    #[cfg(feature = "legacy")]
    fn detected_agent_can_request_status_after_mutation() {
        let (args, _) = parse_args_and_output_format(
            os_args(&["but", "commit", "--status-after", "--no-message"]),
            true,
        );

        assert!(
            args.status_after,
            "detected agents must retain an explicit mutation status request"
        );
    }

    #[test]
    fn detected_agent_defaults_early_help_to_agent_output() {
        let format = temp_env::with_var(envs::BUT_OUTPUT_FORMAT, None::<&str>, || {
            early_help_format(&os_args(&["but", "--help"]), true)
        });

        assert!(matches!(format, OutputFormat::Human { agent: true }));
    }
}

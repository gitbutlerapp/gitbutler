//! CLI argument types and parsing for `but link`.

use std::time::Duration;

use anyhow::Context;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Default observer id for commands that don't mutate coordination state.
pub(crate) const OBSERVER_AGENT_ID: &str = "tier4-observer";

/// Output format for path-based coordination checks and acquisition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum CheckFormat {
    /// Emit a structured JSON payload.
    Full,
    /// Emit one compact text line per path.
    Compact,
}

/// Output format for discovery views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum DiscoveryFormat {
    /// Emit the full discovery payloads.
    Full,
    /// Emit a brief discovery view.
    Brief,
    /// Emit a condensed discovery digest.
    Digest,
}

/// Read views for the coordination state snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ReadView {
    /// Agent-specific inbox with actionable coordination state.
    Inbox,
    /// Full transcript-style snapshot.
    Full,
    /// Discovery-only view.
    Discoveries,
    /// Message transcript only.
    Messages,
    /// Active claims only.
    Claims,
    /// Agent state only.
    Agents,
}

/// Block severity for typed blockers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum BlockMode {
    /// Informational blocker that only denies under `--strict`.
    Advisory,
    /// Strong blocker that always denies acquisition.
    Hard,
}

/// Parsed command representation consumed by the runtime.
#[derive(Debug)]
pub(crate) enum Cmd {
    /// Trusted check-and-claim operation.
    Acquire {
        paths: Vec<String>,
        ttl: Duration,
        strict: bool,
        dry_run: bool,
        format: CheckFormat,
    },
    /// Post a free-text transcript message.
    Post { message: String },
    /// Read coordination state.
    Read {
        view: ReadView,
        format: DiscoveryFormat,
        since: Option<i64>,
    },
    /// Set or clear agent status.
    Status { value: Option<String> },
    /// Set or clear agent plan.
    Plan { value: Option<String> },
    /// Run read-only TUI.
    Tui,
    /// Finish work, release claims, and clear transient state.
    Done { summary: String },
    /// Post a discovery record.
    Discovery {
        title: String,
        evidence: Vec<String>,
        action: String,
        signal: Option<String>,
    },
    /// Publish an intent declaration.
    Intent {
        scope: String,
        tags: Vec<String>,
        surface: Vec<String>,
        paths: Vec<String>,
    },
    /// Publish an ownership declaration.
    Declare {
        scope: String,
        tags: Vec<String>,
        surface: Vec<String>,
        paths: Vec<String>,
    },
    /// Create an authoritative typed block.
    Block {
        paths: Vec<String>,
        reason: String,
        mode: BlockMode,
        ttl: Option<Duration>,
    },
    /// Resolve an authoritative typed block.
    Resolve { block_id: i64 },
    /// Record an authoritative acknowledgement.
    Ack {
        target_agent_id: String,
        paths: Vec<String>,
        note: Option<String>,
    },
}

/// Top-level clap platform for `but link`.
#[derive(Debug, clap::Parser)]
#[command(
    name = "but-link",
    disable_help_subcommand = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Platform {
    #[arg(
        short = 'H',
        long = "help",
        global = true,
        action = clap::ArgAction::Help,
        help = "Print help"
    )]
    _help: Option<bool>,
    #[arg(long, global = true, value_parser = parse_non_empty_string)]
    agent_id: Option<String>,
    #[command(subcommand)]
    cmd: Option<LinkSubcommand>,
}

/// User-facing subcommands for `but link`.
#[derive(Debug, clap::Subcommand)]
enum LinkSubcommand {
    Acquire(AcquireArgs),
    Post(PostArgs),
    Read(ReadArgs),
    Status(StatusPlanArgs),
    Plan(StatusPlanArgs),
    Tui,
    Done(DoneArgs),
    Discovery(DiscoveryArgs),
    Intent(IntentArgs),
    Declare(DeclareArgs),
    Block(BlockArgs),
    Resolve(ResolveArgs),
    Ack(AckArgs),
}

/// Arguments for the trusted acquisition command.
#[derive(Debug, clap::Args)]
struct AcquireArgs {
    /// Paths to acquire.
    #[arg(long = "path", required = true)]
    paths: Vec<String>,
    #[arg(long, default_value = "15m")]
    ttl: String,
    #[arg(long, default_value_t = false)]
    strict: bool,
    #[arg(long, default_value_t = false)]
    dry_run: bool,
    #[arg(long, value_enum, default_value_t = CheckFormat::Full)]
    format: CheckFormat,
}

/// Arguments for posting transcript messages.
#[derive(Debug, clap::Args)]
struct PostArgs {
    #[arg(value_name = "MESSAGE")]
    message: Vec<String>,
}

/// Arguments for reading coordination state.
#[derive(Debug, clap::Args)]
struct ReadArgs {
    #[arg(long, value_enum)]
    view: Option<ReadView>,
    #[arg(long, value_enum, default_value_t = DiscoveryFormat::Full)]
    format: DiscoveryFormat,
    #[arg(long, value_parser = parse_since_timestamp_arg)]
    since: Option<i64>,
}

/// Shared arguments for `status` and `plan`.
#[derive(Debug, clap::Args)]
struct StatusPlanArgs {
    #[arg(long, default_value_t = false)]
    clear: bool,
    #[arg(value_name = "VALUE")]
    value: Vec<String>,
}

/// Arguments for `done`.
#[derive(Debug, clap::Args)]
struct DoneArgs {
    #[arg(value_name = "SUMMARY")]
    summary: Vec<String>,
}

/// Arguments for `discovery`.
#[derive(Debug, clap::Args)]
struct DiscoveryArgs {
    /// What was discovered (e.g. "breaking rename in types.rs").
    #[arg(value_name = "TITLE")]
    title: Vec<String>,
    /// Evidence supporting the discovery (repeatable).
    #[arg(long, required = true)]
    evidence: Vec<String>,
    /// Suggested command for other agents to run.
    #[arg(long)]
    action: String,
    /// Signal level: "high" or "low" (default: high).
    #[arg(long)]
    signal: Option<String>,
}

/// Shared arguments for `intent` and `declare`.
#[derive(Debug, clap::Args)]
struct IntentArgs {
    /// Scope of the API surface (e.g. "crate::auth").
    #[arg(value_name = "SCOPE")]
    scope: Vec<String>,
    /// Tags describing the surface (repeatable, e.g. "api", "internal").
    #[arg(long, required = true)]
    tag: Vec<String>,
    /// Surface tokens that identify the API (repeatable, e.g. function/type names).
    #[arg(long, required = true)]
    surface: Vec<String>,
    /// Optional file/module scopes for this surface declaration.
    #[arg(long = "path")]
    paths: Vec<String>,
}

/// Arguments for `declare`.
#[derive(Debug, clap::Args)]
struct DeclareArgs {
    /// Scope of the API surface (e.g. "crate::auth").
    #[arg(value_name = "SCOPE")]
    scope: Vec<String>,
    /// Tags describing the surface (repeatable, e.g. "api", "internal").
    #[arg(long, required = true)]
    tag: Vec<String>,
    /// Surface tokens that identify the API (repeatable, e.g. function/type names).
    #[arg(long, required = true)]
    surface: Vec<String>,
    /// Optional file/module scopes for this surface declaration.
    #[arg(long = "path")]
    paths: Vec<String>,
}

/// Arguments for creating authoritative typed blocks.
#[derive(Debug, clap::Args)]
struct BlockArgs {
    /// Paths covered by the block.
    #[arg(long = "path", required = true)]
    paths: Vec<String>,
    /// Human-readable reason for the block.
    #[arg(long)]
    reason: String,
    /// Block severity.
    #[arg(long, value_enum, default_value_t = BlockMode::Advisory)]
    mode: BlockMode,
    /// Optional expiry.
    #[arg(long)]
    ttl: Option<String>,
}

/// Arguments for resolving authoritative typed blocks.
#[derive(Debug, clap::Args)]
struct ResolveArgs {
    #[arg(long = "block-id")]
    block_id: i64,
}

/// Arguments for authoritative acknowledgements.
#[derive(Debug, clap::Args)]
struct AckArgs {
    /// Agent being acknowledged.
    #[arg(long = "agent", value_parser = parse_non_empty_string)]
    target_agent_id: String,
    /// Optional path scope for the acknowledgement.
    #[arg(long = "path")]
    paths: Vec<String>,
    /// Optional note included in history output.
    #[arg(long)]
    note: Option<String>,
}

impl Platform {
    /// Convert parsed clap arguments into runtime command values.
    pub(crate) fn into_runtime(self) -> anyhow::Result<(String, Cmd)> {
        let cmd = match self.cmd.context("missing subcommand")? {
            LinkSubcommand::Acquire(args) => Cmd::Acquire {
                paths: args.paths,
                ttl: parse_duration_arg(&args.ttl).map_err(|e| anyhow::anyhow!(e))?,
                strict: args.strict,
                dry_run: args.dry_run,
                format: args.format,
            },
            LinkSubcommand::Post(args) => {
                anyhow::ensure!(!args.message.is_empty(), "message required");
                Cmd::Post {
                    message: args.message.join(" "),
                }
            }
            LinkSubcommand::Read(args) => Cmd::Read {
                view: args.view.unwrap_or(ReadView::Inbox),
                format: args.format,
                since: args.since,
            },
            LinkSubcommand::Status(args) => Cmd::Status {
                value: parse_status_plan_value(&args)?,
            },
            LinkSubcommand::Plan(args) => Cmd::Plan {
                value: parse_status_plan_value(&args)?,
            },
            LinkSubcommand::Tui => Cmd::Tui,
            LinkSubcommand::Done(args) => {
                anyhow::ensure!(!args.summary.is_empty(), "summary required");
                Cmd::Done {
                    summary: args.summary.join(" "),
                }
            }
            LinkSubcommand::Discovery(args) => {
                anyhow::ensure!(!args.title.is_empty(), "title required");
                Cmd::Discovery {
                    title: args.title.join(" "),
                    evidence: args.evidence,
                    action: args.action,
                    signal: args.signal,
                }
            }
            LinkSubcommand::Intent(args) => {
                anyhow::ensure!(!args.scope.is_empty(), "scope required");
                Cmd::Intent {
                    scope: args.scope.join(" "),
                    tags: args.tag,
                    surface: args.surface,
                    paths: args.paths,
                }
            }
            LinkSubcommand::Declare(args) => {
                anyhow::ensure!(!args.scope.is_empty(), "scope required");
                Cmd::Declare {
                    scope: args.scope.join(" "),
                    tags: args.tag,
                    surface: args.surface,
                    paths: args.paths,
                }
            }
            LinkSubcommand::Block(args) => Cmd::Block {
                paths: args.paths,
                reason: args.reason,
                mode: args.mode,
                ttl: args
                    .ttl
                    .as_deref()
                    .map(parse_duration_arg)
                    .transpose()
                    .map_err(|e| anyhow::anyhow!(e))?,
            },
            LinkSubcommand::Resolve(args) => Cmd::Resolve {
                block_id: args.block_id,
            },
            LinkSubcommand::Ack(args) => Cmd::Ack {
                target_agent_id: args.target_agent_id,
                paths: args.paths,
                note: args.note,
            },
        };

        let agent_id = match self.agent_id {
            Some(id) => id,
            None if uses_observer_agent_id(&cmd) => OBSERVER_AGENT_ID.to_owned(),
            None => anyhow::bail!("--agent-id required"),
        };

        Ok((agent_id, cmd))
    }
}

/// Return whether a command may use the default observer agent id.
fn uses_observer_agent_id(cmd: &Cmd) -> bool {
    matches!(
        cmd,
        Cmd::Tui
            | Cmd::Read {
                view: ReadView::Claims | ReadView::Agents,
                ..
            }
    )
}

/// Normalize a repo-relative claim path: strip `./` prefix, trailing `/`, collapse `.` and `..`.
pub(crate) fn normalize_claim_path(s: &str) -> anyhow::Result<String> {
    let trimmed = s.trim();
    anyhow::ensure!(!trimmed.is_empty(), "path must not be empty");
    anyhow::ensure!(
        !std::path::Path::new(trimmed).is_absolute(),
        "path must be repo-relative"
    );

    let mut out = trimmed.replace('\\', "/");
    while out.starts_with("./") {
        out = out.trim_start_matches("./").to_owned();
    }
    while out.ends_with('/') && out != "/" {
        out.pop();
    }

    let mut parts: Vec<&str> = Vec::new();
    for p in out.split('/') {
        if p.is_empty() || p == "." {
            continue;
        }
        if p == ".." {
            if let Some(last) = parts.last()
                && *last != ".."
            {
                parts.pop();
                continue;
            }
            parts.push("..");
            continue;
        }
        parts.push(p);
    }

    let normalized = parts.join("/");
    anyhow::ensure!(!normalized.is_empty(), "path must not be empty");
    anyhow::ensure!(
        !normalized.starts_with("../") && normalized != "..",
        "path must stay within repository"
    );
    Ok(normalized)
}

/// Parse a required non-empty string argument.
fn parse_non_empty_string(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        Err("value must not be empty".to_owned())
    } else {
        Ok(raw.to_owned())
    }
}

/// Parse a human duration string into `Duration`.
fn parse_duration_arg(raw: &str) -> Result<Duration, String> {
    parse_duration(raw).map_err(|()| format!("invalid duration: {raw}"))
}

/// Parse `--since` timestamps in unix-ms or RFC3339 form.
fn parse_since_timestamp_arg(raw: &str) -> Result<i64, String> {
    parse_since_timestamp(raw).map_err(|()| format!("invalid timestamp: {raw}"))
}

/// Parse a timestamp in unix-ms or RFC3339.
fn parse_since_timestamp(raw: &str) -> Result<i64, ()> {
    if let Ok(ms) = raw.trim().parse::<i64>() {
        return Ok(ms);
    }
    let dt = OffsetDateTime::parse(raw.trim(), &Rfc3339).map_err(|_| ())?;
    let nanos = dt.unix_timestamp_nanos();
    let ms = nanos
        .checked_div(1_000_000)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or(())?;
    Ok(ms)
}

/// Parse `status`/`plan` values and `--clear`.
fn parse_status_plan_value(args: &StatusPlanArgs) -> anyhow::Result<Option<String>> {
    if args.clear {
        anyhow::ensure!(args.value.is_empty(), "cannot provide value with --clear");
        return Ok(None);
    }
    anyhow::ensure!(!args.value.is_empty(), "value required");
    Ok(Some(args.value.join(" ")))
}

/// Parse duration strings like `15m`, `1h`, or `60`.
pub(crate) fn parse_duration(s: &str) -> Result<Duration, ()> {
    let (n, unit) = split_num_unit(s)?;
    let secs = match unit {
        "" | "s" => n,
        "m" => n.saturating_mul(60),
        "h" => n.saturating_mul(60 * 60),
        "d" => n.saturating_mul(60 * 60 * 24),
        _ => return Err(()),
    };
    Ok(Duration::from_secs(secs))
}

/// Split a duration into numeric and suffix segments.
fn split_num_unit(s: &str) -> Result<(u64, &str), ()> {
    let s = s.trim();
    if s.is_empty() {
        return Err(());
    }
    let mut idx = 0usize;
    for (i, ch) in s.char_indices() {
        if !ch.is_ascii_digit() {
            idx = i;
            break;
        }
    }
    if idx == 0 {
        if s.chars().all(|c| c.is_ascii_digit()) {
            let n: u64 = s.parse().map_err(|_| ())?;
            return Ok((n, ""));
        }
        return Err(());
    }
    let (num, unit) = s.split_at(idx);
    let n: u64 = num.parse().map_err(|_| ())?;
    Ok((n, unit))
}

/// Map a `Cmd` variant to its string name (for logging).
pub(crate) fn cmd_name(cmd: &Cmd) -> &'static str {
    match cmd {
        Cmd::Acquire { .. } => "acquire",
        Cmd::Post { .. } => "post",
        Cmd::Read { .. } => "read",
        Cmd::Status { .. } => "status",
        Cmd::Plan { .. } => "plan",
        Cmd::Tui => "tui",
        Cmd::Done { .. } => "done",
        Cmd::Discovery { .. } => "discovery",
        Cmd::Intent { .. } => "intent",
        Cmd::Declare { .. } => "declare",
        Cmd::Block { .. } => "block",
        Cmd::Resolve { .. } => "resolve",
        Cmd::Ack { .. } => "ack",
    }
}

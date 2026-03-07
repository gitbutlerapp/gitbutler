//! CLI argument types and parsing for `but link`.

use std::time::Duration;

use anyhow::Context;
#[cfg(test)]
use clap::CommandFactory;
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
    /// Low-level claim command.
    Claim { paths: Vec<String>, ttl: Duration },
    /// Trusted check-and-claim operation.
    Acquire {
        paths: Vec<String>,
        ttl: Duration,
        strict: bool,
        format: CheckFormat,
    },
    /// Release active claims.
    Release { paths: Vec<String> },
    /// List active claims.
    Claims { path_prefix: Option<String> },
    /// Read-only analysis for candidate paths.
    Check {
        paths: Vec<String>,
        strict: bool,
        format: CheckFormat,
    },
    /// Post a free-text transcript message.
    Post { message: String },
    /// Post a typed JSON payload for discovery/intent/declaration.
    PostTyped { kind: String, json: String },
    /// Read coordination state.
    Read { view: ReadView, since: Option<i64> },
    /// Brief discovery view.
    Brief { kind: Option<String>, all: bool },
    /// Digest discovery view.
    Digest { kind: Option<String>, all: bool },
    /// Set or clear agent status.
    Status { value: Option<String> },
    /// Set or clear agent plan.
    Plan { value: Option<String> },
    /// List agents.
    Agents,
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
    /// Hidden evaluation hook.
    EvalUserPromptSubmit,
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
    Claim(ClaimArgs),
    Acquire(AcquireArgs),
    Release(ReleaseArgs),
    Claims(ClaimsArgs),
    Check(CheckArgs),
    Post(PostArgs),
    Read(ReadArgs),
    Brief(BriefDigestArgs),
    Digest(BriefDigestArgs),
    Status(StatusPlanArgs),
    Plan(StatusPlanArgs),
    Agents,
    Tui,
    Done(DoneArgs),
    Discovery(DiscoveryArgs),
    Intent(IntentArgs),
    Declare(DeclareArgs),
    Block(BlockArgs),
    Resolve(ResolveArgs),
    Ack(AckArgs),
    #[command(hide = true)]
    Eval(EvalArgs),
}

/// Arguments for commands that claim paths.
#[derive(Debug, clap::Args)]
struct ClaimArgs {
    /// Paths to claim (positional, comma-separated OK).
    paths: Vec<String>,
    /// Backward-compat flag form.
    #[arg(long = "path", hide = true)]
    flag_paths: Vec<String>,
    #[arg(long, default_value = "5m")]
    ttl: String,
}

/// Arguments for the trusted acquisition command.
#[derive(Debug, clap::Args)]
struct AcquireArgs {
    /// Paths to acquire (positional, comma-separated OK).
    paths: Vec<String>,
    /// Backward-compat flag form.
    #[arg(long = "path", hide = true)]
    flag_paths: Vec<String>,
    #[arg(long, default_value = "15m")]
    ttl: String,
    #[arg(long, default_value_t = false)]
    strict: bool,
    #[arg(long, value_enum, default_value_t = CheckFormat::Full)]
    format: CheckFormat,
}

/// Arguments for commands that release paths.
#[derive(Debug, clap::Args)]
struct ReleaseArgs {
    /// Paths to release (positional, comma-separated OK).
    paths: Vec<String>,
    /// Backward-compat flag form.
    #[arg(long = "path", hide = true)]
    flag_paths: Vec<String>,
}

/// Arguments for listing claims.
#[derive(Debug, clap::Args)]
struct ClaimsArgs {
    #[arg(long = "path-prefix")]
    path_prefix: Option<String>,
}

/// Arguments for read-only path analysis.
#[derive(Debug, clap::Args)]
struct CheckArgs {
    /// Paths to check (positional, comma-separated OK).
    paths: Vec<String>,
    /// Backward-compat flag form.
    #[arg(long = "path", hide = true)]
    flag_paths: Vec<String>,
    #[arg(long, default_value_t = false)]
    strict: bool,
    #[arg(long, value_enum, default_value_t = CheckFormat::Full)]
    format: CheckFormat,
}

/// Arguments for posting transcript messages.
#[derive(Debug, clap::Args)]
struct PostArgs {
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(value_name = "MESSAGE")]
    message: Vec<String>,
}

/// Arguments for reading coordination state.
#[derive(Debug, clap::Args)]
struct ReadArgs {
    #[arg(long, value_enum)]
    view: Option<ReadView>,
    #[arg(long = "type", hide = true)]
    legacy_kind: Option<String>,
    #[arg(long, value_parser = parse_since_timestamp_arg)]
    since: Option<i64>,
}

/// Shared arguments for `brief` and `digest`.
#[derive(Debug, clap::Args)]
struct BriefDigestArgs {
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(long, default_value_t = false)]
    all: bool,
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
    /// Paths covered by the block (positional, comma-separated OK).
    paths: Vec<String>,
    /// Backward-compat flag form.
    #[arg(long = "path", hide = true)]
    flag_paths: Vec<String>,
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
    paths: Vec<String>,
    /// Backward-compat flag form.
    #[arg(long = "path", hide = true)]
    flag_paths: Vec<String>,
    /// Optional note included in history output.
    #[arg(long)]
    note: Option<String>,
}

/// Hidden evaluation hook arguments.
#[derive(Debug, clap::Args)]
struct EvalArgs {
    #[command(subcommand)]
    cmd: EvalSubcommand,
}

/// Hidden evaluation hook subcommands.
#[derive(Debug, clap::Subcommand)]
enum EvalSubcommand {
    #[command(name = "user-prompt-submit")]
    UserPromptSubmit,
}

impl Platform {
    /// Convert parsed clap arguments into runtime command values.
    pub(crate) fn into_runtime(self) -> anyhow::Result<(String, Cmd)> {
        let cmd = match self.cmd.context("missing subcommand")? {
            LinkSubcommand::Claim(args) => Cmd::Claim {
                paths: require_paths(merge_paths(args.paths, args.flag_paths))?,
                ttl: parse_duration_arg(&args.ttl).map_err(|e| anyhow::anyhow!(e))?,
            },
            LinkSubcommand::Acquire(args) => Cmd::Acquire {
                paths: require_paths(merge_paths(args.paths, args.flag_paths))?,
                ttl: parse_duration_arg(&args.ttl).map_err(|e| anyhow::anyhow!(e))?,
                strict: args.strict,
                format: args.format,
            },
            LinkSubcommand::Release(args) => Cmd::Release {
                paths: require_paths(merge_paths(args.paths, args.flag_paths))?,
            },
            LinkSubcommand::Claims(args) => Cmd::Claims {
                path_prefix: args
                    .path_prefix
                    .map(|p| {
                        let trimmed = p.trim();
                        anyhow::ensure!(!trimmed.is_empty(), "path-prefix must not be empty");
                        Ok(trimmed.to_owned())
                    })
                    .transpose()?,
            },
            LinkSubcommand::Check(args) => Cmd::Check {
                paths: require_paths(merge_paths(args.paths, args.flag_paths))?,
                strict: args.strict,
                format: args.format,
            },
            LinkSubcommand::Post(args) => {
                if let Some(kind) = args.kind {
                    anyhow::ensure!(
                        args.message.len() == 1,
                        "typed post requires exactly one JSON argument"
                    );
                    Cmd::PostTyped {
                        kind,
                        json: args.message[0].clone(),
                    }
                } else {
                    anyhow::ensure!(!args.message.is_empty(), "message required");
                    Cmd::Post {
                        message: args.message.join(" "),
                    }
                }
            }
            LinkSubcommand::Read(args) => Cmd::Read {
                view: resolve_read_view(args.view, args.legacy_kind.as_deref())?,
                since: args.since,
            },
            LinkSubcommand::Brief(args) => Cmd::Brief {
                kind: args.kind,
                all: args.all,
            },
            LinkSubcommand::Digest(args) => Cmd::Digest {
                kind: args.kind,
                all: args.all,
            },
            LinkSubcommand::Status(args) => Cmd::Status {
                value: parse_status_plan_value(&args)?,
            },
            LinkSubcommand::Plan(args) => Cmd::Plan {
                value: parse_status_plan_value(&args)?,
            },
            LinkSubcommand::Agents => Cmd::Agents,
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
                    paths: merge_paths(Vec::new(), args.paths),
                }
            }
            LinkSubcommand::Declare(args) => {
                anyhow::ensure!(!args.scope.is_empty(), "scope required");
                Cmd::Declare {
                    scope: args.scope.join(" "),
                    tags: args.tag,
                    surface: args.surface,
                    paths: merge_paths(Vec::new(), args.paths),
                }
            }
            LinkSubcommand::Block(args) => Cmd::Block {
                paths: require_paths(merge_paths(args.paths, args.flag_paths))?,
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
                paths: merge_paths(args.paths, args.flag_paths),
                note: args.note,
            },
            LinkSubcommand::Eval(args) => match args.cmd {
                EvalSubcommand::UserPromptSubmit => Cmd::EvalUserPromptSubmit,
            },
        };

        let agent_id = match self.agent_id {
            Some(id) => id,
            None if matches!(cmd, Cmd::Agents | Cmd::Claims { .. } | Cmd::Tui) => {
                OBSERVER_AGENT_ID.to_owned()
            }
            None => anyhow::bail!("--agent-id required"),
        };

        Ok((agent_id, cmd))
    }
}

/// Require at least one parsed path.
fn require_paths(paths: Vec<String>) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(!paths.is_empty(), "at least one path required");
    Ok(paths)
}

/// Merge positional paths and `--path` flag paths, splitting comma-separated values.
fn merge_paths(positional: Vec<String>, flag: Vec<String>) -> Vec<String> {
    positional
        .into_iter()
        .chain(flag)
        .flat_map(|s| {
            s.split(',')
                .map(|p| p.trim().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|s| !s.is_empty())
        .collect()
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

/// Resolve the requested read view, supporting the legacy `--type` flag.
fn resolve_read_view(
    view: Option<ReadView>,
    legacy_kind: Option<&str>,
) -> anyhow::Result<ReadView> {
    if let Some(view) = view {
        return Ok(view);
    }
    Ok(match legacy_kind.unwrap_or("inbox") {
        "all" => ReadView::Full,
        "discovery" => ReadView::Discoveries,
        "message" => ReadView::Messages,
        "claim" | "release" | "claims" => ReadView::Claims,
        "agents" => ReadView::Agents,
        "full" => ReadView::Full,
        "messages" => ReadView::Messages,
        "discoveries" => ReadView::Discoveries,
        "inbox" => ReadView::Inbox,
        other => anyhow::bail!("unsupported read view: {other}"),
    })
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
fn parse_duration(s: &str) -> Result<Duration, ()> {
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
        Cmd::Claim { .. } => "claim",
        Cmd::Acquire { .. } => "acquire",
        Cmd::Release { .. } => "release",
        Cmd::Claims { .. } => "claims",
        Cmd::Check { paths, .. } if paths.len() > 1 => "check-batch",
        Cmd::Check { .. } => "check",
        Cmd::Post { .. } => "post",
        Cmd::PostTyped { .. } => "post-typed",
        Cmd::Read { .. } => "read",
        Cmd::Brief { .. } => "brief",
        Cmd::Digest { .. } => "digest",
        Cmd::Status { .. } => "status",
        Cmd::Plan { .. } => "plan",
        Cmd::Agents => "agents",
        Cmd::Tui => "tui",
        Cmd::Done { .. } => "done",
        Cmd::Discovery { .. } => "discovery",
        Cmd::Intent { .. } => "intent",
        Cmd::Declare { .. } => "declare",
        Cmd::Block { .. } => "block",
        Cmd::Resolve { .. } => "resolve",
        Cmd::Ack { .. } => "ack",
        Cmd::EvalUserPromptSubmit => "eval-user-prompt-submit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use clap::error::ErrorKind;

    #[test]
    fn normalize_claim_path_strips_dot_slash() -> anyhow::Result<()> {
        assert_eq!(normalize_claim_path("./src/foo.rs")?, "src/foo.rs");
        Ok(())
    }

    #[test]
    fn normalize_claim_path_strips_trailing_slash() -> anyhow::Result<()> {
        assert_eq!(normalize_claim_path("src/")?, "src");
        Ok(())
    }

    #[test]
    fn normalize_claim_path_collapses_dotdot() -> anyhow::Result<()> {
        assert_eq!(normalize_claim_path("src/lib/../foo.rs")?, "src/foo.rs");
        Ok(())
    }

    #[test]
    fn normalize_claim_path_identity() -> anyhow::Result<()> {
        assert_eq!(normalize_claim_path("src/foo.rs")?, "src/foo.rs");
        Ok(())
    }

    #[test]
    fn normalize_claim_path_rejects_absolute_paths() {
        assert!(normalize_claim_path("/tmp/foo.rs").is_err());
    }

    #[test]
    fn normalize_claim_path_rejects_paths_outside_repo() {
        assert!(normalize_claim_path("../foo.rs").is_err());
    }

    #[test]
    fn parse_check_single_path() {
        let plat = Platform::parse_from([
            "but-link",
            "check",
            "--path",
            "src/foo.rs",
            "--agent-id",
            "a1",
        ]);
        let (agent_id, cmd) = plat.into_runtime().unwrap();
        assert_eq!(agent_id, "a1");
        match cmd {
            Cmd::Check {
                paths,
                strict,
                format,
            } => {
                assert_eq!(paths, vec!["src/foo.rs"]);
                assert!(!strict);
                assert_eq!(format, CheckFormat::Full);
            }
            _ => panic!("expected Check"),
        }
    }

    #[test]
    fn parse_acquire_multiple_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "acquire",
            "--path",
            "src/a.rs",
            "--path",
            "src/b.rs",
            "--ttl",
            "15m",
            "--strict",
            "--format",
            "compact",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Acquire {
                paths,
                ttl,
                strict,
                format,
            } => {
                assert_eq!(paths, vec!["src/a.rs", "src/b.rs"]);
                assert_eq!(ttl, Duration::from_secs(900));
                assert!(strict);
                assert_eq!(format, CheckFormat::Compact);
            }
            _ => panic!("expected Acquire"),
        }
    }

    #[test]
    fn parse_claim_positional_with_default_ttl() {
        let plat = Platform::parse_from(["but-link", "claim", "src/foo.rs", "--agent-id", "a1"]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Claim { paths, ttl } => {
                assert_eq!(paths, vec!["src/foo.rs"]);
                assert_eq!(ttl, Duration::from_secs(300));
            }
            _ => panic!("expected Claim"),
        }
    }

    #[test]
    fn parse_claims_absolute_path_prefix_is_preserved_for_runtime_normalization() {
        let plat = Platform::parse_from(["but-link", "claims", "--path-prefix", "/tmp/repo/src"]);
        let (agent_id, cmd) = plat.into_runtime().unwrap();
        assert_eq!(agent_id, OBSERVER_AGENT_ID);
        match cmd {
            Cmd::Claims { path_prefix } => {
                assert_eq!(path_prefix.as_deref(), Some("/tmp/repo/src"));
            }
            _ => panic!("expected Claims"),
        }
    }

    #[test]
    fn parse_read_defaults_to_inbox() {
        let plat = Platform::parse_from(["but-link", "read", "--agent-id", "a1"]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Read { view, since } => {
                assert_eq!(view, ReadView::Inbox);
                assert!(since.is_none());
            }
            _ => panic!("expected Read"),
        }
    }

    #[test]
    fn parse_read_legacy_type_maps_to_view() {
        let plat = Platform::parse_from(["but-link", "read", "--type", "all", "--agent-id", "a1"]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Read { view, .. } => assert_eq!(view, ReadView::Full),
            _ => panic!("expected Read"),
        }
    }

    #[test]
    fn parse_block_subcommand() {
        let plat = Platform::parse_from([
            "but-link",
            "block",
            "src/foo.rs",
            "--reason",
            "shared refactor",
            "--mode",
            "hard",
            "--ttl",
            "10m",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Block {
                paths,
                reason,
                mode,
                ttl,
            } => {
                assert_eq!(paths, vec!["src/foo.rs"]);
                assert_eq!(reason, "shared refactor");
                assert_eq!(mode, BlockMode::Hard);
                assert_eq!(ttl, Some(Duration::from_secs(600)));
            }
            _ => panic!("expected Block"),
        }
    }

    #[test]
    fn parse_ack_subcommand() {
        let plat = Platform::parse_from([
            "but-link",
            "ack",
            "--agent",
            "peer-a",
            "--path",
            "src/foo.rs",
            "--note",
            "saw it",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Ack {
                target_agent_id,
                paths,
                note,
            } => {
                assert_eq!(target_agent_id, "peer-a");
                assert_eq!(paths, vec!["src/foo.rs"]);
                assert_eq!(note.as_deref(), Some("saw it"));
            }
            _ => panic!("expected Ack"),
        }
    }

    #[test]
    fn parse_intent_subcommand_with_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "intent",
            "crate::auth",
            "--tag",
            "api",
            "--surface",
            "AuthToken",
            "--path",
            "src/auth.rs",
            "--agent-id",
            "B",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Intent {
                scope,
                tags,
                surface,
                paths,
            } => {
                assert_eq!(scope, "crate::auth");
                assert_eq!(tags, vec!["api"]);
                assert_eq!(surface, vec!["AuthToken"]);
                assert_eq!(paths, vec!["src/auth.rs"]);
            }
            _ => panic!("expected Intent"),
        }
    }

    #[test]
    fn parse_tui_without_agent_id() {
        let plat = Platform::parse_from(["but-link", "tui"]);
        let (agent_id, cmd) = plat.into_runtime().unwrap();
        assert_eq!(agent_id, OBSERVER_AGENT_ID);
        assert!(matches!(cmd, Cmd::Tui));
    }

    #[test]
    fn help_hides_internal_eval_subcommand() {
        let mut cmd = Platform::command();
        let help = cmd.render_long_help().to_string();

        assert!(help.contains("claim"));
        assert!(!help.contains("eval"));
    }

    #[test]
    fn parse_duration_variants() {
        assert_eq!(parse_duration("15m").unwrap(), Duration::from_secs(900));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("2d").unwrap(), Duration::from_secs(172_800));
        assert_eq!(parse_duration("60").unwrap(), Duration::from_secs(60));
    }

    #[test]
    fn parse_help_long_flag() {
        let err = Platform::try_parse_from(["but-link", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn parse_help_short_h_flag() {
        let err = Platform::try_parse_from(["but-link", "-H"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }
}

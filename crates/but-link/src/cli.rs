//! CLI argument types and parsing for `but link`.

use std::time::Duration;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub(crate) const OBSERVER_AGENT_ID: &str = "tier4-observer";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum CheckFormat {
    Full,
    Compact,
}

#[derive(Debug)]
pub(crate) enum Cmd {
    Claim {
        paths: Vec<String>,
        ttl: Duration,
    },
    Release {
        paths: Vec<String>,
    },
    Claims {
        path_prefix: Option<String>,
    },
    Check {
        paths: Vec<String>,
        strict: bool,
        format: CheckFormat,
    },
    Post {
        message: String,
    },
    PostTyped {
        kind: String,
        json: String,
    },
    Read {
        kind: Option<String>,
        since: Option<i64>,
    },
    Brief {
        kind: Option<String>,
        all: bool,
    },
    Digest {
        kind: Option<String>,
        all: bool,
    },
    Status {
        value: Option<String>,
    },
    Plan {
        value: Option<String>,
    },
    Agents,
    Tui,
    Done {
        summary: String,
    },
    Discovery {
        title: String,
        evidence: Vec<String>,
        action: String,
        signal: Option<String>,
    },
    Intent {
        scope: String,
        tags: Vec<String>,
        surface: Vec<String>,
    },
    Declare {
        scope: String,
        tags: Vec<String>,
        surface: Vec<String>,
    },
    EvalUserPromptSubmit,
}

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

#[derive(Debug, clap::Subcommand)]
enum LinkSubcommand {
    Claim(ClaimArgs),
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
    Eval(EvalArgs),
}

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

#[derive(Debug, clap::Args)]
struct ReleaseArgs {
    /// Paths to release (positional, comma-separated OK).
    paths: Vec<String>,
    /// Backward-compat flag form.
    #[arg(long = "path", hide = true)]
    flag_paths: Vec<String>,
}

#[derive(Debug, clap::Args)]
struct ClaimsArgs {
    #[arg(long = "path-prefix")]
    path_prefix: Option<String>,
}

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

#[derive(Debug, clap::Args)]
struct PostArgs {
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(value_name = "MESSAGE")]
    message: Vec<String>,
}

#[derive(Debug, clap::Args)]
struct ReadArgs {
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(long, value_parser = parse_since_timestamp_arg)]
    since: Option<i64>,
}

#[derive(Debug, clap::Args)]
struct BriefDigestArgs {
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(long, default_value_t = false)]
    all: bool,
}

#[derive(Debug, clap::Args)]
struct StatusPlanArgs {
    #[arg(long, default_value_t = false)]
    clear: bool,
    #[arg(value_name = "VALUE")]
    value: Vec<String>,
}

#[derive(Debug, clap::Args)]
struct DoneArgs {
    #[arg(value_name = "SUMMARY")]
    summary: Vec<String>,
}

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
}

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
}

#[derive(Debug, clap::Args)]
struct EvalArgs {
    #[command(subcommand)]
    cmd: EvalSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum EvalSubcommand {
    #[command(name = "user-prompt-submit")]
    UserPromptSubmit,
}

impl Platform {
    pub(crate) fn into_runtime(self) -> anyhow::Result<(String, Cmd)> {
        let cmd = match self.cmd.context("missing subcommand")? {
            LinkSubcommand::Claim(args) => {
                let paths = merge_paths(args.paths, args.flag_paths);
                anyhow::ensure!(!paths.is_empty(), "at least one path required");
                Cmd::Claim {
                    paths,
                    ttl: parse_duration_arg(&args.ttl).map_err(|e| anyhow::anyhow!(e))?,
                }
            }
            LinkSubcommand::Release(args) => {
                let paths = merge_paths(args.paths, args.flag_paths);
                anyhow::ensure!(!paths.is_empty(), "at least one path required");
                Cmd::Release { paths }
            }
            LinkSubcommand::Claims(args) => {
                let path_prefix = args
                    .path_prefix
                    .map(|p| {
                        let trimmed = p.trim();
                        anyhow::ensure!(!trimmed.is_empty(), "path-prefix must not be empty");
                        Ok(trimmed.to_owned())
                    })
                    .transpose()?;
                Cmd::Claims { path_prefix }
            }
            LinkSubcommand::Check(args) => {
                let paths = merge_paths(args.paths, args.flag_paths);
                anyhow::ensure!(!paths.is_empty(), "at least one path required");
                Cmd::Check {
                    paths,
                    strict: args.strict,
                    format: args.format,
                }
            }
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
                kind: args.kind,
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
                }
            }
            LinkSubcommand::Declare(args) => {
                anyhow::ensure!(!args.scope.is_empty(), "scope required");
                Cmd::Declare {
                    scope: args.scope.join(" "),
                    tags: args.tag,
                    surface: args.surface,
                }
            }
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

use anyhow::Context;

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

fn parse_non_empty_string(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        Err("value must not be empty".to_owned())
    } else {
        Ok(raw.to_owned())
    }
}

fn parse_duration_arg(raw: &str) -> Result<Duration, String> {
    parse_duration(raw).map_err(|()| format!("invalid duration: {raw}"))
}

fn parse_since_timestamp_arg(raw: &str) -> Result<i64, String> {
    parse_since_timestamp(raw).map_err(|()| format!("invalid timestamp: {raw}"))
}

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

fn parse_status_plan_value(args: &StatusPlanArgs) -> anyhow::Result<Option<String>> {
    if args.clear {
        anyhow::ensure!(args.value.is_empty(), "cannot provide value with --clear");
        return Ok(None);
    }
    anyhow::ensure!(!args.value.is_empty(), "value required");
    Ok(Some(args.value.join(" ")))
}

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
    fn parse_check_multiple_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "check",
            "--path",
            "src/a.rs",
            "--path",
            "src/b.rs",
            "--path",
            "src/c.rs",
            "--format",
            "compact",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Check { paths, format, .. } => {
                assert_eq!(paths, vec!["src/a.rs", "src/b.rs", "src/c.rs"]);
                assert_eq!(format, CheckFormat::Compact);
            }
            _ => panic!("expected Check"),
        }
    }

    #[test]
    fn parse_check_no_path_errors() {
        let plat = Platform::parse_from(["but-link", "check", "--agent-id", "a1"]);
        assert!(plat.into_runtime().is_err());
    }

    #[test]
    fn parse_check_positional_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "check",
            "src/foo.rs",
            "src/bar.rs",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Check { paths, .. } => {
                assert_eq!(paths, vec!["src/foo.rs", "src/bar.rs"]);
            }
            _ => panic!("expected Check"),
        }
    }

    #[test]
    fn parse_check_comma_separated_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "check",
            "src/foo.rs,src/bar.rs",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Check { paths, .. } => {
                assert_eq!(paths, vec!["src/foo.rs", "src/bar.rs"]);
            }
            _ => panic!("expected Check"),
        }
    }

    #[test]
    fn parse_claim_positional_with_default_ttl() {
        let plat = Platform::parse_from(["but-link", "claim", "src/foo.rs", "--agent-id", "a1"]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Claim { paths, ttl } => {
                assert_eq!(paths, vec!["src/foo.rs"]);
                assert_eq!(ttl, Duration::from_secs(300)); // 5m default
            }
            _ => panic!("expected Claim"),
        }
    }

    #[test]
    fn parse_claim_comma_separated_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "claim",
            "src/a.rs,src/b.rs",
            "--ttl",
            "10m",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Claim { paths, ttl } => {
                assert_eq!(paths, vec!["src/a.rs", "src/b.rs"]);
                assert_eq!(ttl, Duration::from_secs(600));
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
    fn parse_release_positional_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "release",
            "src/a.rs,src/b.rs",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Release { paths } => {
                assert_eq!(paths, vec!["src/a.rs", "src/b.rs"]);
            }
            _ => panic!("expected Release"),
        }
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
    fn parse_duration_invalid() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("15x").is_err());
    }

    #[test]
    fn parse_claim_multiple_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "claim",
            "--path",
            "src/a.rs",
            "--path",
            "src/b.rs",
            "--ttl",
            "15m",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Claim { paths, ttl } => {
                assert_eq!(paths, vec!["src/a.rs", "src/b.rs"]);
                assert_eq!(ttl, Duration::from_secs(900));
            }
            _ => panic!("expected Claim"),
        }
    }

    #[test]
    fn parse_release_multiple_paths() {
        let plat = Platform::parse_from([
            "but-link",
            "release",
            "--path",
            "src/a.rs",
            "--path",
            "src/b.rs",
            "--agent-id",
            "a1",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Release { paths } => {
                assert_eq!(paths, vec!["src/a.rs", "src/b.rs"]);
            }
            _ => panic!("expected Release"),
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
    fn parse_tui_with_agent_id() {
        let plat = Platform::parse_from(["but-link", "tui", "--agent-id", "a1"]);
        let (agent_id, cmd) = plat.into_runtime().unwrap();
        assert_eq!(agent_id, "a1");
        assert!(matches!(cmd, Cmd::Tui));
    }

    #[test]
    fn parse_discovery_subcommand() {
        let plat = Platform::parse_from([
            "but-link",
            "discovery",
            "breaking",
            "rename",
            "--evidence",
            "AuthToken renamed",
            "--evidence",
            "callers not updated",
            "--action",
            "but link check src/types.rs --agent-id A",
            "--agent-id",
            "A",
        ]);
        let (agent_id, cmd) = plat.into_runtime().unwrap();
        assert_eq!(agent_id, "A");
        match cmd {
            Cmd::Discovery {
                title,
                evidence,
                action,
                signal,
            } => {
                assert_eq!(title, "breaking rename");
                assert_eq!(evidence.len(), 2);
                assert_eq!(action, "but link check src/types.rs --agent-id A");
                assert!(signal.is_none());
            }
            _ => panic!("expected Discovery"),
        }
    }

    #[test]
    fn parse_intent_subcommand() {
        let plat = Platform::parse_from([
            "but-link",
            "intent",
            "crate::auth",
            "--tag",
            "api",
            "--surface",
            "AuthToken",
            "--surface",
            "verify_token",
            "--agent-id",
            "B",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Intent {
                scope,
                tags,
                surface,
            } => {
                assert_eq!(scope, "crate::auth");
                assert_eq!(tags, vec!["api"]);
                assert_eq!(surface, vec!["AuthToken", "verify_token"]);
            }
            _ => panic!("expected Intent"),
        }
    }

    #[test]
    fn parse_declare_subcommand() {
        let plat = Platform::parse_from([
            "but-link",
            "declare",
            "crate::auth",
            "--tag",
            "api",
            "--tag",
            "public",
            "--surface",
            "AuthToken",
            "--agent-id",
            "C",
        ]);
        let (_, cmd) = plat.into_runtime().unwrap();
        match cmd {
            Cmd::Declare {
                scope,
                tags,
                surface,
            } => {
                assert_eq!(scope, "crate::auth");
                assert_eq!(tags, vec!["api", "public"]);
                assert_eq!(surface, vec!["AuthToken"]);
            }
            _ => panic!("expected Declare"),
        }
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

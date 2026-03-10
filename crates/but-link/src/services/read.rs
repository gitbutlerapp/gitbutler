//! Service wrappers for read-side JSON output and TUI snapshots.

use rusqlite::Connection;
use serde_json::Value;

use crate::cli::DiscoveryFormat;
use crate::db;
use crate::read;

/// Validate `read` flag combinations before dispatch.
pub(crate) fn validate_read_args(
    view: crate::cli::ReadView,
    format: DiscoveryFormat,
    has_since: bool,
) -> anyhow::Result<()> {
    if view != crate::cli::ReadView::Discoveries {
        anyhow::ensure!(
            format == DiscoveryFormat::Full,
            "--format only applies to --view discoveries"
        );
    }

    if has_since {
        anyhow::ensure!(
            matches!(
                view,
                crate::cli::ReadView::Discoveries | crate::cli::ReadView::Messages
            ),
            "--since is only supported for --view discoveries or --view messages"
        );
        anyhow::ensure!(
            format == DiscoveryFormat::Full,
            "--format is not supported with --since; incremental reads always use full payloads"
        );
    }

    Ok(())
}

/// Build a JSON response for the requested read view.
pub(crate) fn read_view_json(
    conn: &Connection,
    agent_id: &str,
    view: crate::cli::ReadView,
    format: DiscoveryFormat,
) -> anyhow::Result<Value> {
    validate_read_args(view, format, false)?;

    match view {
        crate::cli::ReadView::Inbox => read::inbox_view(conn, agent_id),
        crate::cli::ReadView::Full => read::full_view(conn),
        crate::cli::ReadView::Discoveries => read::discoveries_view_with_format(conn, format),
        crate::cli::ReadView::Messages => read::messages_view(conn),
        crate::cli::ReadView::Claims => read::claims_view(conn),
        crate::cli::ReadView::Agents => read::agents_view(conn),
    }
}

/// Load incremental rows for `read --since`.
pub(crate) fn read_since_json(
    conn: &Connection,
    view: crate::cli::ReadView,
    since_ms: i64,
) -> anyhow::Result<Value> {
    match view {
        crate::cli::ReadView::Discoveries => {
            Ok(Value::Array(db::load_discoveries_since(conn, since_ms)?))
        }
        crate::cli::ReadView::Messages => Ok(Value::Array(db::load_messages_since(
            conn,
            Some("message"),
            since_ms,
        )?)),
        _ => anyhow::bail!("--since is only supported for --view discoveries or --view messages"),
    }
}

/// Build a shared TUI snapshot.
pub(crate) fn tui_snapshot(
    conn: &Connection,
    since_ms: i64,
    message_limit: i64,
    now_ms: i64,
) -> anyhow::Result<read::TuiSnapshot> {
    read::load_tui_snapshot(conn, since_ms, message_limit, now_ms)
}

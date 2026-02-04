//! Read command implementation.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::db::{DbHandle, is_transient_error};
use crate::duration::parse_duration;
use crate::types::{Message, validate_agent_id};

/// Polling interval for wait mode.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Short initial delays for transient lock contention before settling into
/// the steady polling interval.
const FAST_RETRY_0: Duration = Duration::from_millis(40);
const FAST_RETRY_1: Duration = Duration::from_millis(80);
const FAST_RETRY_2: Duration = Duration::from_millis(120);

/// Default time window for first read (1 hour).
const DEFAULT_FIRST_READ_WINDOW: Duration = Duration::from_secs(60 * 60);

/// Maximum messages to return on first read.
const MAX_FIRST_READ_MESSAGES: usize = 50;

/// Maximum messages to return when no limit specified (prevents unbounded queries).
const MAX_DEFAULT_MESSAGES: usize = 1000;

/// Read messages from the shared channel.
pub fn execute(
    db_path: &Path,
    agent_id: String,
    since: Option<String>,
    unread: bool,
    wait: bool,
    timeout: Option<String>,
) -> anyhow::Result<Vec<Message>> {
    validate_agent_id(&agent_id)?;

    // Parse timeout (None means wait indefinitely)
    let timeout = match timeout {
        Some(ref t) => Some(parse_duration(t)?),
        None => None,
    };

    // Determine the "since" timestamp
    let since_time = determine_since_time(db_path, &agent_id, since.as_deref(), unread)?;

    if wait {
        poll_for_messages(db_path, &agent_id, since_time, timeout)
    } else {
        read_messages_once(db_path, &agent_id, since_time)
    }
}

fn determine_since_time(
    db_path: &Path,
    agent_id: &str,
    since: Option<&str>,
    unread: bool,
) -> anyhow::Result<(DateTime<Utc>, bool)> {
    // If explicit --since is provided, use it (lenient: bad timestamps default to epoch)
    if let Some(since_str) = since {
        let ts = DateTime::parse_from_rfc3339(since_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(DateTime::UNIX_EPOCH);
        return Ok((ts, false));
    }

    // If --unread (default), try to use agent's last_read
    if unread {
        let db = DbHandle::new_at_path(db_path)?;
        if let Some(agent) = db.get_agent(agent_id)?
            && let Some(last_read) = agent.last_read
        {
            return Ok((last_read, false));
        }
    }

    // First read: use last hour, but limit messages
    let since = Utc::now() - chrono::Duration::from_std(DEFAULT_FIRST_READ_WINDOW)?;
    Ok((since, true))
}

fn read_messages_once(
    db_path: &Path,
    agent_id: &str,
    since_info: (DateTime<Utc>, bool),
) -> anyhow::Result<Vec<Message>> {
    let (since_time, is_first_read) = since_info;
    let db = DbHandle::new_at_path(db_path)?;

    let messages = if is_first_read {
        db.query_recent_messages(since_time, MAX_FIRST_READ_MESSAGES)?
    } else {
        db.query_messages_since(since_time, Some(MAX_DEFAULT_MESSAGES))?
    };

    // Update last_read to now (best-effort: don't lose messages on update failure)
    update_last_read_best_effort(&db, agent_id);

    Ok(messages)
}

/// Update the agent's last_read timestamp. Failures are ignored to avoid
/// losing already-retrieved messages. Worst case: agent sees duplicates.
fn update_last_read_best_effort(db: &DbHandle, agent_id: &str) {
    let now = Utc::now();
    let _ = db.upsert_agent(agent_id, now);
    let _ = db.update_agent_last_read(agent_id, now);
}

fn poll_for_messages(
    db_path: &Path,
    agent_id: &str,
    since_info: (DateTime<Utc>, bool),
    timeout: Option<Duration>,
) -> anyhow::Result<Vec<Message>> {
    let (mut since_time, is_first_read) = since_info;
    let start = Instant::now();
    let mut transient_retry_attempt: u32 = 0;

    // On first iteration with first_read, check for recent messages
    let mut first_iteration = true;

    loop {
        // Open fresh connection each iteration to see WAL writes from other processes
        let db = match DbHandle::new_at_path(db_path) {
            Ok(db) => db,
            Err(e) => {
                // Retry on transient errors (database locked/busy)
                if let Some(sqlite_err) = e.downcast_ref::<rusqlite::Error>()
                    && is_transient_error(sqlite_err)
                {
                    thread::sleep(transient_retry_delay(transient_retry_attempt));
                    transient_retry_attempt = transient_retry_attempt.saturating_add(1);
                    continue;
                }
                return Err(e);
            }
        };

        let messages_result = if first_iteration && is_first_read {
            let result = db.query_recent_messages(since_time, MAX_FIRST_READ_MESSAGES);
            // Only mark first iteration complete and update since_time after successful query
            // This ensures we retry with query_recent_messages if transient error occurs
            if result.is_ok() {
                first_iteration = false;
                since_time = Utc::now();
            }
            result
        } else {
            db.query_messages_since(since_time, Some(MAX_DEFAULT_MESSAGES))
        };

        let messages = match messages_result {
            Ok(m) => m,
            Err(e) if is_transient_error(&e) => {
                // Retry on transient errors
                thread::sleep(transient_retry_delay(transient_retry_attempt));
                transient_retry_attempt = transient_retry_attempt.saturating_add(1);
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        transient_retry_attempt = 0;

        if !messages.is_empty() {
            // Update last_read (best-effort: don't lose messages on update failure)
            update_last_read_best_effort(&db, agent_id);
            return Ok(messages);
        }

        // Check timeout (None means wait indefinitely)
        if let Some(t) = timeout
            && start.elapsed() >= t
        {
            // Update last_read even on timeout (best-effort)
            update_last_read_best_effort(&db, agent_id);
            return Ok(vec![]);
        }

        thread::sleep(POLL_INTERVAL);
    }
}

fn transient_retry_delay(attempt: u32) -> Duration {
    match attempt {
        0 => FAST_RETRY_0,
        1 => FAST_RETRY_1,
        2 => FAST_RETRY_2,
        _ => POLL_INTERVAL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_retry_delay_fast_then_caps() {
        assert_eq!(transient_retry_delay(0), FAST_RETRY_0);
        assert_eq!(transient_retry_delay(1), FAST_RETRY_1);
        assert_eq!(transient_retry_delay(2), FAST_RETRY_2);
        assert_eq!(transient_retry_delay(3), POLL_INTERVAL);
        assert_eq!(transient_retry_delay(12), POLL_INTERVAL);
    }
}

use std::time;

use anyhow::Result;

use crate::app::sessions;

use super::check_current_session::should_flush;

const FIVE_MINUTES: time::Duration = time::Duration::new(5 * 60, 0);
const ONE_HOUR: time::Duration = time::Duration::new(60 * 60, 0);

#[test]
fn test_should_flush() -> Result<()> {
    let now = time::SystemTime::now();
    let start = now;
    let last = now;

    let session = sessions::Session {
        id: "session-id".to_string(),
        hash: None,
        meta: sessions::Meta {
            start_timestamp_ms: start.duration_since(time::UNIX_EPOCH)?.as_millis(),
            last_timestamp_ms: last.duration_since(time::UNIX_EPOCH)?.as_millis(),
            branch: None,
            commit: None,
        },
    };

    assert!(!should_flush(now, &session)?);

    assert!(should_flush(start + FIVE_MINUTES, &session)?);
    assert!(should_flush(last + ONE_HOUR, &session)?);

    Ok(())
}

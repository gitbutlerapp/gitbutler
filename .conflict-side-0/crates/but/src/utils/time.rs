//! Time formatting utilities for the but CLI.

/// Format a Unix timestamp (in seconds) as a relative time string (e.g., "2 days ago", "5m ago") from `now`.
///
/// This uses a compact format suitable for commit timestamps and status displays.
pub fn format_relative_time(now: std::time::SystemTime, timestamp_seconds: i64) -> String {
    let now = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let seconds_ago = now.saturating_sub(timestamp_seconds);

    if seconds_ago < 60 {
        format!("{seconds_ago}s ago")
    } else if seconds_ago < 3600 {
        format!("{}m ago", seconds_ago / 60)
    } else if seconds_ago < 86400 {
        format!("{}h ago", seconds_ago / 3600)
    } else if seconds_ago < 604800 {
        let days = seconds_ago / 86400;
        if days == 1 {
            "yesterday".to_string()
        } else {
            format!("{days}d ago")
        }
    } else if seconds_ago < 2592000 {
        format!("{}w ago", seconds_ago / 604800)
    } else if seconds_ago < 31536000 {
        format!("{}mo ago", seconds_ago / 2592000)
    } else {
        format!("{}y ago", seconds_ago / 31536000)
    }
}

/// Format a Unix timestamp (in milliseconds) as a relative time string with verbose formatting from `now`.
///
/// This uses a more verbose format suitable for status displays where clarity is preferred
/// (e.g., "2 days ago", "5 minutes ago").
pub fn format_relative_time_verbose(now: std::time::SystemTime, timestamp_ms: u128) -> String {
    let now_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let elapsed_ms = now_ms.saturating_sub(timestamp_ms);
    let elapsed_secs = elapsed_ms / 1000;

    if elapsed_secs < 60 {
        format!("{elapsed_secs} seconds ago")
    } else if elapsed_secs < 3600 {
        let minutes = elapsed_secs / 60;
        format!(
            "{} {} ago",
            minutes,
            if minutes == 1 { "minute" } else { "minutes" }
        )
    } else if elapsed_secs < 86400 {
        let hours = elapsed_secs / 3600;
        format!(
            "{} {} ago",
            hours,
            if hours == 1 { "hour" } else { "hours" }
        )
    } else {
        let days = elapsed_secs / 86400;
        format!("{} {} ago", days, if days == 1 { "day" } else { "days" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_relative_time_seconds() {
        let now_t = std::time::SystemTime::now();
        let now = now_t
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert_eq!(format_relative_time(now_t, now - 30), "30s ago");
        assert_eq!(format_relative_time(now_t, now - 59), "59s ago");
    }

    #[test]
    fn format_relative_time_minutes() {
        let now_t = std::time::SystemTime::now();
        let now = now_t
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert_eq!(format_relative_time(now_t, now - 60), "1m ago");
        assert_eq!(format_relative_time(now_t, now - 120), "2m ago");
        assert_eq!(format_relative_time(now_t, now - 3599), "59m ago");
    }

    #[test]
    fn format_relative_time_hours() {
        let now_t = std::time::SystemTime::now();
        let now = now_t
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert_eq!(format_relative_time(now_t, now - 3600), "1h ago");
        assert_eq!(format_relative_time(now_t, now - 7200), "2h ago");
    }

    #[test]
    fn format_relative_time_days() {
        let now_t = std::time::SystemTime::now();
        let now = now_t
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert_eq!(format_relative_time(now_t, now - 86400), "yesterday");
        assert_eq!(format_relative_time(now_t, now - 172800), "2d ago");
    }

    #[test]
    fn format_relative_time_verbose_journey() {
        let now = std::time::SystemTime::now();
        let now_ms = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        assert_eq!(
            format_relative_time_verbose(now, now_ms - 30_000),
            "30 seconds ago"
        );
        assert_eq!(
            format_relative_time_verbose(now, now_ms - 60_000),
            "1 minute ago"
        );
        assert_eq!(
            format_relative_time_verbose(now, now_ms - 120_000),
            "2 minutes ago"
        );
        assert_eq!(
            format_relative_time_verbose(now, now_ms - 3_600_000),
            "1 hour ago"
        );
        assert_eq!(
            format_relative_time_verbose(now, now_ms - 7_200_000),
            "2 hours ago"
        );
        assert_eq!(
            format_relative_time_verbose(now, now_ms - 86_400_000),
            "1 day ago"
        );
        assert_eq!(
            format_relative_time_verbose(now, now_ms - 172_800_000),
            "2 days ago"
        );
    }
}

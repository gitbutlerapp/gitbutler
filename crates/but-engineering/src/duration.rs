//! Duration parsing for human-readable formats like "30s", "5m", "1h".

use std::time::Duration;

/// Parse a duration string like "30s", "5m", "1h", "2d".
///
/// Supported suffixes:
/// - `ms` or `millis` or `millisecond` or `milliseconds` - milliseconds
/// - `s` or `sec` or `second` or `seconds` - seconds
/// - `m` or `min` or `minute` or `minutes` - minutes
/// - `h` or `hr` or `hour` or `hours` - hours
/// - `d` or `day` or `days` - days
///
/// If no suffix is provided, seconds are assumed.
pub fn parse_duration(s: &str) -> anyhow::Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("duration cannot be empty");
    }

    // Find where the number ends and the suffix begins
    let (num_part, suffix) = split_number_suffix(s);

    let num: u64 = num_part
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid number in duration: '{num_part}'"))?;

    let suffix_lower = suffix.to_lowercase();
    match suffix_lower.as_str() {
        "ms" | "millis" | "millisecond" | "milliseconds" => Ok(Duration::from_millis(num)),
        "" | "s" | "sec" | "second" | "seconds" => Ok(Duration::from_secs(num)),
        "m" | "min" | "minute" | "minutes" => Ok(Duration::from_secs(num * 60)),
        "h" | "hr" | "hour" | "hours" => Ok(Duration::from_secs(num * 60 * 60)),
        "d" | "day" | "days" => Ok(Duration::from_secs(num * 60 * 60 * 24)),
        _ => anyhow::bail!("unknown duration suffix: '{suffix}'"),
    }
}

fn split_number_suffix(s: &str) -> (&str, &str) {
    let idx = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    (&s[..idx], &s[idx..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_seconds() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("30sec").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("30second").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("30seconds").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("30").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn test_parse_minutes() {
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(5 * 60));
        assert_eq!(parse_duration("5min").unwrap(), Duration::from_secs(5 * 60));
        assert_eq!(parse_duration("5minute").unwrap(), Duration::from_secs(5 * 60));
        assert_eq!(parse_duration("5minutes").unwrap(), Duration::from_secs(5 * 60));
    }

    #[test]
    fn test_parse_hours() {
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(2 * 60 * 60));
        assert_eq!(parse_duration("2hr").unwrap(), Duration::from_secs(2 * 60 * 60));
        assert_eq!(parse_duration("2hour").unwrap(), Duration::from_secs(2 * 60 * 60));
        assert_eq!(parse_duration("2hours").unwrap(), Duration::from_secs(2 * 60 * 60));
    }

    #[test]
    fn test_parse_days() {
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(24 * 60 * 60));
        assert_eq!(parse_duration("1day").unwrap(), Duration::from_secs(24 * 60 * 60));
        assert_eq!(parse_duration("2days").unwrap(), Duration::from_secs(2 * 24 * 60 * 60));
    }

    #[test]
    fn test_parse_invalid() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("5x").is_err());
    }
}

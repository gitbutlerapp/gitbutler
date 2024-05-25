use std::time::UNIX_EPOCH;

/// Gets the number of milliseconds since the Unix epoch.
///
/// # Panics
/// Panics if the system time is set before the Unix epoch.
pub fn now_ms() -> u128 {
    UNIX_EPOCH
        .elapsed()
        .expect("system time is set before the Unix epoch")
        .as_millis()
}

pub fn now_since_unix_epoch_ms() -> i64 {
    UNIX_EPOCH
        .elapsed()
        .map(|d| i64::try_from(d.as_millis()).expect("no system date is this far in the future"))
        .unwrap_or_else(|_| {
            -i64::try_from(
                UNIX_EPOCH
                    .duration_since(std::time::SystemTime::now())
                    .expect("'now' is in the past")
                    .as_millis(),
            )
            .expect("no time is that far in the past")
        })
}

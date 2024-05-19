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

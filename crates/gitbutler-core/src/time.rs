use std::time::{Duration, UNIX_EPOCH};

/// Gets the duration of time since the Unix epoch.
///
/// # Panics
/// Panics if the system time is set before the Unix epoch.
pub fn now() -> Duration {
    UNIX_EPOCH
        .elapsed()
        .expect("system time is set before the Unix epoch")
}

/// Gets the number of milliseconds since the Unix epoch.
///
/// # Panics
/// Panics if the system time is set before the Unix epoch.
pub fn now_ms() -> u128 {
    now().as_millis()
}

pub mod duration_int_string_serde {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&duration.as_millis().to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis_str = String::deserialize(deserializer)?;
        let millis = millis_str
            .parse::<u64>()
            .map_err(serde::de::Error::custom)?;
        Ok(Duration::from_millis(millis))
    }
}

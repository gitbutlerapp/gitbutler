use chrono::Utc;

/// Gets the number of milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub mod duration_int_string_serde {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date_time: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&date_time.timestamp_millis().to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis_str = String::deserialize(deserializer)?;
        let millis = millis_str
            .parse::<i64>()
            .map_err(serde::de::Error::custom)?;
        DateTime::from_timestamp_millis(millis).ok_or(serde::de::Error::custom("Invalid timestamp"))
    }
}

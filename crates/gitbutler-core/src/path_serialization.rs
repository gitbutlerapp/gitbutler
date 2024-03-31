use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serializer};

pub fn wrap_path(path: &PathBuf) -> String {
    let encoded_path = format!("{}", serde_json::json!(path));
    let path = &encoded_path[1..encoded_path.len() - 1];
    path.to_string()
}

pub fn unwrap_path_str(path_str: &str) -> Result<String, String> {
    let path = format!("\"{}\"", path_str);
    match serde_json::from_str(&path) {
        Ok(serde_json::Value::String(path)) => Ok(path),
        _ => Err("failed to unwarp path string".into()),
    }
}

pub fn serialize<S>(path: &PathBuf, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    serializer.serialize_str(&wrap_path(path))
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
{
    let path_str = String::deserialize(deserializer)?;
    match unwrap_path_str(&path_str) {
        Ok(path) => Ok(path.into()),
        _ => Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(&path_str),
            &"failed to parse path",
        )),
    }
}

pub mod option {
    use std::path::PathBuf;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(maybe_path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        match maybe_path {
            Some(path) => super::serialize(path, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
        where
            D: Deserializer<'de>,
    {
        match Option::<String>::deserialize(deserializer)? {
            Some(path_str) => match super::unwrap_path_str(&path_str) {
                Ok(path) => Ok(Some(path.into())),
                _ => Err(serde::de::Error::invalid_value(
                    serde::de::Unexpected::Str(&path_str),
                    &"failed to parse path",
                )),
            },
            None => Ok(None),
        }
    }
}

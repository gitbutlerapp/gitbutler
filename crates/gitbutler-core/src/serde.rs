use crate::virtual_branches::branch::HunkHash;
use bstr::{BString, ByteSlice};
use serde::Serialize;

pub fn as_string_lossy<S>(v: &BString, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    v.to_str_lossy().serialize(s)
}

pub fn hash_to_hex<S>(v: &HunkHash, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    format!("{v:x}").serialize(s)
}

pub mod path {
    use std::path::{Path, PathBuf};

    use anyhow::anyhow;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn json_escape(path: &Path) -> String {
        let encoded_path = format!("{}", serde_json::json!(path));
        let path = &encoded_path[1..encoded_path.len() - 1];
        path.to_string()
    }

    pub fn json_unescape(path_str: &str) -> Result<String, anyhow::Error> {
        let path = format!("\"{}\"", path_str);
        match serde_json::from_str(&path) {
            Ok(serde_json::Value::String(path)) => Ok(path),
            _ => Err(anyhow!("failed to convert to path: {}", path_str)),
        }
    }

    pub fn serialize<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&json_escape(path))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path_str = String::deserialize(deserializer)?;
        match json_unescape(&path_str) {
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
                Some(path_str) => match super::json_unescape(&path_str) {
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
}

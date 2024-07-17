use bstr::{BString, ByteSlice};
use serde::Serialize;

pub fn as_string_lossy<S>(v: &BString, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    v.to_str_lossy().serialize(s)
}

pub fn as_time_seconds_from_unix_epoch<S>(v: &git2::Time, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    v.seconds().serialize(s)
}

pub mod oid_opt {
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &Option<git2::Oid>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.as_ref().map(|v| v.to_string()).serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<git2::Oid>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = <Option<String> as Deserialize>::deserialize(d)?;
        hex.map(|v| {
            v.parse()
                .map_err(|err: git2::Error| serde::de::Error::custom(err.to_string()))
        })
        .transpose()
    }
}

pub mod oid_vec {
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &[git2::Oid], s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec: Vec<String> = v.iter().map(|v| v.to_string()).collect();
        vec.serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Vec<git2::Oid>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = <Vec<String> as Deserialize>::deserialize(d)?;
        let hex: Result<Vec<git2::Oid>, D::Error> = hex
            .into_iter()
            .map(|v| {
                git2::Oid::from_str(v.as_str())
                    .map_err(|err: git2::Error| serde::de::Error::custom(err.to_string()))
            })
            .collect();
        hex
    }
}

pub mod oid {
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &git2::Oid, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.to_string().serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<git2::Oid, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(d)?;
        hex.parse()
            .map_err(|err: git2::Error| serde::de::Error::custom(err.to_string()))
    }
}

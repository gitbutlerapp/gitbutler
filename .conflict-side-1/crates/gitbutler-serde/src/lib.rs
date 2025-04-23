use serde::Serialize;

mod bstring;
pub use bstring::BStringForFrontend;

pub mod bstring_lossy {
    use bstr::{BString, ByteSlice};
    use serde::Serialize;

    pub fn serialize<S>(v: &BString, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.to_str_lossy().serialize(s)
    }
}

pub mod bstring_vec_lossy {
    use bstr::{BString, ByteSlice};
    use serde::Serialize;

    pub fn serialize<S>(v: &[BString], s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec: Vec<String> = v.iter().map(|v| v.to_str_lossy().into()).collect();
        vec.serialize(s)
    }
}

pub mod bstring_opt_lossy {
    use bstr::{BString, ByteSlice};
    use serde::Serialize;

    pub fn serialize<S>(v: &Option<BString>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.as_ref().map(|v| v.to_str_lossy()).serialize(s)
    }
}

pub fn as_string_lossy_vec_remote_name<S>(
    v: &[gix::remote::Name<'static>],
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let vec: Vec<String> = v.iter().map(|v| v.as_bstr().to_string()).collect();
    vec.serialize(s)
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

pub mod object_id_opt {
    use gitbutler_oxidize::{ObjectIdExt, OidExt};
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &Option<gix::ObjectId>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.as_ref().map(|v| v.to_git2().to_string()).serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<gix::ObjectId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = <Option<String> as Deserialize>::deserialize(d)?;
        hex.map(|v| {
            v.parse::<git2::Oid>()
                .map(|oid| oid.to_gix())
                .map_err(|err: git2::Error| serde::de::Error::custom(err.to_string()))
        })
        .transpose()
    }
}

/// use like `#[serde(with = "gitbutler_serde::object_id")]` to serialize [`gix::ObjectId`].
pub mod object_id {
    use serde::{Deserialize, Deserializer, Serialize};
    use std::str::FromStr;

    /// serialize an object ID as hex-string.
    pub fn serialize<S>(v: &gix::ObjectId, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.to_string().serialize(s)
    }

    /// deserialize an object ID from hex-string.
    pub fn deserialize<'de, D>(d: D) -> Result<gix::ObjectId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(d)?;
        gix::ObjectId::from_str(hex.as_ref()).map_err(serde::de::Error::custom)
    }
}

pub mod object_id_vec {
    use serde::{Deserialize, Deserializer, Serialize};
    use std::str::FromStr;

    /// serialize an object ID as hex-string.
    pub fn serialize<S>(v: &[gix::ObjectId], s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec: Vec<String> = v.iter().map(|v| v.to_string()).collect();
        vec.serialize(s)
    }

    /// deserialize an object ID from hex-string.
    pub fn deserialize<'de, D>(d: D) -> Result<Vec<gix::ObjectId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = <Vec<String> as Deserialize>::deserialize(d)?;
        let hex: Result<Vec<gix::ObjectId>, D::Error> = hex
            .into_iter()
            .map(|v| {
                gix::ObjectId::from_str(v.as_ref())
                    .map_err(|err| serde::de::Error::custom(err.to_string()))
            })
            .collect();
        hex
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

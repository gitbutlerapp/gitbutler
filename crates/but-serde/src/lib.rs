//! Serde utilities for JSON serialization and deserialization.
//!
//! Each helper in this crate is meant to be used from `#[serde(with = "...")]`
//! or `#[serde(serialize_with = "...")]` on the field whose runtime type should
//! be exported as a simpler JSON shape.
//!
//! Put the annotation on the field itself:
//!
//! ```rust
//! #[derive(serde::Serialize, serde::Deserialize)]
//! struct Example {
//!     #[serde(with = "but_serde::object_id")]
//!     id: gix::ObjectId,
//! }
//! ```
//!
//! The examples below intentionally show the concrete field type that should
//! carry each annotation.

use serde::Serialize;

mod bstring;
pub use bstring::BStringForFrontend;

/// Use on *lossy* `BString` fields that should serialize as JSON strings.
///
/// ```rust
/// #[derive(serde::Serialize)]
/// struct Example {
///     #[serde(with = "but_serde::bstring_lossy")]
///     name: bstr::BString,
/// }
/// ```
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

/// Use on `gix::refs::FullName` fields that should serialize as JSON strings.
///
/// ```rust
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Example {
///     #[serde(with = "but_serde::fullname_lossy")]
///     reference: gix::refs::FullName,
/// }
/// ```
pub mod fullname_lossy {
    use bstr::ByteSlice;
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &gix::refs::FullName, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.as_bstr().to_str_lossy().serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<gix::refs::FullName, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = <String as Deserialize>::deserialize(d)?;
        gix::refs::FullName::try_from(string)
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    }
}

/// Use on `Vec<BString>` fields that should serialize as `string[]`.
///
/// ```rust
/// #[derive(serde::Serialize)]
/// struct Example {
///     #[serde(with = "but_serde::bstring_vec_lossy")]
///     paths: Vec<bstr::BString>,
/// }
/// ```
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

/// Use on optional `BString` fields that should serialize lossily as
/// `string | null`.
///
/// ```rust
/// #[derive(serde::Serialize)]
/// struct Example {
///     #[serde(with = "but_serde::bstring_lossy_opt")]
///     linked_worktree_id: Option<bstr::BString>,
/// }
/// ```
pub mod bstring_lossy_opt {
    use bstr::{BString, ByteSlice};
    use serde::Serialize;

    pub fn serialize<S>(v: &Option<BString>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.as_ref().map(|v| v.to_str_lossy()).serialize(s)
    }
}

/// Use on `Vec<gix::remote::Name<'static>>` fields that should serialize as
/// `string[]`.
///
/// ```rust
/// #[derive(serde::Serialize)]
/// struct Example {
///     #[serde(serialize_with = "but_serde::as_string_lossy_vec_remote_name")]
///     remotes: Vec<gix::remote::Name<'static>>,
/// }
/// ```
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

#[cfg(feature = "legacy")]
/// Use on legacy `git2::Time` fields that should serialize as Unix seconds.
///
/// ```rust
/// #[derive(serde::Serialize)]
/// struct Example {
///     #[serde(serialize_with = "but_serde::as_time_seconds_from_unix_epoch")]
///     created_at: git2::Time,
/// }
/// ```
pub fn as_time_seconds_from_unix_epoch<S>(v: &git2::Time, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    v.seconds().serialize(s)
}

/// Use on `gix::date::Time` fields that should serialize as Unix milliseconds.
///
/// ```rust
/// #[derive(serde::Serialize)]
/// struct Example {
///     #[serde(serialize_with = "but_serde::as_time_milliseconds_from_unix_epoch")]
///     created_at: gix::date::Time,
/// }
/// ```
pub fn as_time_milliseconds_from_unix_epoch<S>(v: &gix::date::Time, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    (v.seconds as i128 * 1000).serialize(s)
}

#[cfg(feature = "legacy")]
/// Use on `Option<git2::Oid>` fields serialized as `string | null`.
///
/// ```rust
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Example {
///     #[serde(with = "but_serde::oid_opt")]
///     base: Option<git2::Oid>,
/// }
/// ```
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

/// Use on `Option<gix::ObjectId>` fields serialized as `string | null`.
///
/// ```rust
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Example {
///     #[serde(with = "but_serde::object_id_opt")]
///     base: Option<gix::ObjectId>,
/// }
/// ```
pub mod object_id_opt {
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &Option<gix::ObjectId>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.as_ref().map(|v| v.to_string()).serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<gix::ObjectId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = <Option<String> as Deserialize>::deserialize(d)?;
        hex.map(|v| {
            v.parse::<gix::ObjectId>()
                .map_err(|err| serde::de::Error::custom(err.to_string()))
        })
        .transpose()
    }
}

/// Use on `gix::ObjectId` fields serialized as hex strings.
///
/// ```rust
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Example {
///     #[serde(with = "but_serde::object_id")]
///     id: gix::ObjectId,
/// }
/// ```
pub mod object_id {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &gix::ObjectId, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.to_string().serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<gix::ObjectId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(d)?;
        gix::ObjectId::from_str(hex.as_ref()).map_err(serde::de::Error::custom)
    }
}

/// Use on `Vec<gix::ObjectId>` fields serialized as `string[]`.
///
/// ```rust
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Example {
///     #[serde(with = "but_serde::object_id_vec")]
///     parent_ids: Vec<gix::ObjectId>,
/// }
/// ```
pub mod object_id_vec {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(v: &[gix::ObjectId], s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec: Vec<String> = v.iter().map(|v| v.to_string()).collect();
        vec.serialize(s)
    }

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

#[cfg(feature = "legacy")]
/// Use on `Vec<git2::Oid>` fields serialized as `string[]`.
///
/// ```rust
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Example {
///     #[serde(with = "but_serde::oid_vec")]
///     parent_ids: Vec<git2::Oid>,
/// }
/// ```
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

#[cfg(feature = "legacy")]
/// Use on `git2::Oid` fields serialized as hex string.
///
/// ```rust
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Example {
///     #[serde(with = "but_serde::oid")]
///     id: git2::Oid,
/// }
/// ```
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

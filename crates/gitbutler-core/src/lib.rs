#![feature(error_generic_member_access)]
#![cfg_attr(windows, feature(windows_by_handle))]
#![cfg_attr(
    all(windows, not(test), not(debug_assertions)),
    windows_subsystem = "windows"
)]
// FIXME(qix-): Stuff we want to fix but don't have a lot of time for.
// FIXME(qix-): PRs welcome!
#![allow(
    clippy::used_underscore_binding,
    clippy::module_name_repetitions,
    clippy::struct_field_names,
    clippy::too_many_lines
)]

pub mod askpass;
pub mod assets;
pub mod config;
pub mod dedup;
pub mod error;
pub mod fs;
pub mod git;
pub mod id;
pub mod keys;
pub mod lock;
pub mod ops;
pub mod path;
pub mod project_repository;
pub mod projects;
pub mod reader;
pub mod remotes;
pub mod ssh;
pub mod storage;
pub mod synchronize;
pub mod time;
pub mod types;
pub mod users;
pub mod virtual_branches;
#[cfg(target_os = "windows")]
pub mod windows;
pub mod writer;
pub mod zip;
pub mod serde {
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

    pub fn as_time_seconds_from_unix_epoch<S>(v: &git2::Time, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        v.seconds().serialize(s)
    }

    pub mod oid_opt {
        use crate::git;
        use serde::{Deserialize, Deserializer, Serialize};

        pub fn serialize<S>(v: &Option<git::Oid>, s: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            v.as_ref().map(|v| v.to_string()).serialize(s)
        }

        pub fn deserialize<'de, D>(d: D) -> Result<Option<git::Oid>, D::Error>
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

    pub mod oid {
        use crate::git;
        use serde::{Deserialize, Deserializer, Serialize};

        pub fn serialize<S>(v: &git::Oid, s: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            v.to_string().serialize(s)
        }

        pub fn deserialize<'de, D>(d: D) -> Result<git::Oid, D::Error>
        where
            D: Deserializer<'de>,
        {
            let hex = String::deserialize(d)?;
            hex.parse()
                .map_err(|err: git2::Error| serde::de::Error::custom(err.to_string()))
        }
    }
}

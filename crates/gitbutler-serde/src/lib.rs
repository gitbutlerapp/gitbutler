use serde::Serialize;

mod bstring;
pub use bstring::BStringForFrontend;

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

pub mod oid_hash_to_oid_set {
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(
        v: &std::collections::HashMap<git2::Oid, std::collections::HashSet<git2::Oid>>,
        s: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec: std::collections::HashMap<String, std::collections::HashSet<String>> = v
            .iter()
            .map(|(commit_id, commit_dependencies)| {
                (
                    commit_id.to_string(),
                    commit_dependencies
                        .iter()
                        .map(|item| item.to_string())
                        .collect(),
                )
            })
            .collect();
        vec.serialize(s)
    }

    pub fn deserialize<'de, D>(
        d: D,
    ) -> Result<std::collections::HashMap<git2::Oid, std::collections::HashSet<git2::Oid>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = <std::collections::HashMap<String, std::collections::HashSet<String>> as Deserialize>::deserialize(d)?;
        let hex: Result<
            std::collections::HashMap<git2::Oid, std::collections::HashSet<git2::Oid>>,
            D::Error,
        > = hex
            .into_iter()
            .map(|(commit_id, commit_dependencies)| {
                let commit_id = git2::Oid::from_str(commit_id.as_str())
                    .map_err(|err: git2::Error| serde::de::Error::custom(err.to_string()))?;
                let commit_dependencies: Result<std::collections::HashSet<git2::Oid>, D::Error> =
                    commit_dependencies
                        .into_iter()
                        .map(|id| {
                            git2::Oid::from_str(id.as_str()).map_err(|err: git2::Error| {
                                serde::de::Error::custom(err.to_string())
                            })
                        })
                        .collect();
                commit_dependencies.map(|commit_dependencies| (commit_id, commit_dependencies))
            })
            .collect();
        hex
    }
}

pub mod oid_hash_to_hunkhash_set {
    use serde::{Deserialize, Deserializer, Serialize};

    pub fn serialize<S>(
        v: &std::collections::HashMap<git2::Oid, std::collections::HashSet<md5::Digest>>,
        s: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let vec: std::collections::HashMap<String, std::collections::HashSet<String>> = v
            .iter()
            .map(|(commit_id, commit_dependencies)| {
                (
                    commit_id.to_string(),
                    commit_dependencies
                        .iter()
                        .map(|item| format!("{:x}", item))
                        .collect(),
                )
            })
            .collect();
        vec.serialize(s)
    }

    pub fn deserialize<'de, D>(
        d: D,
    ) -> Result<
        std::collections::HashMap<git2::Oid, std::collections::HashSet<md5::Digest>>,
        D::Error,
    >
    where
        D: Deserializer<'de>,
    {
        let hex = <std::collections::HashMap<String, std::collections::HashSet<String>> as Deserialize>::deserialize(d)?;
        let hex: Result<
            std::collections::HashMap<git2::Oid, std::collections::HashSet<md5::Digest>>,
            D::Error,
        > = hex
            .into_iter()
            .map(|(commit_id, commit_dependencies)| {
                let commit_id = git2::Oid::from_str(commit_id.as_str())
                    .map_err(|err: git2::Error| serde::de::Error::custom(err.to_string()))?;
                let commit_dependencies: Result<std::collections::HashSet<md5::Digest>, D::Error> =
                    commit_dependencies
                        .into_iter()
                        .map(|id| {
                            let mut buf = [0u8; 16];
                            hex::decode_to_slice(id.as_str(), &mut buf).map_err(|err| {
                                serde::de::Error::custom(format!("failed to decode hex: {}", err))
                            })?;
                            Ok(md5::Digest(buf))
                        })
                        .collect();
                commit_dependencies.map(|commit_dependencies| (commit_id, commit_dependencies))
            })
            .collect();
        hex
    }
}

mod reader;
mod writer;

use std::str::FromStr;

pub use reader::TargetReader as Reader;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
pub use writer::TargetWriter as Writer;

use crate::git;

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    pub branch: git::RemoteRefname,
    pub remote_url: String,
    pub sha: git::Oid,
}

impl Serialize for Target {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Target", 5)?;
        state.serialize_field("branchName", &self.branch.branch())?;
        state.serialize_field("remoteName", &self.branch.remote())?;
        state.serialize_field("remoteUrl", &self.remote_url)?;
        state.serialize_field("sha", &self.sha.to_string())?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for Target {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TargetData {
            branch_name: String,
            remote_name: String,
            remote_url: String,
            sha: String,
        }
        let target_data: TargetData = serde::Deserialize::deserialize(d)?;
        let sha = git::Oid::from_str(&target_data.sha)
            .map_err(|x| serde::de::Error::custom(x.message()))?;

        let target = Target {
            branch: git::RemoteRefname::new(&target_data.remote_name, &target_data.branch_name),
            remote_url: target_data.remote_url,
            sha,
        };
        Ok(target)
    }
}

impl Target {
    fn try_from(reader: &crate::reader::Reader) -> Result<Target, crate::reader::Error> {
        let results = reader.batch(&["name", "branch_name", "remote", "remote_url", "sha"])?;

        let name = results[0].clone();
        let branch_name = results[1].clone();
        let remote = results[2].clone();
        let remote_url = results[3].clone();
        let sha = results[4].clone();

        let branch_name = match name {
            Ok(branch) => {
                let branch: String = branch.try_into()?;
                Ok(branch.clone())
            }
            Err(crate::reader::Error::NotFound) => {
                // fallback to the old format
                let branch_name: String = branch_name?.try_into()?;
                Ok(branch_name)
            }
            Err(e) => Err(crate::reader::Error::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("branch: {}", e)).into(),
            )),
        }?;

        let remote_url: String = match remote_url {
            Ok(url) => Ok(url.try_into()?),
            // fallback to the old format
            Err(crate::reader::Error::NotFound) => Ok(remote?.try_into()?),
            Err(error) => Err(crate::reader::Error::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("remote: {}", error)).into(),
            )),
        }?;

        let sha: String = sha?.try_into()?;
        let sha = sha.parse().map_err(|e| {
            crate::reader::Error::Io(
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("sha: {}", e)).into(),
            )
        })?;

        Ok(Self {
            branch: format!("refs/remotes/{}", branch_name).parse().unwrap(),
            remote_url,
            sha,
        })
    }
}

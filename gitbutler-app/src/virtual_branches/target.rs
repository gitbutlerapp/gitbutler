mod reader;
mod writer;

use serde::{ser::SerializeStruct, Serialize, Serializer};

pub use reader::TargetReader as Reader;
pub use writer::TargetWriter as Writer;

use crate::git;

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    pub branch: git::RemoteRefname,
    pub remote_url: String,
    pub sha: git::Oid,
    pub last_fetched_ms: Option<u128>,
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
        state.serialize_field("lastFetchedMs", &self.last_fetched_ms)?;
        state.end()
    }
}

impl Target {
    fn try_from(reader: &crate::reader::Reader) -> Result<Target, crate::reader::Error> {
        let results = reader.batch(&[
            "name",
            "branch_name",
            "remote",
            "remote_url",
            "sha",
            "last_fetched_ms",
        ])?;

        let name = results[0].clone();
        let branch_name = results[1].clone();
        let remote = results[2].clone();
        let remote_url = results[3].clone();
        let sha = results[4].clone();
        let last_fetched_ms = results[5].clone();

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

        let last_fetched_ms: Option<u128> = match last_fetched_ms {
            Ok(last_fetched) => Some(last_fetched.try_into().map_err(|e| {
                crate::reader::Error::Io(
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("last_fetched_ms: {}", e),
                    )
                    .into(),
                )
            })?),
            Err(crate::reader::Error::NotFound) => None,
            Err(e) => return Err(e),
        };

        Ok(Self {
            branch: format!("refs/remotes/{}", branch_name).parse().unwrap(),
            remote_url,
            sha,
            last_fetched_ms,
        })
    }
}

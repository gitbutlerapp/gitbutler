use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};

use crate::git;

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    pub branch: git::RemoteRefname,
    pub remote_url: String,
    pub sha: git2::Oid,
    pub push_remote_name: Option<String>,
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
        if let Some(push_remote_name) = &self.push_remote_name {
            state.serialize_field("pushRemoteName", push_remote_name)?;
        }
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
            push_remote_name: Option<String>,
            sha: String,
        }
        let target_data: TargetData = serde::Deserialize::deserialize(d)?;
        let sha = git2::Oid::from_str(&target_data.sha)
            .map_err(|x| serde::de::Error::custom(x.message()))?;

        let target = Target {
            branch: git::RemoteRefname::new(&target_data.remote_name, &target_data.branch_name),
            remote_url: target_data.remote_url,
            sha,
            push_remote_name: target_data.push_remote_name,
        };
        Ok(target)
    }
}

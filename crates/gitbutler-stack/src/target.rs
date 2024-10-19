use gitbutler_reference::RemoteRefname;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    /// The combination of remote name and branch name, i.e. `origin` and `main`.
    /// The remote name is the one used to fetch from.
    /// It's equivalent to e.g. `refs/remotes/origin/main` , and the type `RemoteRefName`
    /// stores it as `<remote>` and `<suffix>` so that finding references named `<remote>/<suffix>`
    /// will typically find the local tracking branch unambiguously.
    pub branch: RemoteRefname,
    /// The URL of the remote behind the symbolic name.
    pub remote_url: String,
    /// The merge-base between `branch` and the current worktree `HEAD` upon first creation,
    /// but then it's the set to the new destination of e.g. `refs/remotes/origin/main` after
    /// the remote was fetched. This value is used to determine if there was a change,
    /// and if the *workspace* needs to be recalculated/rebased against the new commit.
    // TODO(ST): is it safe/correct to rename this to `branch_target_id`? Should be!
    //           It's just a bit strange it starts life as merge-base, but maybe it ends
    //           up the same anyway? Definitely could use a test then.
    pub sha: git2::Oid,
    /// The name of the remote to push to.
    pub push_remote_name: Option<String>,
}

impl Target {
    pub fn push_remote_name(&self) -> String {
        let upstream_remote = match &self.push_remote_name {
            Some(remote) => remote.clone(),
            None => self.branch.remote().to_owned(),
        };
        upstream_remote
    }
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
            branch: RemoteRefname::new(&target_data.remote_name, &target_data.branch_name),
            remote_url: target_data.remote_url,
            sha,
            push_remote_name: target_data.push_remote_name,
        };
        Ok(target)
    }
}

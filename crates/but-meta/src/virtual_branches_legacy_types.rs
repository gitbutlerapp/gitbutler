//! Legacy types that won't be needed anymore once the toml is removed.
//!
//! These are here to break the dependency to `gitbutler-stack`. In there, we have a dupe of these
//! types with conversions to allow us to keep the structs dumb.
//!
//! The types here are the only ones to implement `serde`.
#![allow(missing_docs)]
use std::collections::HashMap;

use but_core::ref_metadata::StackId;
use serde::{Deserialize, Serialize};

/// The state of virtual branches data, as persisted in a TOML file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VirtualBranches {
    /// This is the target/base that is set when a repo is added to gb
    pub default_target: Option<Target>,
    /// The targets for each virtual branch
    pub branch_targets: HashMap<StackId, Target>,
    /// The current state of the virtual branches
    pub branches: HashMap<StackId, Stack>,
    #[serde(with = "but_serde::object_id_opt", default)]
    pub last_pushed_base: Option<gix::ObjectId>,
}

mod stack {
    use std::{fmt, fmt::Display, path, str::FromStr};

    use anyhow::{Context as _, anyhow};
    use but_core::ref_metadata::StackId;
    use gitbutler_reference::{Refname, RemoteRefname};
    use serde::{Deserialize, Serialize, Serializer};

    // this is the struct for the virtual branch data that is stored in our data
    // store. it is more or less equivalent to a git branch reference, but it is not
    // stored or accessible from the git repository itself. it is stored in our
    // session storage under the branches/ directory.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Stack {
        pub id: StackId,
        /// If set, this means this virtual branch was originally created from `Some(branch)`.
        /// It can be *any* branch.
        pub source_refname: Option<Refname>,
        /// Upstream tracking branch reference, added when creating a stack from a branch.
        /// Used e.g. when listing commits from a fork.
        pub upstream: Option<RemoteRefname>,
        // order is the number by which UI should sort branches
        pub order: usize,
        /// This is the new metric for determining whether the branch is in the workspace, which means it's applied
        /// and its effects are available to the user.
        #[serde(default = "default_true")]
        pub in_workspace: bool,
        /// Represents the Stack state of pseudo-references ("heads").
        /// Do **NOT** edit this directly, instead use the `Stack` trait in gitbutler_stack.
        #[serde(default)]
        pub heads: Vec<StackBranch>,

        // For serialization backwards compatibility
        // These should not be read, it's just to satisfy past versions of the app
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(default)]
        pub notes: String,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(default)]
        pub ownership: BranchOwnershipClaims,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(default = "default_true")]
        pub allow_rebasing: bool,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(default = "default_false")]
        pub post_commits: bool,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(with = "but_serde::object_id")]
        #[serde(default = "default_null_object_id")]
        pub tree: gix::ObjectId,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(serialize_with = "serialize_u128", deserialize_with = "deserialize_u128")]
        #[serde(default)]
        pub created_timestamp_ms: u128,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(serialize_with = "serialize_u128", deserialize_with = "deserialize_u128")]
        #[serde(default)]
        pub updated_timestamp_ms: u128,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(default)]
        pub name: String,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(with = "but_serde::object_id")]
        #[serde(default = "default_null_object_id")]
        pub head: gix::ObjectId,
    }

    impl Stack {
        /// This is the name of the top-most branch, provided by the API for convenience
        /// Copy of `gitbutler-stack::Stack::derived_name()`.
        pub fn derived_name(&self) -> anyhow::Result<String> {
            self.heads
                .last()
                .map(|head| head.name.clone())
                .ok_or_else(|| anyhow!("but_meta::Stack::derived_name: Stack is uninitialized"))
        }
    }

    fn default_null_object_id() -> gix::ObjectId {
        gix::hash::Kind::Sha1.null()
    }

    fn default_true() -> bool {
        true
    }
    fn default_false() -> bool {
        false
    }

    fn serialize_u128<S>(x: &u128, s: S) -> anyhow::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_str(&x.to_string())
    }

    fn deserialize_u128<'de, D>(d: D) -> anyhow::Result<u128, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        let x: u128 = s.parse().map_err(serde::de::Error::custom)?;
        Ok(x)
    }

    impl Stack {
        pub fn new_with_just_heads(heads: Vec<StackBranch>, order: usize, in_workspace: bool) -> Self {
            Stack {
                id: StackId::generate(),
                order,
                in_workspace,
                heads,

                // Don't keep redundant information
                source_refname: None,
                upstream: None,

                // Unused - everything is defined by the top-most branch name.
                // unclear, obsolete

                // For serialization backwards compatibility
                #[expect(deprecated)]
                notes: String::new(),
                #[expect(deprecated)]
                ownership: BranchOwnershipClaims::default(),
                #[expect(deprecated)]
                allow_rebasing: true,
                #[expect(deprecated)]
                post_commits: false,
                #[expect(deprecated)]
                tree: gix::hash::Kind::Sha1.null(),
                #[expect(deprecated)]
                created_timestamp_ms: 0,
                #[expect(deprecated)]
                updated_timestamp_ms: 0,
                #[expect(deprecated)]
                name: String::default(),
                #[expect(deprecated)]
                head: gix::hash::Kind::Sha1.null(),
            }
        }
    }

    /// A GitButler-specific reference type that points to a commit or a patch (change).
    /// The principal difference between a `PatchReference` and a regular git reference is that a `PatchReference` can point to a change (patch) that is mutable.
    ///
    /// Because this is **NOT** a regular git reference, it will not be found in the `.git/refs`. It is instead managed by GitButler.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct StackBranch {
        /// The target of the reference - this can be a commit or a change that points to a commit.
        #[serde(alias = "target")]
        #[serde(with = "commit_id_serde")]
        pub head: gix::ObjectId,
        /// The name of the reference e.g. `master` or `feature/branch`. This should **NOT** include the `refs/heads/` prefix.
        /// The name must be unique within the repository.
        pub name: String,
        /// The pull request associated with the branch, or None if a pull request has not been created.
        #[serde(default)]
        pub pr_number: Option<usize>,
        /// Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
        /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
        #[serde(default)]
        pub archived: bool,

        #[serde(default)]
        pub review_id: Option<String>,
    }

    impl StackBranch {
        pub fn new_with_zero_head(
            name: String,
            pr_number: Option<usize>,
            review_id: Option<String>,
            archived: bool,
        ) -> Self {
            StackBranch {
                name,
                pr_number,
                archived,
                review_id,
                head: gix::hash::Kind::Sha1.null(),
            }
        }
    }

    /// Custom serde module for handling the legacy CommitOrChangeId format.
    /// This deserializes the old enum format but stores it as a gix::ObjectId.
    /// If a ChangeId is encountered during deserialization, it returns a null ObjectId.
    mod commit_id_serde {
        use std::str::FromStr;

        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        #[derive(Deserialize)]
        enum CommitOrChangeIdHelper {
            CommitId(String),
            #[allow(dead_code)]
            ChangeId(String),
        }

        pub fn serialize<S>(value: &gix::ObjectId, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            #[derive(Serialize)]
            enum CommitOrChangeId {
                CommitId(String),
            }
            CommitOrChangeId::CommitId(value.to_string()).serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<gix::ObjectId, D::Error>
        where
            D: Deserializer<'de>,
        {
            match CommitOrChangeIdHelper::deserialize(deserializer)? {
                CommitOrChangeIdHelper::CommitId(id) => gix::ObjectId::from_str(&id).map_err(serde::de::Error::custom),
                CommitOrChangeIdHelper::ChangeId(_) => {
                    // To find the commit id from a change ID, it would require to scan all applicable commits.
                    // Change IDs are deprecated anyway, so we return null here. For the most part, the app will use the reference anyways.
                    Ok(gix::hash::Kind::Sha1.null())
                }
            }
        }
    }

    /// A patch identifier which is either `CommitId` or a `ChangeId`.
    /// ChangeId should always be used if available.
    ///
    /// DEPRECATED: This enum is kept only for backwards compatibility during deserialization.
    /// New code should use `gix::ObjectId` directly.
    #[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
    #[deprecated(note = "Use gix::ObjectId directly instead")]
    pub enum CommitOrChangeId {
        /// A reference that points directly to a commit.
        CommitId(String),
    }

    #[derive(Debug, PartialEq, Default, Clone)]
    pub struct BranchOwnershipClaims {
        pub claims: Vec<OwnershipClaim>,
    }

    impl Serialize for BranchOwnershipClaims {
        fn serialize<S: Serializer>(&self, serializer: S) -> anyhow::Result<S::Ok, S::Error> {
            serializer.serialize_str(self.to_string().as_str())
        }
    }

    impl<'de> Deserialize<'de> for BranchOwnershipClaims {
        fn deserialize<D>(deserializer: D) -> anyhow::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            s.parse().map_err(serde::de::Error::custom)
        }
    }

    impl fmt::Display for BranchOwnershipClaims {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for file in &self.claims {
                writeln!(f, "{file}")?;
            }
            Ok(())
        }
    }

    impl FromStr for BranchOwnershipClaims {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> anyhow::Result<Self, Self::Err> {
            let mut ownership = BranchOwnershipClaims::default();
            for line in s.lines() {
                ownership.claims.push(line.parse()?);
            }
            Ok(ownership)
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct OwnershipClaim {
        pub file_path: path::PathBuf,
        pub hunks: Vec<Hunk>,
    }

    impl FromStr for OwnershipClaim {
        type Err = anyhow::Error;

        fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
            let mut file_path_parts = vec![];
            let mut ranges = vec![];
            for part in value.split(':').rev() {
                match part.split(',').map(str::parse).collect::<anyhow::Result<Vec<Hunk>>>() {
                    Ok(rr) => ranges.extend(rr),
                    Err(_) => {
                        file_path_parts.insert(0, part);
                    }
                }
            }

            if ranges.is_empty() {
                Err(anyhow::anyhow!("ownership ranges cannot be empty"))
            } else {
                Ok(Self {
                    file_path: file_path_parts
                        .join(":")
                        .parse()
                        .context(format!("failed to parse file path from {value}"))?,
                    hunks: ranges.clone(),
                })
            }
        }
    }

    impl fmt::Display for OwnershipClaim {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
            if self.hunks.is_empty() {
                write!(f, "{}", self.file_path.display())
            } else {
                write!(
                    f,
                    "{}:{}",
                    self.file_path.display(),
                    self.hunks
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(",")
                )
            }
        }
    }

    pub type HunkHash = md5::Digest;

    #[derive(Debug, PartialEq, Clone)]
    pub struct Hunk {
        /// A hash over the actual lines of the hunk, including the newlines between them
        /// (i.e. the first character of the first line to the last character of the last line in the input buffer)
        pub hash: Option<HunkHash>,
        /// The index of the first line this hunk is representing.
        pub start: u32,
        /// The index of *one past* the last line this hunk is representing.
        pub end: u32,
        /// Only set by the frontend when amending
        pub hunk_header: Option<HunkHeader>,
    }

    impl FromStr for Hunk {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
            let mut range = s.split('-');
            let start = if let Some(raw_start) = range.next() {
                raw_start
                    .parse::<u32>()
                    .context(format!("failed to parse start of range: {s}"))
            } else {
                Err(anyhow!("invalid range: {s}"))
            }?;

            let end = if let Some(raw_end) = range.next() {
                raw_end
                    .parse::<u32>()
                    .context(format!("failed to parse end of range: {s}"))
            } else {
                Err(anyhow!("invalid range: {s}"))
            }?;

            let hash = if let Some(raw_hash) = range.next() {
                if raw_hash.is_empty() {
                    None
                } else {
                    let mut buf = [0u8; 16];
                    hex::decode_to_slice(raw_hash, &mut buf)?;
                    Some(md5::Digest(buf))
                }
            } else {
                None
            };

            Hunk::new(start, end, hash)
        }
    }

    impl Hunk {
        pub fn new(start: u32, end: u32, hash: Option<HunkHash>) -> anyhow::Result<Self> {
            if start > end {
                Err(anyhow!("invalid range: {start}-{end}"))
            } else {
                Ok(Hunk {
                    hash,
                    start,
                    end,
                    hunk_header: None,
                })
            }
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub struct HunkHeader {
        pub old_start: u32,
        pub old_lines: u32,
        pub new_start: u32,
        pub new_lines: u32,
    }

    impl Display for Hunk {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}-{}", self.start, self.end)?;
            match &self.hash {
                Some(hash) => write!(f, "-{hash:x}"),
                None => Ok(()),
            }
        }
    }
}
pub use stack::*;

mod target {
    use std::str::FromStr;

    use gitbutler_reference::RemoteRefname;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeStruct};

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
        pub sha: gix::ObjectId,
        /// The name of the remote to push to.
        pub push_remote_name: Option<String>,
    }

    impl Serialize for Target {
        fn serialize<S>(&self, serializer: S) -> anyhow::Result<S::Ok, S::Error>
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
        fn deserialize<D: Deserializer<'de>>(d: D) -> anyhow::Result<Self, D::Error> {
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
            let sha = gix::ObjectId::from_str(&target_data.sha).map_err(|x| serde::de::Error::custom(x.to_string()))?;

            let target = Target {
                branch: RemoteRefname::new(&target_data.remote_name, &target_data.branch_name),
                remote_url: target_data.remote_url,
                sha,
                push_remote_name: target_data.push_remote_name,
            };
            Ok(target)
        }
    }
}
pub use target::Target;

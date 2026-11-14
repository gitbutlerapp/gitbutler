//! Legacy wire format retained for backwards-compatible metadata mirrors and operation-log snapshots.
use std::{collections::HashMap, path::PathBuf, str::FromStr};

use crate::{VbStack, VbStackHead, VbState, VirtualBranchesSnapshot};
use anyhow::{Context as _, anyhow};
use gitbutler_reference::{Refname, RemoteRefname};

use but_core::ref_metadata::StackId;
use serde::{Deserialize, Serialize};

/// The state of virtual branches data, as persisted in a TOML file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VirtualBranches {
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
        #[serde(
            serialize_with = "serialize_u128",
            deserialize_with = "deserialize_u128"
        )]
        #[serde(default)]
        pub created_timestamp_ms: u128,
        #[deprecated(note = "Legacy field, do not use. Kept for backwards compatibility.")]
        #[serde(
            serialize_with = "serialize_u128",
            deserialize_with = "deserialize_u128"
        )]
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
        /// This is the name of the top-most branch, provided by the API for convenience.
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
        pub fn new_with_just_heads(
            heads: Vec<StackBranch>,
            order: usize,
            in_workspace: bool,
        ) -> Self {
            Stack {
                id: StackId::generate(),
                order,
                in_workspace,
                heads,

                // Don't keep redundant information
                source_refname: None,
                upstream: None,

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
        #[serde(alias = "target")]
        #[serde(with = "commit_id_serde")]
        pub head: gix::ObjectId,
        /// The name of the reference e.g. `master` or `feature/branch`. This should **NOT** include the `refs/heads/` prefix.
        /// The name must be unique within the repository.
        pub name: String,
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
            #[expect(dead_code)]
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
                CommitOrChangeIdHelper::CommitId(id) => {
                    gix::ObjectId::from_str(&id).map_err(serde::de::Error::custom)
                }
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
                match part
                    .split(',')
                    .map(str::parse)
                    .collect::<anyhow::Result<Vec<Hunk>>>()
                {
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

/// Convert legacy snapshot data to the database representation.
pub fn legacy_to_snapshot(vb: &VirtualBranches) -> anyhow::Result<VirtualBranchesSnapshot> {
    let initialized = true;
    let last_pushed_base_sha = vb.last_pushed_base.map(|oid| oid.to_string());

    let mut stacks: Vec<_> = vb.branches.iter().collect();
    stacks.sort_by_key(|(sid, stack)| (stack.order, **sid));

    let mut out_stacks = Vec::with_capacity(stacks.len());
    let mut out_heads = Vec::new();
    for (stack_id, stack) in stacks {
        let Stack {
            id: _, // should match with `stack_id`, but we effectively normalise on the unique key here
            source_refname,
            upstream,
            order,
            in_workspace,
            heads,
            #[expect(deprecated)]
            notes,
            #[expect(deprecated)]
            ownership,
            #[expect(deprecated)]
            allow_rebasing,
            #[expect(deprecated)]
            post_commits,
            #[expect(deprecated)]
            tree,
            #[expect(deprecated)]
            created_timestamp_ms,
            #[expect(deprecated)]
            updated_timestamp_ms,
            #[expect(deprecated)]
            name,
            #[expect(deprecated)]
            head,
        } = stack;
        out_stacks.push(VbStack {
            id: stack_id.to_string(),
            source_refname: source_refname.as_ref().map(ToString::to_string),
            upstream_remote_name: upstream
                .as_ref()
                .and_then(|up| up.remote())
                .map(|remote| remote.to_owned()),
            upstream_branch_name: upstream.as_ref().map(|up| up.branch().to_owned()),
            sort_order: i64::try_from(*order).context("Stack order exceeds i64")?,
            in_workspace: *in_workspace,
            legacy_name: name.clone(),
            legacy_notes: notes.clone(),
            legacy_ownership: ownership.to_string(),
            legacy_allow_rebasing: *allow_rebasing,
            legacy_post_commits: *post_commits,
            legacy_tree_sha: tree.to_string(),
            legacy_head_sha: head.to_string(),
            legacy_created_timestamp_ms: created_timestamp_ms.to_string(),
            legacy_updated_timestamp_ms: updated_timestamp_ms.to_string(),
        });

        for (position, head) in heads.iter().enumerate() {
            let StackBranch {
                head,
                name,
                pr_number,
                archived,
                review_id,
            } = head;
            out_heads.push(VbStackHead {
                stack_id: stack_id.to_string(),
                position: i64::try_from(position).context("Head position exceeds i64")?,
                name: name.clone(),
                head_sha: head.to_string(),
                pr_number: pr_number.map(|value| value as i64),
                archived: *archived,
                review_id: review_id.clone(),
            });
        }
    }

    Ok(VirtualBranchesSnapshot {
        state: VbState {
            last_pushed_base_sha,
            initialized,
            ..Default::default()
        },
        stacks: out_stacks,
        heads: out_heads,
    })
}

/// Convert database metadata to the legacy snapshot format used by the operation log.
pub fn snapshot_to_legacy(snapshot: &VirtualBranchesSnapshot) -> anyhow::Result<VirtualBranches> {
    let VirtualBranchesSnapshot {
        state,
        stacks,
        heads,
    } = snapshot;
    let last_pushed_base = state
        .last_pushed_base_sha
        .as_ref()
        .map(|value| {
            gix::ObjectId::from_str(value)
                .with_context(|| format!("Invalid last_pushed_base sha: {value}"))
        })
        .transpose()?;

    let mut branches = HashMap::new();
    for stack in stacks {
        let VbStack {
            id,
            source_refname,
            upstream_remote_name,
            upstream_branch_name,
            sort_order,
            in_workspace,
            legacy_name,
            legacy_notes,
            legacy_ownership,
            legacy_allow_rebasing,
            legacy_post_commits,
            legacy_tree_sha,
            legacy_head_sha,
            legacy_created_timestamp_ms,
            legacy_updated_timestamp_ms,
        } = stack;
        let stack_id = StackId::from_str(id).with_context(|| format!("Invalid stack id '{id}'"))?;
        let source_refname = source_refname
            .as_ref()
            .map(|name| {
                Refname::from_str(name).with_context(|| format!("Invalid source_refname '{name}'"))
            })
            .transpose()?;
        let upstream = match (upstream_remote_name, upstream_branch_name) {
            (Some(remote), Some(branch)) => Some(RemoteRefname::new(remote, branch)),
            _ => None,
        };

        branches.insert(
            stack_id,
            Stack {
                id: stack_id,
                source_refname,
                upstream,
                order: usize::try_from(*sort_order)
                    .with_context(|| format!("Invalid stack sort order '{sort_order}'"))?,
                in_workspace: *in_workspace,
                heads: Vec::new(),
                #[expect(deprecated)]
                notes: legacy_notes.clone(),
                #[expect(deprecated)]
                ownership: legacy_ownership
                    .parse()
                    .with_context(|| format!("Invalid ownership claims for '{stack_id}'"))?,
                #[expect(deprecated)]
                allow_rebasing: *legacy_allow_rebasing,
                #[expect(deprecated)]
                post_commits: *legacy_post_commits,
                #[expect(deprecated)]
                tree: gix::ObjectId::from_str(legacy_tree_sha).with_context(|| {
                    format!("Invalid legacy tree sha '{legacy_tree_sha}' for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                created_timestamp_ms: legacy_created_timestamp_ms.parse().with_context(|| {
                    format!("Invalid legacy created timestamp for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                updated_timestamp_ms: legacy_updated_timestamp_ms.parse().with_context(|| {
                    format!("Invalid legacy updated timestamp for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                name: legacy_name.clone(),
                #[expect(deprecated)]
                head: gix::ObjectId::from_str(legacy_head_sha).with_context(|| {
                    format!("Invalid legacy head sha '{legacy_head_sha}' for '{stack_id}'")
                })?,
            },
        );
    }

    for head in heads {
        let VbStackHead {
            stack_id,
            position: _, // previously set based on vec position
            name,
            head_sha,
            pr_number,
            archived,
            review_id,
        } = head;
        let stack_id = StackId::from_str(stack_id)
            .with_context(|| format!("Invalid stack id '{stack_id}'"))?;
        let stack = branches
            .get_mut(&stack_id)
            .ok_or_else(|| anyhow!("Missing stack '{stack_id}' for head '{name}'"))?;
        stack.heads.push(StackBranch {
            head: gix::ObjectId::from_str(head_sha)
                .with_context(|| format!("Invalid head sha '{head_sha}' on '{name}'"))?,
            name: name.clone(),
            pr_number: pr_number
                .map(usize::try_from)
                .transpose()
                .with_context(|| {
                    format!(
                        "Invalid pr_number '{}' on stack '{stack_id}'",
                        pr_number.unwrap_or_default(),
                    )
                })?,
            archived: *archived,
            review_id: review_id.clone(),
        });
    }

    Ok(VirtualBranches {
        branches,
        last_pushed_base,
    })
}

pub(crate) struct LegacyMirror {
    path: PathBuf,
    contents: String,
}

pub(crate) fn prepare_mirror(
    conn: &rusqlite::Connection,
) -> rusqlite::Result<Option<LegacyMirror>> {
    let Some(path) = conn
        .path()
        .filter(|path| !path.is_empty() && *path != ":memory:")
    else {
        return Ok(None);
    };
    // The transaction wrapper also serves cache databases, which have no workspace metadata.
    if !conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'vb_state')",
        [],
        |row| row.get::<_, bool>(0),
    )? {
        return Ok(None);
    }
    let Some(snapshot) = (crate::VirtualBranchesHandle { conn }).get_snapshot()? else {
        return Ok(None);
    };
    let mut data = snapshot_to_legacy(&snapshot)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
    // Old clients need object IDs in their mirror; runtime metadata uses Git references instead.
    let repo = PathBuf::from(path)
        .parent()
        .and_then(|dir| gix::discover(dir).ok());
    for stack in data.branches.values_mut() {
        #[expect(deprecated)]
        if stack.name.is_empty()
            && let Some(head) = stack.heads.last()
        {
            stack.name = head.name.clone();
        }
        if let Some(repo) = repo.as_ref() {
            for head in &mut stack.heads {
                if head.head.is_null()
                    && let Ok(mut reference) = repo.find_reference(&head.name)
                    && let Ok(id) = reference.peel_to_id()
                {
                    head.head = id.detach();
                }
            }
        }
    }
    let contents = toml::to_string(&data)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
    Ok(Some(LegacyMirror {
        path: PathBuf::from(path).with_file_name("virtual_branches.toml"),
        contents,
    }))
}

pub(crate) fn write_prepared_mirror(mirror: Option<LegacyMirror>) -> rusqlite::Result<()> {
    let Some(mirror) = mirror else {
        return Ok(());
    };
    but_utils::write(&mirror.path, mirror.contents.as_bytes())
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
    if let Some(directory) = mirror.path.parent() {
        but_project_handle::write_refresh_sentinel(directory);
    }
    Ok(())
}

pub(crate) fn commit_with_mirror(
    sp: rusqlite::Savepoint<'_>,
    mirror_on_commit: bool,
    metadata_changed: Option<&std::cell::Cell<bool>>,
) -> rusqlite::Result<()> {
    let mirror = if mirror_on_commit {
        prepare_mirror(&sp)?
    } else {
        None
    };
    sp.commit()?;
    if let Some(changed) = metadata_changed {
        changed.set(true);
    }
    write_prepared_mirror(mirror)
}

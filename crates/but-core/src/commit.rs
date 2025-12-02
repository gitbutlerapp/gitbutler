use std::{collections::HashSet, path::PathBuf, str::FromStr};

use anyhow::{Context as _, bail};
use bstr::{BString, ByteSlice};
use gix::prelude::ObjectIdExt;
use serde::{Deserialize, Serialize};

use crate::Commit;

/// A unique ID to track any commit.
pub type ChangeId = crate::Id<'C'>;

/// A collection of all the extra information we keep in the headers of a commit.
#[derive(Debug, Clone, Copy)]
pub struct HeadersV2 {
    /// A property we can use to determine if two different commits are
    /// actually the same "patch" at different points in time. We carry it
    /// forwards when you rebase a commit in GitButler.
    /// Note that these don't have to be unique within a branch even,
    /// and it's possible that different commits with the same change-id
    /// have different content.
    pub change_id: ChangeId,
    /// A property used to indicate that we've written a conflicted tree to a
    /// commit, and `Some(num_files)` is the amount of conflicted files.
    ///
    /// Conflicted commits should never make it into the main trunk.
    /// If `None`, the commit is a normal commit without a special tree.
    pub conflicted: Option<u64>,
}

impl HeadersV2 {
    /// Extract header information from the given `commit`, or return `None` if not present.
    pub fn try_from_commit(commit: &gix::objs::Commit) -> Option<Self> {
        if let Some(header) = commit.extra_headers().find(HEADERS_VERSION_FIELD) {
            let version = header.to_owned();

            if version == HEADERS_VERSION {
                let change_id = ChangeId::from_str(
                    commit
                        .extra_headers()
                        .find(HEADERS_CHANGE_ID_FIELD)?
                        .to_str()
                        .ok()?,
                )
                .ok()?;

                let conflicted = commit
                    .extra_headers()
                    .find(HEADERS_CONFLICTED_FIELD)
                    .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

                Some(HeadersV2 {
                    change_id,
                    conflicted,
                })
            } else {
                tracing::warn!(
                    "Ignoring unknown commit header version '{version}' in commit {commit:?}",
                );
                None
            }
        } else {
            // Parse v1 headers
            let change_id = commit.extra_headers().find("change-id")?;
            let change_id = change_id.to_str().ok()?.to_string();
            let headers = HeadersV1 { change_id };
            Some(headers.into())
        }
    }

    /// Remove all header fields from `commit`.
    pub fn remove_in_commit(commit: &mut gix::objs::Commit) {
        for field in [
            HEADERS_VERSION_FIELD,
            HEADERS_CHANGE_ID_FIELD,
            HEADERS_CONFLICTED_FIELD,
        ] {
            if let Some(pos) = commit.extra_headers().find_pos(field) {
                commit.extra_headers.remove(pos);
            }
        }
    }

    /// Write the values from this instance to the given `commit`,  fully replacing any header
    /// that might have been there before.
    pub fn set_in_commit(&self, commit: &mut gix::objs::Commit) {
        Self::remove_in_commit(commit);
        commit
            .extra_headers
            .extend(Vec::<(BString, BString)>::from(self));
    }
}

impl Default for HeadersV2 {
    fn default() -> Self {
        HeadersV2 {
            // Change ID using base16 encoding
            change_id: if cfg!(feature = "testing") {
                if std::env::var("GITBUTLER_CHANGE_ID").is_ok() {
                    ChangeId::from_number_for_testing(1)
                } else {
                    ChangeId::generate()
                }
            } else {
                ChangeId::generate()
            },
            conflicted: None,
        }
    }
}

/// Used to represent the old commit headers layout, here just for backwards compatibility.
#[derive(Debug)]
struct HeadersV1 {
    change_id: String,
}

impl From<HeadersV1> for HeadersV2 {
    fn from(commit_headers_v1: HeadersV1) -> HeadersV2 {
        HeadersV2 {
            change_id: commit_headers_v1
                .change_id
                .parse()
                .unwrap_or_else(|_| ChangeId::generate()),
            conflicted: None,
        }
    }
}

const HEADERS_VERSION_FIELD: &str = "gitbutler-headers-version";
const HEADERS_CHANGE_ID_FIELD: &str = "gitbutler-change-id";
/// The name of the header field that stores the amount of conflicted files.
pub const HEADERS_CONFLICTED_FIELD: &str = "gitbutler-conflicted";
const HEADERS_VERSION: &str = "2";

impl From<&HeadersV2> for Vec<(BString, BString)> {
    fn from(hdr: &HeadersV2) -> Self {
        let mut out = vec![
            (
                BString::from(HEADERS_VERSION_FIELD),
                BString::from(HEADERS_VERSION),
            ),
            (
                HEADERS_CHANGE_ID_FIELD.into(),
                hdr.change_id.to_string().into(),
            ),
        ];

        if let Some(conflicted) = hdr.conflicted {
            out.push((
                HEADERS_CONFLICTED_FIELD.into(),
                conflicted.to_string().into(),
            ));
        }
        out
    }
}

/// When commits are in conflicting state, they store various trees which to help deal with the conflict.
///
/// This also includes variant that represents the blob which contains the
/// conflicted information.
#[derive(Debug, Copy, Clone)]
pub enum TreeKind {
    /// Our tree that caused a conflict during the merge.
    Ours,
    /// Their tree that caused a conflict during the merge.
    Theirs,
    /// The base of the conflicting mereg.
    Base,
    /// The tree that resulted from the merge with auto-resolution enabled.
    AutoResolution,
    /// The information about what is conflicted.
    ConflictFiles,
}

impl TreeKind {
    /// Return then name of the entry this tree would take in the 'meta' tree that captures cherry-pick conflicts.
    pub fn as_tree_entry_name(&self) -> &'static str {
        match self {
            TreeKind::Ours => ".conflict-side-0",
            TreeKind::Theirs => ".conflict-side-1",
            TreeKind::Base => ".conflict-base-0",
            TreeKind::AutoResolution => ".auto-resolution",
            TreeKind::ConflictFiles => ".conflict-files",
        }
    }
}

/// Instantiation
impl<'repo> Commit<'repo> {
    /// Decode the object at `commit_id` and keep its data for later query.
    pub fn from_id(commit_id: gix::Id<'repo>) -> anyhow::Result<Self> {
        let commit = commit_id
            .object()?
            .try_into_commit()?
            .decode()?
            .try_into()?;
        Ok(Commit {
            id: commit_id,
            inner: commit,
        })
    }
}

impl Commit<'_> {
    /// Set this commit to use the given `headers`, completely replacing the ones it might currently have.
    pub fn set_headers(&mut self, header: &HeadersV2) {
        header.set_in_commit(self)
    }
}

impl std::ops::Deref for Commit<'_> {
    type Target = gix::objs::Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for Commit<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl HeadersV2 {
    /// Return `true` if this commit contains a tree that is conflicted.
    pub fn is_conflicted(&self) -> bool {
        self.conflicted.is_some()
    }
}

/// Access
impl<'repo> Commit<'repo> {
    /// Return `true` if this commit contains a tree that is conflicted.
    pub fn is_conflicted(&self) -> bool {
        self.headers().is_some_and(|hdr| hdr.is_conflicted())
    }

    /// If the commit is conflicted, then it returns the auto-resolution tree,
    /// otherwise it returns the commit's tree.
    ///
    /// Most of the time this is what you want to use when diffing or
    /// displaying the commit to the user.
    pub fn tree_id_or_auto_resolution(&self) -> anyhow::Result<gix::Id<'repo>> {
        self.tree_id_or_kind(TreeKind::AutoResolution)
    }

    /// If the commit is conflicted, then return the particular conflict-tree
    /// specified by `kind`, otherwise return the commit's tree.
    ///
    /// Most of the time, you will probably want to use [`Self::tree_id_or_auto_resolution()`]
    /// instead.
    pub fn tree_id_or_kind(&self, kind: TreeKind) -> anyhow::Result<gix::Id<'repo>> {
        Ok(if self.is_conflicted() {
            self.inner
                .tree
                .attach(self.id.repo)
                .object()?
                .into_tree()
                .find_entry(kind.as_tree_entry_name())
                .with_context(|| format!("Unexpected tree in conflicting commit {}", self.id))?
                .id()
        } else {
            self.inner.tree.attach(self.id.repo)
        })
    }

    /// Return our custom headers, of present.
    pub fn headers(&self) -> Option<HeadersV2> {
        HeadersV2::try_from_commit(&self.inner)
    }
}

/// Conflict specific details
impl Commit<'_> {
    /// Obtains the conflict entries of a conflicted commit if the commit is
    /// conflicted, otherwise returns None.
    pub fn conflict_entries(&self) -> anyhow::Result<Option<ConflictEntries>> {
        let repo = self.id.repo;

        if !self.is_conflicted() {
            return Ok(None);
        }

        let tree = repo.find_tree(self.tree)?;
        let Some(conflicted_entries_blob) =
            tree.find_entry(TreeKind::ConflictFiles.as_tree_entry_name())
        else {
            bail!(
                "There has been a malformed conflicted commit, unable to find the conflicted files"
            );
        };
        let conflicted_entries_blob = conflicted_entries_blob.object()?.into_blob();
        let conflicted_entries: ConflictEntries =
            toml::from_str(&conflicted_entries_blob.data.as_bstr().to_str_lossy())?;

        Ok(Some(conflicted_entries))
    }
}

/// Represents what was causing a particular commit to conflict when rebased.
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntries {
    /// The ancestors that were conflicted
    pub ancestor_entries: Vec<PathBuf>,
    /// The ours side entries that were conflicted
    pub our_entries: Vec<PathBuf>,
    /// The theirs side entries that were conflicted
    pub their_entries: Vec<PathBuf>,
}

impl ConflictEntries {
    /// If there are any conflict entries
    pub fn has_entries(&self) -> bool {
        !self.ancestor_entries.is_empty()
            || !self.our_entries.is_empty()
            || !self.their_entries.is_empty()
    }

    /// The total count of conflicted entries
    pub fn total_entries(&self) -> usize {
        let set = self
            .ancestor_entries
            .iter()
            .chain(self.our_entries.iter())
            .chain(self.their_entries.iter())
            .collect::<HashSet<_>>();

        set.len()
    }
}

use crate::Commit;
use anyhow::Context;
use bstr::{BString, ByteSlice};
use gix::prelude::ObjectIdExt;
use uuid::Uuid;

/// A collection of all the extra information we keep in the headers of a commit.
#[derive(Debug, Clone)]
pub struct HeadersV2 {
    /// A property we can use to determine if two different commits are
    /// actually the same "patch" at different points in time. We carry it
    /// forwards when you rebase a commit in GitButler.
    /// Note that these don't have to be unique within a branch even,
    /// and it's possible that different commits with the same change-id
    /// have different content.
    pub change_id: String,
    /// A property used to indicate that we've written a conflicted tree to a
    /// commit, and `Some(num_files)` is the amount of conflicted files.
    ///
    /// Conflicted commits should never make it into the main trunk.
    /// If `None`, the commit is a normal commit without a special tree.
    pub conflicted: Option<u64>,
}

impl Default for HeadersV2 {
    fn default() -> Self {
        HeadersV2 {
            // Change ID using base16 encoding
            change_id: if cfg!(feature = "testing") {
                std::env::var("CHANGE_ID").unwrap_or_else(|_| {
                    eprintln!(
                        "With 'testing' feature the `CHANGE_ID` \
environment variable can be set have stable values"
                    );
                    Uuid::new_v4().to_string()
                })
            } else {
                Uuid::new_v4().to_string()
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
            change_id: commit_headers_v1.change_id,
            conflicted: None,
        }
    }
}

const HEADERS_VERSION_FIELD: &str = "gitbutler-headers-version";
const HEADERS_CHANGE_ID_FIELD: &str = "gitbutler-change-id";
const HEADERS_CONFLICTED_FIELD: &str = "gitbutler-conflicted";

impl From<HeadersV2> for Vec<(BString, BString)> {
    fn from(hdr: HeadersV2) -> Self {
        let mut out = vec![
            (BString::from(HEADERS_VERSION_FIELD), BString::from("2")),
            (HEADERS_CHANGE_ID_FIELD.into(), hdr.change_id.clone().into()),
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
}

/// Instantiation
impl<'repo> Commit<'repo> {
    /// Decode the object at `commit_id` and keep its data for later query.
    pub fn from_id(commit_id: gix::Id<'repo>) -> anyhow::Result<Self> {
        let commit = commit_id.object()?.try_into_commit()?.decode()?.into();
        Ok(Commit {
            id: commit_id,
            inner: commit,
        })
    }
}

/// Access
impl<'repo> Commit<'repo> {
    /// Return `true` if this commit contains a tree that is conflicted.
    pub fn is_conflicted(&self) -> bool {
        self.headers().is_some_and(|hdr| hdr.conflicted.is_some())
    }

    /// Return the hash of *our* tree, even if this commit is conflicted.
    pub fn tree_id(&self) -> anyhow::Result<gix::Id<'repo>> {
        Ok(self
            .tree_id_by_kind(TreeKind::Ours)?
            .expect("our tree is always available"))
    }

    /// Return the tree of the given `kind`, or `None` if no such tree exists as this instance is *not* conflicted.
    /// If `kind` is [`TreeKind::Ours`] one can always expect `Some()` tree.
    pub fn tree_id_by_kind(&self, kind: TreeKind) -> anyhow::Result<Option<gix::Id<'repo>>> {
        Ok(if self.is_conflicted() {
            let our_tree = self
                .inner
                .tree
                .attach(self.id.repo)
                .object()?
                .into_tree()
                .find_entry(match kind {
                    TreeKind::Ours => ".conflict-side-0",
                    TreeKind::Theirs => ".conflict-side-1",
                    TreeKind::Base => ".conflict-base-0",
                    TreeKind::AutoResolution => ".auto-resolution",
                })
                .with_context(|| format!("Unexpected tree in conflicting commit {}", self.id))?
                .id();
            Some(our_tree)
        } else if matches!(kind, TreeKind::Ours) {
            Some(self.inner.tree.attach(self.id.repo))
        } else {
            None
        })
    }

    /// Just like [`Self::tree_id_by_kind()`], but automatically return our tree if this instance isn't conflicted.
    pub fn tree_id_by_kind_or_ours(&self, kind: TreeKind) -> anyhow::Result<gix::Id<'repo>> {
        Ok(self
            .tree_id_by_kind(kind)?
            .unwrap_or_else(|| self.inner.tree.attach(self.id.repo)))
    }

    /// Return our custom headers, of present.
    pub fn headers(&self) -> Option<HeadersV2> {
        let decoded = &self.inner;
        if let Some(header) = decoded.extra_headers().find(HEADERS_VERSION_FIELD) {
            let version = header.to_owned();

            if version == "2" {
                let change_id = decoded.extra_headers().find(HEADERS_CHANGE_ID_FIELD)?;
                let change_id = change_id.to_str().ok()?.to_string();

                let conflicted = decoded
                    .extra_headers()
                    .find(HEADERS_CONFLICTED_FIELD)
                    .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

                Some(HeadersV2 {
                    change_id,
                    conflicted,
                })
            } else {
                tracing::warn!(
                    "Ignoring unknown commit header version '{version}' in commit {}",
                    self.id
                );
                None
            }
        } else {
            // Parse v1 headers
            let change_id = decoded.extra_headers().find("change-id")?;
            let change_id = change_id.to_str().ok()?.to_string();
            let headers = HeadersV1 { change_id };
            Some(headers.into())
        }
    }
}

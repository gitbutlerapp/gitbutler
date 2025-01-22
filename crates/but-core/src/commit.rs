use crate::Commit;
use anyhow::Context;
use bstr::ByteSlice;
use gix::prelude::ObjectIdExt;

pub(super) mod changes {
    use crate::{ChangeState, TreeStatus};
    use crate::{Commit, ModeFlags, TreeChange};
    use gix::diff::tree_with_rewrites::Change;
    use gix::prelude::ObjectIdExt;

    /// Produce all changes that are needed to turn the tree of `lhs_commit` into the tree of `rhs_commit`.
    /// If `lhs_commit` is `None`, it will be treated like an empty tree, which is useful if
    /// there was no tree to compare `lhs_commit` to (e.g. in case of the first commit).
    ///
    /// Note that we deal with conflicted commits correctly by resolving to the actual tree, not the one with meta-data.
    ///
    /// They are sorted by their current path.
    pub fn function(
        repo: &gix::Repository,
        lhs_commit: Option<gix::ObjectId>,
        rhs_commit: gix::ObjectId,
    ) -> anyhow::Result<Vec<TreeChange>> {
        let lhs_tree = lhs_commit
            .map(|commit_id| {
                Commit::from_id(commit_id.attach(repo)).and_then(|commit| {
                    let id = commit.tree_id()?;
                    Ok(id.object()?.into_tree())
                })
            })
            .transpose()?;
        let rhs_tree = Commit::from_id(rhs_commit.attach(repo))?
            .tree_id()?
            .object()
            .map(|obj| obj.into_tree())?;

        let changes = repo.diff_tree_to_tree(lhs_tree.as_ref(), &rhs_tree, None)?;
        let mut out: Vec<TreeChange> = changes.into_iter().map(Into::into).collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    impl From<gix::object::tree::diff::ChangeDetached> for TreeChange {
        fn from(value: gix::object::tree::diff::ChangeDetached) -> Self {
            match value {
                Change::Addition {
                    location,
                    entry_mode,
                    id,
                    ..
                } => TreeChange {
                    path: location,
                    status: TreeStatus::Addition {
                        state: ChangeState {
                            id,
                            kind: entry_mode.kind(),
                        },
                        is_untracked: false,
                    },
                },
                Change::Deletion {
                    location,
                    id,
                    entry_mode,
                    ..
                } => TreeChange {
                    path: location,
                    status: TreeStatus::Deletion {
                        previous_state: ChangeState {
                            id,
                            kind: entry_mode.kind(),
                        },
                    },
                },
                Change::Modification {
                    location,
                    previous_entry_mode,
                    previous_id,
                    id,
                    entry_mode,
                } => {
                    let previous_state = ChangeState {
                        id: previous_id,
                        kind: previous_entry_mode.kind(),
                    };
                    let state = ChangeState {
                        id,
                        kind: entry_mode.kind(),
                    };
                    TreeChange {
                        path: location,
                        status: TreeStatus::Modification {
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    }
                }
                Change::Rewrite {
                    source_location,
                    source_entry_mode,
                    source_id,
                    entry_mode,
                    id,
                    location,
                    diff: _,
                    copy: _,
                    ..
                } => {
                    let previous_state = ChangeState {
                        id: source_id,
                        kind: source_entry_mode.kind(),
                    };
                    let state = ChangeState {
                        id,
                        kind: entry_mode.kind(),
                    };
                    TreeChange {
                        path: location,
                        status: TreeStatus::Rename {
                            previous_path: source_location,
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    }
                }
            }
        }
    }
}

/// A collection of all the extra information we keep in a commit.
#[derive(Debug, Clone)]
pub struct HeadersV2 {
    /// A property we can use to determine if two different commits are
    /// actually the same "patch" at different points in time. We carry it
    /// forwards when you rebase a commit in GitButler.
    pub change_id: String,
    /// A property used to indicate that we've written a conflicted tree to a
    /// commit. This is only written if the property is present. Conflicted
    /// commits should never make it into the main trunk.
    pub conflicted: Option<u64>,
}

/// Used to represent the old commit headers layout. This should not be used in new code
#[derive(Debug)]
struct HeadersV1 {
    /// A property we can use to determine if two different commits are
    /// actually the same "patch" at different points in time. We carry it
    /// forwards when you rebase a commit in GitButler.
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
        if self.is_conflicted() {
            let our_tree = self
                .inner
                .tree
                .attach(self.id.repo)
                .object()?
                .into_tree()
                .find_entry(".conflict-side-0")
                .with_context(|| format!("Unexpected tree in conflicting commit {}", self.id))?
                .id();
            Ok(our_tree)
        } else {
            Ok(self.inner.tree.attach(self.id.repo))
        }
    }

    /// Return our custom headers, of present.
    pub fn headers(&self) -> Option<HeadersV2> {
        let decoded = &self.inner;
        if let Some(header) = decoded.extra_headers().find("gitbutler-headers-version") {
            let version = header.to_owned();

            if version == "2" {
                let change_id = decoded.extra_headers().find("gitbutler-change-id")?;
                let change_id = change_id.to_str().ok()?.to_string();

                let conflicted = decoded
                    .extra_headers()
                    .find("gitbutler-conflicted")
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

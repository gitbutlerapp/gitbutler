use anyhow::Context;
use bstr::{BStr, BString, ByteSlice};
use serde::Serialize;

/// Utilities for diffing, with workspace integration.
pub mod diff;

/// RefInfo types for the UI.
pub mod ref_info {
    use crate::ui;
    use crate::ui::{UpstreamCommit, ref_info};
    use anyhow::{Context, bail};
    use bstr::{BString, ByteSlice};
    use but_core::ref_metadata;
    use but_core::ref_metadata::StackId;
    use but_graph::projection::StackCommitFlags;
    use gix::refs::Category;
    use std::borrow::Cow;
    use std::ops::{Deref, DerefMut};

    /// A commit with must useful information extracted from the Git commit itself.
    ///
    /// Note that additional information can be computed and placed in the [`LocalCommit`] and [`RemoteCommit`]
    #[derive(Clone, Eq, PartialEq)]
    pub struct Commit {
        /// The hash of the commit.
        pub id: gix::ObjectId,
        /// The IDs of the parent commits, but may be empty if this is the first commit.
        pub parent_ids: Vec<gix::ObjectId>,
        /// The hash of the tree associated with the object.
        pub tree_id: gix::ObjectId,
        /// The complete message, verbatim.
        pub message: BString,
        /// The signature at which the commit was authored.
        pub author: gix::actor::Signature,
        /// The references pointing to this commit, even after dereferencing tag objects.
        /// These can be names of tags and branches.
        pub refs: Vec<gix::refs::FullName>,
        /// Additional properties to help classify this commit.
        pub flags: StackCommitFlags,
        /// Whether the commit is in a conflicted state, a GitButler concept.
        /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
        /// Conflicts are resolved via the Edit Mode mechanism.
        ///
        /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
        pub has_conflicts: bool,
        /// The GitButler assigned change-id that we hold on to for convenience to avoid duplicate decoding of commits
        /// when trying to associate remote commits with local ones.
        pub change_id: Option<but_core::commit::ChangeId>,
    }

    impl std::fmt::Debug for Commit {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "Commit({hash}, {msg:?}{flags})",
                hash = self.id.to_hex_with_len(7),
                msg = self.message,
                flags = self.flags.debug_string()
            )
        }
    }

    impl From<but_core::Commit<'_>> for Commit {
        fn from(value: but_core::Commit<'_>) -> Self {
            let has_conflicts = value.is_conflicted();
            let change_id = value.headers().map(|hdr| hdr.change_id);
            Commit {
                id: value.id.into(),
                tree_id: value.tree,
                parent_ids: value.parents.iter().cloned().collect(),
                message: value.inner.message,
                author: value.inner.author,
                has_conflicts,
                change_id,
                refs: Vec::new(),
                flags: StackCommitFlags::empty(),
            }
        }
    }

    impl Commit {
        /// A special constructor for very specific case.
        pub(crate) fn from_commit_ahead_of_workspace_commit(
            commit: gix::objs::Commit,
            graph_commit: &but_graph::Commit,
        ) -> Self {
            let hdr = but_core::commit::HeadersV2::try_from_commit(&commit);
            Commit {
                id: graph_commit.id,
                parent_ids: commit.parents.into_iter().collect(),
                tree_id: commit.tree,
                message: commit.message,
                has_conflicts: hdr.as_ref().is_some_and(|hdr| hdr.is_conflicted()),
                author: commit
                    .author
                    .to_ref(&mut gix::date::parse::TimeBuf::default())
                    .into(),
                refs: graph_commit.refs.clone(),
                flags: graph_commit.flags.into(),
                change_id: hdr.map(|hdr| hdr.change_id),
            }
        }
    }

    /// A commit that is reachable through the *local tracking branch*, with additional, computed information.
    #[derive(Clone, Eq, PartialEq)]
    pub struct LocalCommit {
        /// The simple commit.
        pub inner: Commit,
        /// Provide additional information on how this commit relates to other points of reference, like its remote branch,
        /// or the target branch to integrate with.
        pub relation: LocalCommitRelation,
    }

    impl std::fmt::Debug for LocalCommit {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let refs = self
                .refs
                .iter()
                .map(|rn| format!("►{}", rn.shorten()))
                .collect::<Vec<_>>()
                .join(", ");
            write!(
                f,
                "LocalCommit({conflict}{hash}, {msg:?}, {relation}{refs})",
                conflict = if self.has_conflicts { "💥" } else { "" },
                hash = self.id.to_hex_with_len(7),
                msg = self.message,
                relation = self.relation.display(self.id),
                refs = if refs.is_empty() {
                    "".to_string()
                } else {
                    format!(", {refs}")
                }
            )
        }
    }

    /// The state of the [local commit](LocalCommit) in relation to its remote tracking branch or its integration branch.
    #[derive(Default, Debug, Eq, PartialEq, Clone, Copy)]
    pub enum LocalCommitRelation {
        /// The commit is only local
        #[default]
        LocalOnly,
        /// The commit is also present in the remote tracking branch.
        ///
        /// This is the case if:
        ///  - The commit has been pushed to the remote
        ///  - The commit has been copied from a remote commit (when applying a remote branch)
        ///
        /// This variant carries the remote commit id.
        /// The `remote_commit_id` may be the same as the `id` or it may be different if the local commit has been rebased
        /// or updated in another way.
        LocalAndRemote(gix::ObjectId),
        /// The commit is considered integrated, using the given hash as the commit that contains this one.
        /// Note that this can be a 1:1 relation in case of rebased commits, or an N:1 relation in case of squash commits.
        /// If the id of this value is the same as the owning commit, this means it's included in the ancestry
        /// of the target branch.
        /// This should happen when the commit or the contents of this commit is already part of the base.
        Integrated(gix::ObjectId),
    }

    impl LocalCommitRelation {
        /// Convert this relation into something displaying, mainly for debugging.
        pub fn display(&self, id: gix::ObjectId) -> Cow<'static, str> {
            match self {
                LocalCommitRelation::LocalOnly => Cow::Borrowed("local"),
                LocalCommitRelation::LocalAndRemote(remote_id) => {
                    if *remote_id == id {
                        "local/remote(identity)".into()
                    } else {
                        format!("local/remote({})", remote_id.to_hex_with_len(7)).into()
                    }
                }
                LocalCommitRelation::Integrated(id) => {
                    format!("integrated({})", id.to_hex_with_len(7)).into()
                }
            }
        }
    }

    impl Deref for LocalCommit {
        type Target = Commit;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl DerefMut for LocalCommit {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    /// A reference in `refs/heads`.
    #[derive(serde::Serialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct BranchReference {
        /// The full ref name, like `refs/heads/feat`, for usage with the backend.
        pub full_name_bytes: BString,
        /// The short version of `full_name_bytes` for display.
        pub display_name: String,
    }

    impl From<gix::refs::FullName> for BranchReference {
        fn from(value: gix::refs::FullName) -> Self {
            BranchReference {
                display_name: value.shorten().to_str_lossy().into_owned(),
                full_name_bytes: value.into_inner(),
            }
        }
    }

    /// A reference in `refs/remotes`.
    #[derive(serde::Serialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct RemoteTrackingReference {
        /// The full ref name, like `refs/remotes/origin/on-remote`, for usage with the backend.
        pub full_name_bytes: BString,
        /// The short version of `full_name_bytes` for display, like `on-remote`, without the remote name.
        pub display_name: String,
        /// The symbolic name of the remote, like `origin`, or `origin/other`.
        pub remote_name: String,
    }

    impl RemoteTrackingReference {
        fn for_ui(
            ref_name: gix::refs::FullName,
            remote_names: &gix::remote::Names,
        ) -> anyhow::Result<Self> {
            let (category, short_name) = ref_name.category_and_short_name().with_context(|| {
                format!(
                    "Failed to categorize presume remote reference '{}'",
                    ref_name.as_bstr()
                )
            })?;
            if category != Category::RemoteBranch {
                bail!(
                    "Expected '{}' to be a remote tracking branch, but was {category:?}",
                    ref_name.as_bstr()
                );
            }
            let (longest_remote, short_name) = remote_names
                .iter()
                .rev()
                .find_map(|remote_name| {
                    short_name
                        .strip_prefix(remote_name.as_bytes())
                        .and_then(|stripped| {
                            if stripped.first() == Some(&b'/') {
                                Some((remote_name, stripped[1..].as_bstr()))
                            } else {
                                None
                            }
                        })
                })
                .ok_or(anyhow::anyhow!(
                    "Failed to find remote branch's corresponding remote"
                ))
                .with_context(|| {
                    format!(
                        "Remote reference '{}' couldn't be matched with any known remote",
                        ref_name.as_bstr()
                    )
                })?;

            Ok(RemoteTrackingReference {
                display_name: short_name.to_str_lossy().into_owned(),
                remote_name: longest_remote.to_str_lossy().into_owned(),
                full_name_bytes: ref_name.into_inner(),
            })
        }
    }

    /// Information about the target reference, the one we want to integrate with.
    #[derive(serde::Serialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Target {
        /// The remote tracking branch of the traget to integrate with, like `refs/remotes/origin/main`.
        pub remote_tracking_ref: RemoteTrackingReference,
        /// The amount of commits that aren't reachable by any segment in the workspace, they are in its future.
        pub commits_ahead: usize,
    }

    impl Target {
        fn for_ui(
            but_graph::projection::Target {
                ref_name,
                segment_index: _,
                commits_ahead,
            }: but_graph::projection::Target,
            remote_names: &gix::remote::Names,
        ) -> anyhow::Result<Self> {
            Ok(Target {
                remote_tracking_ref: RemoteTrackingReference::for_ui(ref_name, remote_names)?,
                commits_ahead,
            })
        }
    }
    /// The UI-clone of [`crate::RefInfo`].
    /// TODO: should also include base-branch data, see `get_base_branch_data()`.
    #[derive(serde::Serialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct RefInfo {
        /// The name of the ref that points to a workspace commit,
        /// *or* the name of the first stack segment.
        pub workspace_ref: Option<BranchReference>,
        /// The stacks visible in the current workspace.
        ///
        /// This is an empty array if the `HEAD` is unborn.
        /// Otherwise, there is one or more stacks.
        pub stacks: Vec<Stack>,
        /// The target to integrate workspace stacks into.
        ///
        /// If `None`, this is a local workspace that doesn't know when possibly pushed branches are considered integrated.
        /// This happens when there is a local branch checked out without a remote tracking branch.
        pub target: Option<Target>,
        /// The `workspace_ref_name` is `Some(_)` and belongs to GitButler, because it had metadata attached.
        /// This will be `false` when in single-branch mode.
        pub is_managed_ref: bool,
        /// The `workspace_ref_name` points to a commit that was specifically created by us.
        /// If the user advanced the workspace head by hand, this would be `false`.
        /// See if `ancestor_workspace_commit` is `Some()` to understand if anything could be fixed here.
        /// If there is no managed commits, we have to be extra careful as to what we allow, but setting
        /// up stacks and dependent branches is usually fine, and limited commit creation. Play it safe though,
        /// this is mainly for graceful handling of special cases.
        pub is_managed_commit: bool,
        /// The workspace represents what `HEAD` is pointing to.
        pub is_entrypoint: bool,
    }

    impl RefInfo {
        /// The `repo` is used just to get ref-names, for convenience.
        pub fn for_ui(
            crate::RefInfo {
                workspace_ref_name,
                stacks,
                target,
                extra_target: _,
                lower_bound: _,
                is_managed_ref,
                is_managed_commit,
                ancestor_workspace_commit: _,
                is_entrypoint,
            }: crate::RefInfo,
            repo: &gix::Repository,
        ) -> anyhow::Result<Self> {
            let remote_names = repo.remote_names();
            let stacks: Vec<_> = stacks
                .into_iter()
                .map(|stack| Stack::for_ui(stack, &remote_names))
                .collect::<Result<_, _>>()?;
            Ok(RefInfo {
                workspace_ref: workspace_ref_name.map(Into::into),
                stacks,
                target: target
                    .map(|t| Target::for_ui(t, &remote_names))
                    .transpose()?,
                is_managed_ref,
                is_managed_commit,
                is_entrypoint,
            })
        }
    }

    /// The UI-clone of [`branch::Stack`].
    #[derive(serde::Serialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Stack {
        /// If the stack belongs to a managed workspace, the `id` will be set and persist.
        /// Otherwise, it is `None`.
        pub id: Option<StackId>,
        /// If there is an integration branch, we know a base commit shared with the integration branch from
        /// which we branched off.
        /// Otherwise, it's the merge-base of all stacks in the current workspace.
        /// It is `None` if this is a stack derived from a branch without relation to any other branch.
        pub base: Option<gix::ObjectId>,
        /// The branch-name denoted segments of the stack from its tip to the point of reference, typically a merge-base.
        /// This array is never empty.
        pub segments: Vec<Segment>,
    }

    impl Stack {
        fn for_ui(
            crate::branch::Stack { id, base, segments }: crate::branch::Stack,
            names: &gix::remote::Names,
        ) -> anyhow::Result<Self> {
            let segments = segments
                .into_iter()
                .map(|s| Segment::for_ui(s, names))
                .collect::<Result<_, _>>()?;
            Ok(Stack { id, base, segments })
        }
    }

    /// A segment of a commit graph, representing a set of commits exclusively.
    #[derive(serde::Serialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct Segment {
        /// The unambiguous or disambiguated name of the branch at the tip of the segment, i.e. at the first commit.
        ///
        /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
        /// a commit anymore that was reached by our rev-walk.
        /// This can happen if the ref is deleted, or if it was advanced by other means.
        /// Alternatively, the naming would have been ambiguous.
        /// Finally, this is `None` of the original name can be found searching upwards, finding exactly one
        /// named segment.
        pub ref_name: Option<BranchReference>,
        /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
        /// Its presence means that a remote is configured and that the stack content
        pub remote_tracking_ref_name: Option<RemoteTrackingReference>,
        /// The portion of commits that can be reached from the tip of the *branch* downwards, so that they are unique
        /// for that stack segment and not included in any other stack or stack segment.
        ///
        /// The list could be empty for when this is a dedicated empty segment as insertion position of commits.
        pub commits: Vec<ui::Commit>,
        /// Commits that are reachable from the remote-tracking branch associated with this branch,
        /// but are not reachable from this branch or duplicated by a commit in it.
        /// Note that commits that are also similar to commits in `commits` are pruned, and not present here.
        ///
        /// Note that remote commits along with their remote tracking branch should always retain a shared history
        /// with the local tracking branch. If these diverge, we can represent this in data, but currently there is
        /// no derived value to make this visible explicitly.
        pub commits_on_remote: Vec<UpstreamCommit>,
        /// All commits *that are not workspace commits* reachable by (and including commits in) this segment.
        /// The list was created by walking all parents, not only the first parent.
        /// This means the segment needs fixing.
        pub commits_outside: Option<Vec<ui::Commit>>,
        /// Read-only metadata with additional information about the branch naming the segment,
        /// or `None` if nothing was present.
        // TODO:
        // pub metadata: Option<ref_metadata::Branch>,
        /// This is `true` a segment in a workspace if the entrypoint of [the traversal](Graph::from_commit_traversal())
        /// is this segment, and the surrounding workspace is provided for context.
        ///
        /// This means one will see the entire workspace, while knowing the focus is on one specific segment.
        /// *Note* that this segment can be listed in *multiple stacks* as it's reachable from multiple 'ahead' segments.
        pub is_entrypoint: bool,
        /// A derived value to help the UI decide which functions to make available.
        pub push_status: ui::PushStatus,
        /// This is always the `first()` commit in `commits` of the next stacksegment, or the first commit of
        /// the first ancestor segment.
        /// It can be imagined as the base upon which the segment is resting, or the connection point to the rest
        /// of the commit-graph along the first parent.
        /// It is `None` if the stack segment contains the first commit in the history, an orphan without ancestry,
        /// or if the history traversal was stopped early.
        // #[serde(with = "gitbutler_serde::object_id_opt")]
        pub base: Option<gix::ObjectId>,
    }

    impl Segment {
        fn for_ui(
            crate::ref_info::Segment {
                ref_name,
                id: _,
                remote_tracking_ref_name,
                commits,
                commits_on_remote,
                commits_outside,
                metadata,
                is_entrypoint,
                push_status,
                base,
            }: crate::ref_info::Segment,
            names: &gix::remote::Names,
        ) -> anyhow::Result<Self> {
            Ok(Segment {
                ref_name: ref_name.map(Into::into),
                remote_tracking_ref_name: remote_tracking_ref_name
                    .map(|r| RemoteTrackingReference::for_ui(r, names))
                    .transpose()?,
                commits: commits.iter().map(Into::into).collect(),
                commits_on_remote: commits_on_remote.iter().map(Into::into).collect(),
                commits_outside: commits_outside.map(|commits| {
                    commits
                        .into_iter()
                        .map(|c| {
                            (&LocalCommit {
                                inner: c,
                                relation: LocalCommitRelation::LocalOnly,
                            })
                                .into()
                        })
                        .collect()
                }),
                // metadata,
                is_entrypoint,
                push_status,
                base,
            })
        }
    }
}

/// This code is a fork of [`gitbutler_branch_actions::author`] to avoid depending on the `gitbutler_branch_actions` crate.
mod author {
    use bstr::ByteSlice;
    use serde::Serialize;

    /// Represents the author of a commit.
    #[derive(Serialize, Hash, Clone, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct Author {
        /// The name from the git commit signature
        pub name: String,
        /// The email from the git commit signature
        pub email: String,
        /// A URL to a gravatar image for the email from the commit signature
        pub gravatar_url: url::Url,
    }

    impl std::fmt::Debug for Author {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{} <{}>", self.name, self.email)
        }
    }

    impl From<git2::Signature<'_>> for Author {
        fn from(value: git2::Signature<'_>) -> Self {
            let name = value.name().unwrap_or_default().to_string();
            let email = value.email().unwrap_or_default().to_string();
            let gravatar_url = gravatar_url_from_email(email.as_str());
            Author {
                name,
                email,
                gravatar_url,
            }
        }
    }

    impl From<gix::actor::SignatureRef<'_>> for Author {
        fn from(value: gix::actor::SignatureRef<'_>) -> Self {
            let gravatar_url = gravatar_url_from_email(&value.email.to_str_lossy());

            Author {
                name: value.name.to_string(),
                email: value.email.to_string(),
                gravatar_url,
            }
        }
    }

    pub fn gravatar_url_from_email(email: &str) -> url::Url {
        let gravatar_url = format!(
            "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
            md5::compute(email.to_lowercase())
        );
        url::Url::parse(gravatar_url.as_str()).expect("an MD5 as part of the URl is always valid")
    }
}
pub use author::Author;
use gitbutler_stack::{Stack, StackId};

/// The information about the branch inside a stack
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackHeadInfo {
    /// The name of the branch.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub name: BString,
    /// The tip of the branch.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// If `true`, then this head is checked directly so `HEAD` points to it, and this is only ever `true` for a single head.
    /// This is `false` if the worktree is checked out.
    pub is_checked_out: bool,
}

/// Represents a lightweight version of a [`Stack`] for listing.
/// NOTE: this is a UI type mostly because it's still modeled after the legacy stack with StackId, something that doesn't exist anymore.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackEntry {
    /// The ID of the stack.
    pub id: Option<StackId>,
    /// The list of the branch information that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub heads: Vec<StackHeadInfo>,
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// The zero-based index for sorting stacks.
    pub order: Option<usize>,
    /// If `true`, then any head in this stack is checked directly so `HEAD` points to it, and this is only ever `true` for a single stack.
    pub is_checked_out: bool,
}

/// **Temporary type to help transitioning to the optional version of stack-entry** and ultimately, to [`crate::RefInfo`].
/// WARNING: for use by parts in the code that can rely on having a non-optional `stack_id`. The goal is to have none of these.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackEntryNoOpt {
    /// The ID of the stack.
    pub id: StackId,
    /// The list of the branch information that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub heads: Vec<StackHeadInfo>,
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// The zero-based index for sorting stacks.
    pub order: Option<usize>,
    /// If `true`, then any head in this stack is checked directly so `HEAD` points to it, and this is only ever `true` for a single stack.
    pub is_checked_out: bool,
}

impl StackEntry {
    /// The name of the stack, which is the name of the top-most branch.
    pub fn name(&self) -> Option<&BStr> {
        self.heads.first().map(|head| head.name.as_ref())
    }
}

impl StackEntryNoOpt {
    /// The name of the stack, which is the name of the top-most branch.
    pub fn name(&self) -> Option<&BStr> {
        self.heads.first().map(|head| head.name.as_ref())
    }
}

impl TryFrom<StackEntry> for StackEntryNoOpt {
    type Error = anyhow::Error;

    fn try_from(
        StackEntry {
            id,
            heads,
            tip,
            order,
            is_checked_out,
        }: StackEntry,
    ) -> Result<Self, Self::Error> {
        let id = id.context("BUG(opt-stack-id)")?;
        Ok(StackEntryNoOpt {
            id,
            heads,
            tip,
            order,
            is_checked_out,
        })
    }
}

impl From<StackEntryNoOpt> for StackEntry {
    fn from(
        StackEntryNoOpt {
            id,
            heads,
            tip,
            order,
            is_checked_out,
        }: StackEntryNoOpt,
    ) -> Self {
        StackEntry {
            id: Some(id),
            heads,
            tip,
            order,
            is_checked_out,
        }
    }
}

impl StackEntry {
    pub(crate) fn try_new(repo: &gix::Repository, stack: &Stack) -> anyhow::Result<Self> {
        Ok(StackEntry {
            id: Some(stack.id),
            heads: crate::stack_heads_info(stack, repo)?,
            tip: stack.head_oid(repo)?,
            order: Some(stack.order),
            is_checked_out: false,
        })
    }
}

impl StackEntryNoOpt {
    pub(crate) fn try_new(repo: &gix::Repository, stack: &Stack) -> anyhow::Result<Self> {
        Ok(StackEntryNoOpt {
            id: stack.id,
            heads: crate::stack_heads_info(stack, repo)?,
            tip: stack.head_oid(repo)?,
            order: Some(stack.order),
            is_checked_out: false,
        })
    }
}

/// Represents the state a commit could be in.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject")]
pub enum CommitState {
    /// The commit is only local
    LocalOnly,
    /// The commit is also present at the remote tracking branch.
    /// This is the commit state if:
    ///  - The commit has been pushed to the remote
    ///  - The commit has been copied from a remote commit (when applying a remote branch)
    ///
    /// This variant carries the remote commit id.
    /// The `remote_commit_id` may be the same as the `id` or it may be different if the local commit has been rebased or updated in another way.
    #[serde(with = "gitbutler_serde::object_id")]
    LocalAndRemote(gix::ObjectId),
    /// The commit is considered integrated.
    /// This should happen when this commit or the contents of this commit is already part of the base.
    Integrated,
}

impl CommitState {
    fn display(&self, id: gix::ObjectId) -> &'static str {
        match self {
            CommitState::LocalOnly => "local",
            CommitState::LocalAndRemote(remote_id) => {
                if *remote_id == id {
                    "local/remote(identity)"
                } else {
                    "local/remote(similarity)"
                }
            }
            CommitState::Integrated => "integrated",
        }
    }
}

/// Commit that is a part of a [`StackBranch`](gitbutler_stack::StackBranch) and, as such, containing state derived in relation to the specific branch.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    /// The OID of the commit.
    #[serde(with = "gitbutler_serde::object_id")]
    pub id: gix::ObjectId,
    /// The parent OIDs of the commit.
    #[serde(with = "gitbutler_serde::object_id_vec")]
    pub parent_ids: Vec<gix::ObjectId>,
    /// The message of the commit.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub message: BString,
    /// Whether the commit is in a conflicted state.
    /// The Conflicted state of a commit is a GitButler concept.
    /// GitButler will perform rebasing/reordering etc without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    pub has_conflicts: bool,
    /// Represents whether the commit is considered integrated, local only,
    /// or local and remote with respect to the branch it belongs to.
    /// Note that remote only commits in the context of a branch are expressed with the [`UpstreamCommit`] struct instead of this.
    pub state: CommitState,
    /// Commit creation time in Epoch milliseconds.
    pub created_at: i128,
    /// The author of the commit.
    pub author: Author,
}

impl TryFrom<gix::Commit<'_>> for Commit {
    type Error = anyhow::Error;
    fn try_from(commit: gix::Commit<'_>) -> Result<Self, Self::Error> {
        Ok(Commit {
            id: commit.id,
            parent_ids: commit.parent_ids().map(|id| id.detach()).collect(),
            message: commit.message_raw_sloppy().into(),
            has_conflicts: false,
            state: CommitState::LocalAndRemote(commit.id),
            created_at: i128::from(commit.time()?.seconds) * 1000,
            author: commit.author()?.into(),
        })
    }
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Commit({short_hex}, {message:?}, {state})",
            short_hex = self.id.to_hex_with_len(7),
            message = self.message.trim().as_bstr(),
            state = self.state.display(self.id)
        )
    }
}

/// Commit that is only at the remote.
/// Unlike the `Commit` struct, there is no knowledge of GitButler concepts like conflicted state etc.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamCommit {
    /// The OID of the commit.
    #[serde(with = "gitbutler_serde::object_id")]
    pub id: gix::ObjectId,
    /// The message of the commit.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub message: BString,
    /// Commit creation time in Epoch milliseconds.
    pub created_at: i128,
    /// The author of the commit.
    pub author: Author,
}

impl std::fmt::Debug for UpstreamCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UpstreamCommit({short_hex}, {message:?})",
            short_hex = self.id.to_hex_with_len(7),
            message = self.message.trim().as_bstr()
        )
    }
}

/// Represents the pushable status for the current stack.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PushStatus {
    /// Can push, but there are no changes to be pushed
    NothingToPush,
    /// Can push. This is the case when there are local changes that can be pushed to the remote.
    UnpushedCommits,
    /// Can push, but requires a force push to the remote because commits were rewritten.
    UnpushedCommitsRequiringForce,
    /// Completely unpushed - there is no remote tracking branch so Git never interacted with the remote.
    CompletelyUnpushed,
    /// Fully integrated, no changes to push.
    Integrated,
}

/// Information about the current state of a branch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDetails {
    /// The name of the branch.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub name: BString,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    #[serde(with = "gitbutler_serde::bstring_opt_lossy")]
    pub remote_tracking_branch: Option<BString>,
    /// Description of the branch.
    /// Can include arbitrary utf8 data, eg. markdown etc.
    pub description: Option<String>,
    /// The pull(merge) request associated with the branch, or None if no such entity has not been created.
    pub pr_number: Option<usize>,
    /// A unique identifier for the GitButler review associated with the branch, if any.
    pub review_id: Option<String>,
    /// This is the last commit in the branch, aka the tip of the branch.
    /// If this is the only branch in the stack or the top-most branch, this is the tip of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// This is the base commit from the perspective of this branch.
    /// If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
    /// If this branch is at the bottom of the stack, this is the merge base of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub base_commit: gix::ObjectId,
    /// The pushable status for the branch.
    pub push_status: PushStatus,
    /// Last time, the branch was updated in Epoch milliseconds.
    pub last_updated_at: Option<i128>,
    /// All authors of the commits in the branch.
    pub authors: Vec<Author>,
    /// Whether the branch is conflicted.
    pub is_conflicted: bool,
    /// The commits contained in the branch, excluding the upstream commits.
    pub commits: Vec<Commit>,
    /// The commits that are only at the remote.
    pub upstream_commits: Vec<UpstreamCommit>,
    /// Whether it's representing a remote head
    pub is_remote_head: bool,
}

/// Information about the current state of a stack
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackDetails {
    /// This is the name of the top-most branch, provided by the API for convenience
    pub derived_name: String,
    /// The pushable status for the stack
    pub push_status: PushStatus,
    /// The details about the contained branches
    pub branch_details: Vec<BranchDetails>,
    /// Whether the stack is conflicted.
    pub is_conflicted: bool,
}

/// Represents a branch in a [`Stack`]. It contains commits derived from the local pseudo branch and it's respective remote
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// The name of the branch.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub name: BString,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    #[serde(with = "gitbutler_serde::bstring_opt_lossy")]
    pub remote_tracking_branch: Option<BString>,
    /// Description of the branch.
    /// Can include arbitrary utf8 data, eg. markdown etc.
    pub description: Option<String>,
    /// The pull(merge) request associated with the branch, or None if no such entity has not been created.
    pub pr_number: Option<usize>,
    /// A unique identifier for the GitButler review associated with the branch, if any.
    pub review_id: Option<String>,
    /// Indicates that the branch was previously part of a stack but it has since been integrated.
    /// In other words, the merge base of the stack is now above this branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    /// An archived branch will not have any commits associated with it.
    pub archived: bool,
    /// This is the last commit in the branch, aka the tip of the branch.
    /// If this is the only branch in the stack or the top-most branch, this is the tip of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// This is the base commit from the perspective of this branch.
    /// If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
    /// If this branch is at the bottom of the stack, this is the merge base of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub base_commit: gix::ObjectId,
}

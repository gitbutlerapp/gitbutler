use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice as _;
use gix::refs::FullNameRef;
use uuid::Uuid;

use crate::{Id, extract_remote_name_and_short_name, git_config};

const PROJECT_TARGET_REF: &str = "gitbutler.project.targetRef";
const PROJECT_TARGET_COMMIT_ID: &str = "gitbutler.project.targetCommitId";
const PROJECT_PUSH_REMOTE: &str = "gitbutler.project.pushRemote";
const PROJECT_PORTED_META: &str = "gitbutler.project.portedMeta";

/// Project-wide metadata stored in repository-local Git configuration.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct ProjectMeta {
    /// The name of the reference to integrate with, if present.
    pub target_ref: Option<gix::refs::FullName>,
    /// A stable target commit that should be included in the workspace.
    pub target_commit_id: Option<gix::ObjectId>,
    /// The symbolic name of the remote to push branches to.
    pub push_remote: Option<String>,
}

/// Metadata about workspaces, associated with references that are designated to a workspace,
/// i.e. `refs/heads/gitbutler/workspaces/<name>`.
/// Such a ref either points to a *Workspace Commit* which we rewrite at will, or a commit
/// owned by the user.
///
/// Note that associating data with the workspace, particularly with its parents, is very safe
/// as the commit is under our control and merges aren't usually changed. However, users could
/// point it to another commit merely by `git checkout` which means our stored data is completely
/// out of sync.
///
/// We would have to detect this case by validating parents, and the refs pointing to it, before
/// using the metadata, or at least have a way to communicate possible states when trying to use
/// this information.
#[derive(Default, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,

    /// An array entry for each parent of the *workspace commit* the last time we saw it, and while it is
    /// considered to be inside the workspace, *or outside of it*.
    /// The first parent, and always the first parent, or the first entry in this list,
    /// could have a tip named `Self::target_ref`, and if so, it's not meant to be visible when asking for stacks.
    pub stacks: Vec<WorkspaceStack>,

    /// The name of the reference to integrate with, if present.
    /// Fetch its metadata for more information.
    ///
    /// If there is no target name, this is a local workspace (and if no global target is set).
    /// Note that even though this is per workspace, the implementation can fill in global information at will.
    target_ref: Option<gix::refs::FullName>,

    /// The commit id of a commit that was reachable by [`Self::target_ref`] and that should be included in the workspace.
    /// This is useful to make workspaces appear stable in relationship to the target reference, which may be updated each
    /// time a `git fetch` is performed.
    ///
    /// This commit id has the same effect as the commit that the [`Self::target_ref`] is pointing to, and they are cumulative,
    /// to include up to two commits of the target in the workspace.
    target_commit_id: Option<gix::ObjectId>,
    /// The symbolic name of the remote to push branches to.
    ///
    /// This is useful when there are no push permissions for the remote behind `target_ref`.
    push_remote: Option<String>,
}

/// A projected workspace stack used to reconcile persisted workspace metadata.
///
/// This is intentionally smaller than the full workspace projection so metadata
/// code does not depend on graph presentation types and only sees what it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedWorkspaceStack {
    /// Existing stable stack id from projection, if one was already known.
    ///
    /// `Some(id)` means reconciliation may use that id to match an existing
    /// metadata stack, or preserve it when creating metadata for a projected
    /// stack that is missing from metadata.
    ///
    /// `None` means reconciliation should create a new stack id if the projected
    /// stack does not match any existing metadata stack.
    pub id: Option<StackId>,
    /// Branch names in stack order, from tip toward base.
    pub branches: Vec<gix::refs::FullName>,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Workspace {
            ref_info,
            stacks,
            target_ref,
            push_remote,
            target_commit_id,
        } = self;
        f.debug_struct("Workspace")
            .field("ref_info", ref_info)
            .field("stacks", stacks)
            .field(
                "target_ref",
                &MaybeDebug(&target_ref.as_ref().map(|rn| rn.as_bstr())),
            )
            .field("target_commit_id", &MaybeDebug(target_commit_id))
            .field("push_remote", &MaybeDebug(push_remote))
            .finish()
    }
}

/// Mutations
impl Workspace {
    /// Create workspace metadata from its parts.
    pub fn new(
        ref_info: RefInfo,
        stacks: Vec<WorkspaceStack>,
        project_meta: ProjectMeta,
    ) -> Workspace {
        let ProjectMeta {
            target_ref,
            target_commit_id,
            push_remote,
        } = project_meta;
        Workspace {
            ref_info,
            stacks,
            target_ref,
            target_commit_id,
            push_remote,
        }
    }

    /// Return metadata that is moving to project-local Git configuration.
    pub fn project_meta(&self) -> ProjectMeta {
        ProjectMeta {
            target_ref: self.target_ref.clone(),
            target_commit_id: self.target_commit_id,
            push_remote: self.push_remote.clone(),
        }
    }

    /// Back-fill the legacy workspace metadata fields.
    pub fn set_project_meta(&mut self, project_meta: ProjectMeta) {
        let ProjectMeta {
            target_ref,
            target_commit_id,
            push_remote,
        } = project_meta;
        self.target_ref = target_ref;
        self.target_commit_id = target_commit_id;
        self.push_remote = push_remote;
    }

    /// Add missing metadata for stacks visible in the current workspace projection.
    ///
    /// This is additive with respect to branch names: projected branches are
    /// added to metadata when missing, existing branch metadata is preserved,
    /// and branches not present in `projected_stacks` are not removed.
    ///
    /// Projected stacks are authoritative for grouping. If a projected branch is
    /// already in another metadata stack, it is moved to the projected stack.
    /// Metadata stacks made empty by such moves are removed. Existing metadata
    /// may contain duplicate branch names across stacks; these are tolerated as
    /// stale hints. Projected branch names must still be unique.
    pub fn reconcile_projected_stacks(
        &mut self,
        projected_stacks: impl IntoIterator<Item = ProjectedWorkspaceStack>,
        mut new_stack_id: impl FnMut(&gix::refs::FullNameRef) -> StackId,
    ) -> Result<()> {
        let projected_stacks = projected_stacks
            .into_iter()
            .filter(|stack| !stack.branches.is_empty())
            .collect::<Vec<_>>();
        ensure_unique_branch_names(
            projected_stacks
                .iter()
                .flat_map(|stack| stack.branches.iter().map(|branch| branch.as_ref())),
            "projected workspace",
        )?;

        for ProjectedWorkspaceStack {
            id: projected_stack_id,
            branches: projected_branches,
        } in projected_stacks
        {
            let owning_stack_idx = projected_stack_id
                .and_then(|id| self.stacks.iter().position(|stack| stack.id == id))
                .or_else(|| {
                    projected_branches.iter().find_map(|branch| {
                        self.find_owner_indexes_by_name(
                            branch.as_ref(),
                            StackKind::AppliedAndUnapplied,
                        )
                        .map(|(stack_idx, _branch_idx)| stack_idx)
                    })
                });

            if let Some(stack_idx) = owning_stack_idx {
                let mut branches = Vec::new();
                for branch in projected_branches {
                    branches.push(
                        remove_branch_from_stacks(&mut self.stacks, stack_idx, branch.as_ref())
                            .unwrap_or(WorkspaceStackBranch {
                                ref_name: branch,
                                archived: false,
                            }),
                    );
                }
                let stack = &mut self.stacks[stack_idx];
                branches.extend(std::mem::take(&mut stack.branches));
                stack.branches = branches;
                stack.workspacecommit_relation = WorkspaceCommitRelation::Merged;
            } else {
                let stack_id = projected_stack_id
                    .unwrap_or_else(|| new_stack_id(projected_branches[0].as_ref()));
                self.stacks.push(WorkspaceStack {
                    id: stack_id,
                    branches: projected_branches
                        .into_iter()
                        .map(|ref_name| WorkspaceStackBranch {
                            ref_name,
                            archived: false,
                        })
                        .collect(),
                    workspacecommit_relation: WorkspaceCommitRelation::Merged,
                });
            }
        }
        self.stacks.retain(|stack| !stack.branches.is_empty());

        Ok(())
    }

    /// Remove the named segment `branch`, which removes the whole stack if it's empty after removing a segment
    /// of that name.
    /// Returns `true` if it was removed or `false` if it wasn't found.
    pub fn remove_segment(&mut self, branch: &FullNameRef) -> bool {
        let Some((stack_idx, segment_idx)) =
            self.find_owner_indexes_by_name(branch, StackKind::AppliedAndUnapplied)
        else {
            return false;
        };

        let stack = &mut self.stacks[stack_idx];
        stack.branches.remove(segment_idx);

        if stack.branches.is_empty() {
            self.stacks.remove(stack_idx);
        }
        true
    }

    /// Remove `branch` from applied workspace metadata, and return `true` if it was removed from the metadata,
    /// or `false` if it wasn't present.
    ///
    /// If `branch` is the only segment in its stack, the whole stack metadata entry is removed so
    /// virtual apply/unapply roundtrips return to the original empty-workspace shape.
    ///
    /// If `branch` is the tip of a multi-segment stack, the remaining stack is marked outside
    /// the workspace because its visible tip was removed. This is necessary
    /// to keep previous stacks alive in metadata, in case they are re-applied later, to return
    /// to the same shape.
    #[expect(clippy::indexing_slicing)]
    pub fn unapply_branch(&mut self, branch: &FullNameRef) -> bool {
        let Some((stack_idx, segment_idx)) =
            self.find_owner_indexes_by_name(branch, StackKind::Applied)
        else {
            return false;
        };

        let stack_len = self.stacks[stack_idx].branches.len();
        if stack_len == 1 {
            // There is nothing to remember, remove the whole stack.
            self.stacks.remove(stack_idx);
        } else if segment_idx == 0 {
            // The tip of the stack should be removed, mark the whole stack as outside, to remember its configuration.
            self.stacks[stack_idx].workspacecommit_relation = WorkspaceCommitRelation::Outside;
        } else {
            // It's a segment in the middle, remove its metadata.
            self.stacks[stack_idx].branches.remove(segment_idx);
        }

        true
    }

    /// Insert `branch` as new stack if it's not yet contained in the workspace and if `order` is not `None` or push
    /// it to the end of the stack list.
    /// Use `relation` to indicate how the new stack should be seen.
    /// If a new stack is created, it's considered to be *in* the workspace. If there is an existing stack, it
    /// may also be marked as *outside the workspace*, we do not change this.
    /// Note that `order` is only relevant at insertion time, not if the branch already exists, and `relation`
    /// is only used if the stack is newly created.
    /// Returns `(stack_id, segment_idx)` of the stack that was either newly created, or already present.
    /// Note that `segment_idx` may be non-0 if `branch` already existed as segment, and the caller has to
    /// deal with this.
    /// Use `new_stack_id` can be used to control the stack id to be assigned, but it generally should be a plain `StackId::generate()`.
    pub fn add_or_insert_new_stack_if_not_present(
        &mut self,
        branch: &FullNameRef,
        order: Option<usize>,
        relation: WorkspaceCommitRelation,
        new_stack_id: impl FnOnce(&gix::refs::FullNameRef) -> StackId,
    ) -> (usize, usize) {
        if let Some(owners) =
            self.find_owner_indexes_by_name(branch, StackKind::AppliedAndUnapplied)
        {
            return owners;
        };

        let stack = WorkspaceStack {
            id: new_stack_id(branch),
            workspacecommit_relation: relation,
            branches: vec![WorkspaceStackBranch {
                ref_name: branch.to_owned(),
                archived: false,
            }],
        };
        let stack_idx = match order.map(|idx| idx.min(self.stacks.len())) {
            None => {
                let idx = self.stacks.len();
                self.stacks.push(stack);
                idx
            }
            Some(existing_index) => {
                self.stacks.insert(existing_index, stack);
                existing_index
            }
        };
        (stack_idx, 0)
    }

    /// Insert `branch` as new segment if it's not yet contained in the workspace,
    /// and insert it above the given `anchor` segment name, which maybe the tip of a stack or any segment within one
    /// Returns `true` if the ref was newly added, or `false` if it already existed, or `None` if `anchor` didn't exist.
    pub fn insert_new_segment_above_anchor_if_not_present(
        &mut self,
        branch: &FullNameRef,
        anchor: &FullNameRef,
    ) -> Option<bool> {
        if self.contains_ref(branch, StackKind::AppliedAndUnapplied) {
            return Some(false);
        };
        let (stack_idx, segment_idx) =
            self.find_owner_indexes_by_name(anchor, StackKind::AppliedAndUnapplied)?;
        self.stacks[stack_idx].branches.insert(
            segment_idx,
            WorkspaceStackBranch {
                ref_name: branch.to_owned(),
                archived: false,
            },
        );
        Some(true)
    }
}

impl ProjectMeta {
    /// Return [`Self::target_ref`], or a [`DefaultTargetNotFound`](but_error::Code::DefaultTargetNotFound)
    /// error if no target is configured.
    pub fn target_ref_or_err(&self) -> Result<&gix::refs::FullName> {
        self.target_ref.as_ref().ok_or_else(|| {
            anyhow::anyhow!("there is no default target")
                .context(but_error::Code::DefaultTargetNotFound)
        })
    }

    /// Return [`Self::target_commit_id`], or a [`DefaultTargetNotFound`](but_error::Code::DefaultTargetNotFound)
    /// error if no target commit is known.
    pub fn target_commit_id_or_err(&self) -> Result<gix::ObjectId> {
        self.target_commit_id.ok_or_else(|| {
            anyhow::anyhow!("there is no default target commit")
                .context(but_error::Code::DefaultTargetNotFound)
        })
    }

    /// The name of the remote to push to: [`Self::push_remote`], falling back to the
    /// remote behind [`Self::target_ref`].
    ///
    /// If no configured remote matches the target ref, fall back to the first path component
    /// after `refs/remotes/`, the textual remote name that legacy metadata stored verbatim.
    pub fn push_remote_name(&self, repo: &gix::Repository) -> Result<String> {
        if let Some(name) = self.push_remote.clone() {
            return Ok(name);
        }
        let target_ref = self.target_ref_or_err()?;
        if let Some((remote_name, _short_name)) =
            extract_remote_name_and_short_name(target_ref.as_ref(), &repo.remote_names())
        {
            return Ok(remote_name);
        }
        let (category, short_name) = target_ref
            .category_and_short_name()
            .with_context(|| format!("failed to determine remote for branch {target_ref}"))?;
        if category != gix::refs::Category::RemoteBranch {
            bail!("failed to determine remote for non remote-tracking branch {target_ref}");
        }
        let slash_pos = short_name.find_byte(b'/').with_context(|| {
            format!("remote tracking branch {target_ref} didn't have '/' in its short name")
        })?;
        let remote_name = short_name[..slash_pos].to_str_lossy().into_owned();
        tracing::warn!(
            "remote '{remote_name}' of target ref {target_ref} is not configured in git config"
        );
        Ok(remote_name)
    }

    /// Get the fetch URL of the remote behind [`Self::target_ref`].
    pub fn remote_url_with_fallback(&self, repo: &gix::Repository) -> Result<String> {
        let Some(target_ref) = self.target_ref.as_ref() else {
            bail!("Target ref required for remote url")
        };
        let remote_names = repo.remote_names();
        let (remote_name, _short_name) =
            extract_remote_name_and_short_name(target_ref.as_ref(), &remote_names).context(
                format!("failed to determine remote for branch {target_ref}"),
            )?;
        let remote = repo.find_remote(remote_name.as_str()).context(format!(
            "failed to find remote {remote_name} for branch {target_ref}"
        ))?;
        remote
            .url(gix::remote::Direction::Fetch)
            .map(|url| url.to_bstring().to_string())
            .context(format!("failed to get fetch url for remote {remote_name}"))
    }

    /// The URL to push to, inferred by the [`Self::push_remote`] property.
    ///
    /// Falls back to the fetch URL of the remote behind [`Self::target_ref`] if
    /// the push remote is not set.
    pub fn push_remote_url(&self, repo: &gix::Repository) -> Result<String> {
        let push_remote_url = match self.push_remote {
            Some(ref name) => repo.find_remote(name.as_str()).ok().and_then(|remote| {
                remote
                    .url(gix::remote::Direction::Push)
                    .or_else(|| remote.url(gix::remote::Direction::Fetch))
                    .map(|url| url.to_bstring().to_string())
            }),
            None => None,
        };

        push_remote_url
            .map(Ok)
            .unwrap_or_else(|| self.remote_url_with_fallback(repo))
    }
}

impl ProjectMeta {
    /// Read project metadata for `repo`, falling back to the legacy workspace metadata in `meta`
    /// when it wasn't ported to Git configuration yet.
    ///
    /// This re-reads the repository-local configuration file from disk so that changes made
    /// through other repository handles or by other processes are always observed.
    /// It never writes - porting happens when project metadata is persisted.
    pub fn resolve(repo: &gix::Repository, meta: &impl crate::RefMetadata) -> anyhow::Result<Self> {
        let config = git_config::open_repo_local_config_for_reading(repo)?;
        if Self::is_ported(&config) {
            return Self::try_from_config(&config);
        }
        Self::from_legacy_meta(meta)
    }

    /// Like [`Self::resolve()`], but obtain the legacy metadata through `legacy_fallback` so it
    /// is only constructed when the repository wasn't ported to Git configuration yet.
    ///
    /// Prefer this over [`Self::resolve()`] when creating the legacy metadata is costly.
    pub fn resolve_with<M: crate::RefMetadata>(
        repo: &gix::Repository,
        legacy_fallback: impl FnOnce() -> anyhow::Result<M>,
    ) -> anyhow::Result<Self> {
        let config = git_config::open_repo_local_config_for_reading(repo)?;
        if Self::is_ported(&config) {
            return Self::try_from_config(&config);
        }
        Self::from_legacy_meta(&legacy_fallback()?)
    }

    fn from_legacy_meta(meta: &impl crate::RefMetadata) -> anyhow::Result<Self> {
        Ok(meta
            .workspace(crate::WORKSPACE_REF_NAME.try_into()?)?
            .project_meta())
    }

    /// Read project metadata from the given repository-local Git configuration.
    ///
    /// Malformed values are tolerated: a target ref that doesn't parse as a full ref name or
    /// isn't a remote tracking branch, or a target commit id that isn't a valid object id, are
    /// ignored with a warning so one bad hand-edited value doesn't make the project unusable.
    /// Errors only surface where a target is actually required, like [`Self::target_ref_or_err()`].
    pub fn try_from_config(config: &gix::config::File<'_>) -> anyhow::Result<Self> {
        let target_ref = config.string(PROJECT_TARGET_REF).and_then(|value| {
            match gix::refs::FullName::try_from(value.as_ref()) {
                Ok(name) if name.category() == Some(gix::refs::Category::RemoteBranch) => {
                    Some(name)
                }
                Ok(name) => {
                    tracing::warn!(
                        "ignoring {PROJECT_TARGET_REF}: target ref {name} is not a remote tracking branch"
                    );
                    None
                }
                Err(err) => {
                    tracing::warn!(
                        "ignoring {PROJECT_TARGET_REF} '{value}' that failed to parse as a full ref name: {err}"
                    );
                    None
                }
            }
        });
        let target_commit_id = config
            .string(PROJECT_TARGET_COMMIT_ID)
            .and_then(
                |value| match gix::ObjectId::from_hex(value.as_ref()) {
                    Ok(id) => Some(id),
                    Err(err) => {
                        tracing::warn!(
                            "ignoring {PROJECT_TARGET_COMMIT_ID} '{value}' that failed to parse as an object id: {err}"
                        );
                        None
                    }
                },
            )
            // The null id is a placeholder for an unknown commit in storage that
            // cannot represent absence - interpret it as such.
            .filter(|id| !id.is_null());
        let push_remote = config
            .string(PROJECT_PUSH_REMOTE)
            .map(|value| value.to_string());
        Ok(ProjectMeta {
            target_ref,
            target_commit_id,
            push_remote,
        })
    }

    /// Return whether project metadata has already been ported to Git config.
    pub fn is_ported(config: &gix::config::File<'_>) -> bool {
        matches!(config.boolean(PROJECT_PORTED_META), Some(Ok(true)))
    }

    /// Return whether project metadata has already been ported to the repository-local
    /// Git configuration of `repo`, re-reading it from disk.
    ///
    /// This is the cheap way to check the ported marker without resolving any metadata.
    pub fn is_ported_repo(repo: &gix::Repository) -> anyhow::Result<bool> {
        let config = git_config::open_repo_local_config_for_reading(repo)?;
        Ok(Self::is_ported(&config))
    }

    /// Persist project metadata to repository-local Git config and mark it as ported.
    pub fn persist_to_local_config(&self, repo: &gix::Repository) -> anyhow::Result<()> {
        git_config::edit_repo_config(repo, gix::config::Source::Local, |config| {
            set_or_remove(
                config,
                PROJECT_TARGET_REF,
                self.target_ref.as_ref().map(ToString::to_string),
            )?;
            set_or_remove(
                config,
                PROJECT_TARGET_COMMIT_ID,
                self.target_commit_id
                    .filter(|id| !id.is_null())
                    .map(|id| id.to_string()),
            )?;
            set_or_remove(config, PROJECT_PUSH_REMOTE, self.push_remote.as_deref())?;
            git_config::set_config_value(config, PROJECT_PORTED_META, "true")?;
            Ok(())
        })?;
        Ok(())
    }

    /// Remove all project metadata keys, including the ported marker, from the
    /// repository-local Git configuration of `repo`.
    ///
    /// Afterwards [`Self::resolve()`] falls back to the legacy workspace metadata again.
    pub fn remove_from_local_config(repo: &gix::Repository) -> anyhow::Result<()> {
        git_config::edit_repo_config(repo, gix::config::Source::Local, |config| {
            for key in [
                PROJECT_TARGET_REF,
                PROJECT_TARGET_COMMIT_ID,
                PROJECT_PUSH_REMOTE,
                PROJECT_PORTED_META,
            ] {
                git_config::remove_config_value(config, key)?;
            }
            Ok(())
        })?;
        Ok(())
    }
}

fn set_or_remove(
    config: &mut gix::config::File<'static>,
    key: &str,
    value: Option<impl AsRef<str>>,
) -> anyhow::Result<()> {
    match value {
        Some(value) => git_config::set_config_value(config, key, value.as_ref())?,
        None => git_config::remove_config_value(config, key)?,
    }
    Ok(())
}

fn ensure_unique_branch_names<'a>(
    names: impl IntoIterator<Item = &'a gix::refs::FullNameRef>,
    source: &str,
) -> Result<()> {
    let mut seen = Vec::<gix::refs::FullName>::new();
    for name in names {
        if seen.iter().any(|seen| seen.as_ref() == name) {
            bail!("Cannot reconcile {source}: branch name '{name}' occurs more than once");
        }
        seen.push(name.to_owned());
    }
    Ok(())
}

fn remove_branch_from_stacks(
    stacks: &mut [WorkspaceStack],
    preferred_stack_idx: usize,
    name: &gix::refs::FullNameRef,
) -> Option<WorkspaceStackBranch> {
    if let Some(stack) = stacks.get_mut(preferred_stack_idx)
        && let Some(branch_idx) = stack
            .branches
            .iter()
            .position(|branch| branch.ref_name.as_ref() == name)
    {
        return Some(stack.branches.remove(branch_idx));
    }

    stacks.iter_mut().find_map(|stack| {
        let branch_idx = stack
            .branches
            .iter()
            .position(|branch| branch.ref_name.as_ref() == name)?;
        Some(stack.branches.remove(branch_idx))
    })
}

/// Determine what kind of stack a query operation is interested in.
#[derive(Debug, Clone, Copy)]
pub enum StackKind {
    /// Find stacks that are meant to be applied only.
    Applied,
    /// Find all stacks.
    AppliedAndUnapplied,
}

/// Access
impl Workspace {
    /// The name of the reference to integrate with, if present.
    pub fn target_ref(&self) -> Option<&gix::refs::FullName> {
        self.target_ref.as_ref()
    }

    /// The stable target commit that should be included in the workspace.
    pub fn target_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_commit_id
    }

    /// The symbolic name of the remote to push branches to.
    pub fn push_remote(&self) -> Option<&str> {
        self.push_remote.as_deref()
    }

    /// Return all stacks that are supposed to be inside the workspace, i.e. applied.
    /// Use `kind` for filtering.
    pub fn stacks(&self, kind: StackKind) -> impl Iterator<Item = &WorkspaceStack> {
        self.stacks.iter().filter(move |s| {
            if matches!(kind, StackKind::Applied) {
                s.workspacecommit_relation.is_in_workspace()
            } else {
                true
            }
        })
    }

    /// Return all stacks that are supposed to be inside the workspace as mutable reference, i.e. applied.
    /// Use `kind` for filtering.
    pub fn stacks_mut(&mut self, kind: StackKind) -> impl Iterator<Item = &mut WorkspaceStack> {
        self.stacks.iter_mut().filter(move |s| {
            if matches!(kind, StackKind::Applied) {
                s.workspacecommit_relation.is_in_workspace()
            } else {
                true
            }
        })
    }

    /// Return the names of the tips of all stacks in the workspace.
    /// Use `kind` for filtering.
    pub fn stack_names(&self, kind: StackKind) -> impl Iterator<Item = &gix::refs::FullNameRef> {
        self.stacks(kind)
            .filter_map(|s| s.ref_name().map(|rn| rn.as_ref()))
    }

    /// Return `true` if the branch with `name` is the workspace target or the targets local tracking branch,
    /// using `repo` for the lookup of the local tracking branch.
    pub fn is_branch_the_target_or_its_local_tracking_branch(
        &self,
        name: &gix::refs::FullNameRef,
        repo: &gix::Repository,
    ) -> anyhow::Result<bool> {
        let Some(target_ref) = self.target_ref.as_ref() else {
            return Ok(false);
        };

        if target_ref.as_ref() == name {
            Ok(true)
        } else {
            let Some((local_tracking_branch, _remote_name)) =
                repo.upstream_branch_and_remote_for_tracking_branch(target_ref.as_ref())?
            else {
                return Ok(false);
            };
            Ok(local_tracking_branch.as_ref() == name)
        }
    }

    /// Return `true` if `name` is an reference mentioned in our [stacks](Workspace::stacks).
    /// Use `kind` for filtering.
    pub fn contains_ref(&self, name: &gix::refs::FullNameRef, kind: StackKind) -> bool {
        self.stacks(kind)
            .any(|stack| stack.branches.iter().any(|b| b.ref_name.as_ref() == name))
    }

    /// Find a given `name` within our stack branches and return it for modification.
    /// Use `kind` for filtering.
    pub fn find_branch_mut(
        &mut self,
        name: &gix::refs::FullNameRef,
        kind: StackKind,
    ) -> Option<&mut WorkspaceStackBranch> {
        self.stacks_mut(kind).find_map(|stack| {
            stack
                .branches
                .iter_mut()
                .find(|b| b.ref_name.as_ref() == name)
        })
    }

    /// Find a given `name` within our stack branches and return it.
    /// Use `kind` for filtering.
    pub fn find_branch(
        &self,
        name: &gix::refs::FullNameRef,
        kind: StackKind,
    ) -> Option<&WorkspaceStackBranch> {
        self.stacks(kind)
            .find_map(|stack| stack.branches.iter().find(|b| b.ref_name.as_ref() == name))
    }

    /// Find a given `name` within our stack branches and return the stack itself.
    /// Use `kind` for filtering.
    pub fn find_stack_with_branch(
        &self,
        name: &gix::refs::FullNameRef,
        kind: StackKind,
    ) -> Option<&WorkspaceStack> {
        self.stacks(kind).find_map(|stack| {
            stack
                .branches
                .iter()
                .find_map(|b| (b.ref_name.as_ref() == name).then_some(stack))
        })
    }

    /// Find the `(stack_idx, branch_idx)` of `name` within *all* stack branches
    /// if `kind` is [StackKind::AppliedAndUnapplied], or those that are in the workspace
    /// for direct access like `ws.stacks[stack_idx].branches[branch_idx]`.
    pub fn find_owner_indexes_by_name(
        &self,
        name: &gix::refs::FullNameRef,
        kind: StackKind,
    ) -> Option<(usize, usize)> {
        self.stacks
            .iter()
            .enumerate()
            .filter(|(_, stack)| {
                matches!(kind, StackKind::AppliedAndUnapplied)
                    || stack.workspacecommit_relation.is_in_workspace()
            })
            .find_map(|(stack_idx, stack)| {
                stack.branches.iter().enumerate().find_map(|(seg_idx, b)| {
                    (b.ref_name.as_ref() == name).then_some((stack_idx, seg_idx))
                })
            })
    }
}

/// Metadata about branches, associated with any Git branch.
#[derive(serde::Serialize, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Standard data we want to know about any ref.
    pub ref_info: RefInfo,
    /// Information about possibly ongoing reviews in various forges.
    pub review: Review,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(Branch);

/// Mutations
impl Branch {
    /// Claim that we now updated the branch in some way, and possibly also set the created time
    /// if `is_new_ref` is `true`
    pub fn update_times(&mut self, is_new_ref: bool) {
        self.ref_info.set_updated_to_now();
        if is_new_ref {
            self.ref_info.set_created_to_now();
        }
    }
}

impl std::fmt::Debug for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const DEFAULT_IN_TESTSUITE: gix::date::Time = gix::date::Time {
            seconds: 0,
            offset: 0,
        };
        let mut d = f.debug_struct("Branch");
        if self
            .ref_info
            .created_at
            .is_some_and(|t| t != DEFAULT_IN_TESTSUITE)
            || self
                .ref_info
                .updated_at
                .is_some_and(|t| t != DEFAULT_IN_TESTSUITE)
            || self.review.pull_request.is_some()
        {
            d.field("ref_info", &self.ref_info)
                .field("review", &self.review);
        }
        d.finish()
    }
}

/// A utility to prevent `Option` from being too verbose in debug printings.
pub struct MaybeDebug<'a, T: std::fmt::Debug>(pub &'a Option<T>);

impl<T: std::fmt::Debug> std::fmt::Debug for MaybeDebug<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            None => f.write_str("None"),
            Some(dbg) => dbg.fmt(f),
        }
    }
}

/// Basic information to know about a reference we store with the metadata system.
///
/// It allows keeping track of when it changed, but also if we created it initially, a useful
/// bit of information.
#[derive(serde::Serialize, Default, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-schema", schemars(rename = "MetadataRefInfo"))]
pub struct RefInfo {
    /// The time of creation, *if we created the reference*.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::gix_time_opt")
    )]
    pub created_at: Option<gix::date::Time>,
    /// The time at which the reference was last modified if we modified it.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::gix_time_opt")
    )]
    pub updated_at: Option<gix::date::Time>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RefInfo);

/// Mutations
impl RefInfo {
    /// Set the `updated_at` field to the current time.
    pub fn set_updated_to_now(&mut self) {
        self.updated_at = Some(gix::date::Time::now_local_or_utc());
    }
    /// Set the `created_at` field to the current time.
    pub fn set_created_to_now(&mut self) {
        self.created_at = Some(gix::date::Time::now_local_or_utc());
    }
}

impl std::fmt::Debug for RefInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format = gix::date::time::format::ISO8601;
        write!(
            f,
            "RefInfo {{ created_at: {:?}, updated_at: {:?} }}",
            MaybeDebug(&self.created_at.map(|date| date.format_or_unix(format))),
            MaybeDebug(&self.updated_at.map(|date| date.format_or_unix(format))),
        )
    }
}

/// Access
impl RefInfo {
    /// If `true`, this means we created the branch as part of creating a new stack.
    /// This means we may also remove it and its remote tracking branch if it's removed
    /// from the stack *and* integrated.
    pub fn is_managed(&self) -> bool {
        self.created_at.is_some()
    }
}

/// The ID of a stack for somewhat stable identification of ever-changing stacks.
pub type StackId = Id<'S'>;

impl StackId {
    /// A special fixed ID used to represent the lane in single branch mode.
    ///
    /// It can be used like an ordinary ID, with the note that the contents of said stack
    /// can change drastically with each checkout.
    ///
    /// Single branch mode happens when no `gitbutler/workspace` reference is checked out,
    /// or any branch that is between that and the lowest base of the workspace.
    pub fn single_branch_id() -> Self {
        Self::from(Uuid::from_u128(1))
    }
}

/// A stack that was, at some point in time, applied to the workspace, i.e. a parent of the *workspace commit*.
/// Note that if `in_workspace` is `false`, it's not considered unapplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceStack {
    /// A unique and stable identifier for the stack itself.
    pub id: StackId,
    /// All branches that were reachable from the tip of the stack that at the time it was merged into
    /// the *workspace commit*.
    /// `[0]` is the first reachable branch, usually the tip of the stack, and `[N]` is the last
    /// reachable branch before reaching the merge-base among all stacks or the `target_ref`.
    ///
    /// Thus, branches are stored in traversal order, from the tip towards the base.
    pub branches: Vec<WorkspaceStackBranch>,
    /// How the stack acts in relation to the workspace commit.
    pub workspacecommit_relation: WorkspaceCommitRelation,
}

/// The relationship that a [WorkspaceStack] *supposedly* has with a workspace commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceCommitRelation {
    /// The stack is considered to be merged into the workspace commit, with its tree being observable
    /// in the worktree associated with the workspace reference.
    Merged,
    /// The stack is supposed to be in the workspace commit, but its tree either isn't observable
    /// in the worktree associated with the workspace reference, or it's observable only from the given `commit`.
    ///
    /// Stacks with this relationship can never conflict with any other `Merged` stack, if their commit is `None`.
    MergeFrom {
        /// If `None`, the tree of the stack isn't observable at all in the workspace tree, even though it's stack tip
        /// is a parent of the workspace commit. This is useful to 'mute' stacks, without losing track of them.
        ///
        /// If `Some(commit_id)`, the `commit_id^{tree}` is merged into the workspace tree, while the tip of the stack
        /// is a parent of the workspace. `commit_id` is assumed to be in the stack, and observable by the user.
        /// This is useful to view a different stack segment, not only the tip/top-most commit.
        commit_id: Option<gix::ObjectId>,
    },
    /// The stack may have previously been merged into the workspace commit,
    /// and is considered *outside* of the workspace.
    ///
    /// The reason we have to keep stacks that aren't in the workspace is to keep
    /// information about their constituent branches, as well as their stack-ids which should remain as stable as possible.
    /// It's notable that stack-ids will change, and it's not possible overall to have a stack identity as such as
    /// the contained branches can be reshuffled at will.
    Outside,
}

impl WorkspaceCommitRelation {
    /// Return `true` if this relation suggests that the owning stack is reachable from the workspace commit.
    pub fn is_in_workspace(&self) -> bool {
        match self {
            WorkspaceCommitRelation::Merged | WorkspaceCommitRelation::MergeFrom { .. } => true,
            WorkspaceCommitRelation::Outside => false,
        }
    }
}

/// A branch within a [`WorkspaceStack`], holding per-branch metadata that is
/// stored alongside a stack that is available in a workspace.
#[derive(Clone, PartialEq, Eq)]
pub struct WorkspaceStackBranch {
    /// The name of the branch.
    pub ref_name: gix::refs::FullName,
    /// If `true`, the branch is now underneath the lower-base of the workspace after a workspace update.
    /// This means it's not interesting anymore, by all means, but we'd still have to keep it available and list
    /// these segments as being part of the workspace when creating PRs. Their descriptions contain references
    /// to archived segments, which simply shouldn't disappear from PRs just yet.
    /// However, they must disappear once the whole stack has been integrated and the workspace has moved past it.
    /// Note that this flag must be stored with the workspace as it must survive the deletion of a reference.
    pub archived: bool,
}

impl std::fmt::Debug for WorkspaceStackBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let WorkspaceStackBranch { ref_name, archived } = self;
        f.debug_struct("WorkspaceStackBranch")
            .field("ref_name", &ref_name.as_bstr())
            .field("archived", archived)
            .finish()
    }
}

impl WorkspaceStack {
    /// The name of the stack itself, if it exists.
    pub fn ref_name(&self) -> Option<&gix::refs::FullName> {
        self.branches.first().map(|b| &b.ref_name)
    }

    /// The same as [`ref_name()`](Self::ref_name()), but returns an actual `Ref`.
    pub fn name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_name().map(|rn| rn.as_ref())
    }

    /// Return `true` if this relation suggests that the owning stack is reachable from the workspace commit.
    pub fn is_in_workspace(&self) -> bool {
        self.workspacecommit_relation.is_in_workspace()
    }
}

/// Metadata about branches, associated with any Git branch.
#[derive(serde::Serialize, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct Review {
    /// The number for the PR that was associated with this branch.
    pub pull_request: Option<usize>,
    /// A handle to the review created with the GitButler review system.
    pub review_id: Option<String>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(Review);

impl std::fmt::Debug for Review {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Review {{ pull_request: {:?}, review_id: {:?} }}",
            MaybeDebug(&self.pull_request),
            MaybeDebug(&self.review_id)
        )
    }
}

/// Additional information about the RefMetadata value itself.
pub trait ValueInfo {
    /// Return `true` if the value didn't exist for a given `ref_name` and thus was defaulted.
    fn is_default(&self) -> bool;
}

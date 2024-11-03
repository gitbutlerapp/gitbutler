use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use git2::Commit;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_commit::commit_ext::CommitVecExt;
use gitbutler_id::id::Id;
use gitbutler_reference::{normalize_branch_name, Refname, RemoteRefname, VirtualRefname};
use gitbutler_repo::{LogUntil, RepositoryExt};
use gix::validate::reference::name_partial;
use gix_utils::str::decompose;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::heads::add_head;
use crate::heads::get_head;
use crate::heads::remove_head;
use crate::Branch;
use crate::CommitOrChangeId;
use crate::ForgeIdentifier;
use crate::Series;
use crate::{ownership::BranchOwnershipClaims, VirtualBranchesHandle};

pub type StackId = Id<Stack>;

// this is the struct for the virtual branch data that is stored in our data
// store. it is more or less equivalent to a git branch reference, but it is not
// stored or accessible from the git repository itself. it is stored in our
// session storage under the branches/ directory.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Stack {
    pub id: StackId,
    /// A user-specified name with no restrictions.
    /// It will be normalized except to be a valid [ref-name](Branch::refname()) if named `refs/gitbutler/<normalize(name)>`.
    pub name: String,
    pub notes: String,
    /// If set, this means this virtual branch was originally created from `Some(branch)`.
    /// It can be *any* branch.
    pub source_refname: Option<Refname>,
    /// The local tracking branch, holding the state of the remote.
    pub upstream: Option<RemoteRefname>,
    // upstream_head is the last commit on we've pushed to the upstream branch
    #[serde(with = "gitbutler_serde::oid_opt", default)]
    pub upstream_head: Option<git2::Oid>,
    #[serde(
        serialize_with = "serialize_u128",
        deserialize_with = "deserialize_u128"
    )]
    pub created_timestamp_ms: u128,
    #[serde(
        serialize_with = "serialize_u128",
        deserialize_with = "deserialize_u128"
    )]
    pub updated_timestamp_ms: u128,
    /// tree is the last git tree written to a session, or merge base tree if this is new. use this for delta calculation from the session data
    #[serde(with = "gitbutler_serde::oid")]
    pub tree: git2::Oid,
    /// head is id of the last "virtual" commit in this branch
    #[serde(with = "gitbutler_serde::oid")]
    head: git2::Oid,
    pub ownership: BranchOwnershipClaims,
    // order is the number by which UI should sort branches
    pub order: usize,
    // is Some(timestamp), the branch is considered a default destination for new changes.
    // if more than one branch is selected, the branch with the highest timestamp wins.
    pub selected_for_changes: Option<i64>,
    #[serde(default = "default_true")]
    pub allow_rebasing: bool,
    /// This is the new metric for determining whether the branch is in the workspace, which means it's applied
    /// and its effects are available to the user.
    #[serde(default = "default_true")]
    pub in_workspace: bool,
    #[serde(default)]
    pub not_in_workspace_wip_change_id: Option<String>,
    /// Represents the Stack state of pseudo-references ("heads").
    /// Do **NOT** edit this directly, instead use the `Stack` trait in gitbutler_stack.
    #[serde(default)]
    pub heads: Vec<Branch>,
}

fn default_true() -> bool {
    true
}

fn serialize_u128<S>(x: &u128, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&x.to_string())
}

fn deserialize_u128<'de, D>(d: D) -> Result<u128, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    let x: u128 = s.parse().map_err(serde::de::Error::custom)?;
    Ok(x)
}

/// A (series) Stack represents multiple "branches" that are dependent on each other in series.
///
/// An initialized Stack must:
/// - have at least one head (branch)
/// - include only references that are part of the stack
/// - always have its commits under a reference i.e. no orphaned commits
///
/// This operates via a list of PatchReferences (heads) that are an attribute of gitbutler_branch::Branch.
/// In this context a (virtual) "Branch" is a stack of PatchReferences, each pointing to a commit (or change) within the stack.
///
/// This trait provides a defined interface for interacting with a Stack, the field `heads` on `Branch` should never be modified directly
/// outside the trait implementation.
///
/// The heads must always be sorted in accordance with the order of the patches in the stack.
/// The first patches are in the beginning of the list and the most recent patches are at the end of the list (top of the stack)
/// Similarly, heads that point to earlier commits are first in the order, and the last head always points to the most recent patch.
/// If there are multiple heads that point to the same patch, the `add` and `update` operations can specify the intended order.
impl Stack {
    /// Creates a new `Branch` with the given name. The `in_workspace` flag is set to `true`.
    #[allow(clippy::too_many_arguments)]
    #[deprecated(note = "DO NOT USE THIS DIRECTLY, use `stack_ext::StackExt::create` instead.")]
    pub fn new(
        name: String,
        source_refname: Option<Refname>,
        upstream: Option<RemoteRefname>,
        upstream_head: Option<git2::Oid>,
        tree: git2::Oid,
        head: git2::Oid,
        order: usize,
        selected_for_changes: Option<i64>,
        allow_rebasing: bool,
    ) -> Self {
        let now = gitbutler_time::time::now_ms();
        Self {
            id: StackId::generate(),
            name,
            notes: String::new(),
            source_refname,
            upstream,
            upstream_head,
            created_timestamp_ms: now,
            updated_timestamp_ms: now,
            tree,
            head,
            ownership: BranchOwnershipClaims::default(),
            order,
            selected_for_changes,
            allow_rebasing,
            in_workspace: true,
            not_in_workspace_wip_change_id: None,
            heads: Default::default(),
        }
    }

    pub fn refname(&self) -> anyhow::Result<VirtualRefname> {
        self.try_into()
    }

    pub fn head(&self) -> git2::Oid {
        self.head
    }

    fn set_head(&mut self, head: git2::Oid) {
        self.head = head;
    }

    // TODO: When this is stable, make it error out on initialization failure
    /// Constructs and initializes a new Stack.
    /// If initialization fails, a warning is logged and the stack is returned as is.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        ctx: &CommandContext,
        name: String,
        source_refname: Option<Refname>,
        upstream: Option<RemoteRefname>,
        upstream_head: Option<git2::Oid>,
        tree: git2::Oid,
        head: git2::Oid,
        order: usize,
        selected_for_changes: Option<i64>,
        allow_rebasing: bool,
        allow_duplicate_refs: bool,
    ) -> Self {
        #[allow(deprecated)]
        // this should be the only place (other than tests) where this is allowed
        let mut branch = Stack::new(
            name,
            source_refname,
            upstream,
            upstream_head,
            tree,
            head,
            order,
            selected_for_changes,
            allow_rebasing,
        );
        if let Err(e) = branch.initialize(ctx, allow_duplicate_refs) {
            // TODO: When this is stable, make it error out
            tracing::warn!("failed to initialize stack: {:?}", e);
        }
        branch
    }

    /// Returns the commits between the stack head (including) and the merge base (not including) for the stack.
    /// The commits are ordered from most recent to oldest.
    ///
    /// E.g. `[ 3, 2, 1 ]` where `3` is the branch head, and `1` is the oldest commit with the merge base as it's parent
    ///
    /// # Errors
    /// - If a merge base cannot be found
    /// - If logging between the head and merge base fails
    pub fn commits<'a>(&'a self, ctx: &'a CommandContext) -> Result<Vec<Commit>> {
        let repo = ctx.repository();
        let stack_commits = repo.log(
            self.head(),
            LogUntil::Commit(self.merge_base(ctx)?.id()),
            false,
        )?;
        Ok(stack_commits)
    }

    /// Returns the commits between the stack head (including) and the merge base (including) for the stack.
    /// The commits are ordered from most recent to oldest.
    ///
    /// E.g. `[ 3, 2, 1 ]` where `3` is the branch head, and `1` is the oldest commit with the merge base as it's parent
    ///
    /// # Errors
    /// - If a merge base cannot be found
    /// - If logging between the head and merge base fails
    pub fn commits_with_merge_base<'a>(&'a self, ctx: &'a CommandContext) -> Result<Vec<Commit>> {
        let mut commits = self.commits(ctx)?;
        commits.push(self.merge_base(ctx)?);
        Ok(commits)
    }

    /// Returns the merge base of the stack head and the project's target branch.
    /// The merge base is the common ancestor of the stack head and the project's target branch.
    ///
    /// # Errors
    /// - If a target is not set for the project
    /// - If the head commit of the stack is not found
    pub fn merge_base<'a>(&'a self, ctx: &'a CommandContext) -> Result<Commit> {
        let default_target = branch_state(ctx).get_default_target()?;
        let repo = ctx.repository();
        let merge_base = repo.merge_base(self.head(), default_target.sha)?;
        let merge_base = repo.find_commit(merge_base)?;
        Ok(merge_base)
    }

    /// An initialized stack has at least one head (branch).
    ///
    /// # Errors
    /// - If the stack has not been initialized
    fn initialized(&self) -> Result<()> {
        if self.heads.is_empty() {
            return Err(anyhow!("Stack has not been initialized"));
        }
        Ok(())
    }
    /// Initializes a new stack.
    /// An initialized stack means that the heads will always have at least one entry.
    /// When initialized, first stack head will point to the "Branch" head.
    /// Errors out if the stack has already been initialized.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    pub fn initialize(&mut self, ctx: &CommandContext, allow_duplicate_refs: bool) -> Result<()> {
        if self.initialized().is_ok() {
            return Ok(());
        }
        let commit = ctx.repository().find_commit(self.head())?;

        let mut reference = Branch {
            target: commit.into(),
            name: if let Some(refname) = self.upstream.as_ref() {
                refname.branch().to_string()
            } else {
                let (author, _committer) = ctx.repository().signatures()?;
                generate_branch_name(author)?
            },
            description: None,
            forge_id: Default::default(),
            archived: Default::default(),
        };
        let state = branch_state(ctx);

        let is_duplicate = |reference: &Branch| -> Result<bool> {
            Ok(if allow_duplicate_refs {
                patch_reference_exists(&state, &reference.name)?
            } else {
                patch_reference_exists(&state, &reference.name)?
                    || local_reference_exists(ctx, &reference.name)?
                    || remote_reference_exists(ctx, &state, reference)?
            })
        };

        while is_duplicate(&reference)? {
            // keep incrementing the suffix until the name is unique
            let split = reference.name.split('-');
            let left = split.clone().take(split.clone().count() - 1).join("-");
            reference.name = split
                .last()
                .and_then(|last| last.parse::<u32>().ok())
                .map(|last| format!("{}-{}", left, last + 1)) //take everything except last, and append last + 1
                .unwrap_or_else(|| format!("{}-1", reference.name));
        }
        validate_name(&reference, &state)?;
        self.heads = vec![reference];
        state.set_branch(self.clone())
    }

    /// Adds a new "Branch" to the Stack.
    /// This is in fact just creating a new  GitButler patch reference (head) and associates it with the stack.
    /// The name cannot be the same as existing git references or existing patch references.
    /// The target must reference a commit (or change) that is part of the stack.
    /// The branch name must be a valid reference name (i.e. can not contain spaces, special characters etc.)
    ///
    /// When creating heads, it is possible to have multiple heads that point to the same patch/commit.
    /// If this is the case, the order can be disambiguated by specifying the `preceding_head`.
    /// If there are multiple heads pointing to the same patch and `preceding_head` is not specified,
    /// that means the new head will be first in order for that patch.
    /// The argument `preceding_head` is only used if there are multiple heads that point to the same patch, otherwise it is ignored.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    pub fn add_series(
        &mut self,
        ctx: &CommandContext,
        new_head: Branch,
        preceding_head_name: Option<String>,
    ) -> Result<()> {
        self.initialized()?;
        let preceding_head = if let Some(preceding_head_name) = preceding_head_name {
            let (_, preceding_head) = get_head(&self.heads, &preceding_head_name)
                .context("The specified preceding_head could not be found")?;
            Some(preceding_head)
        } else {
            None
        };
        let state = branch_state(ctx);
        let patches = self.stack_patches(ctx, true)?;
        validate_name(&new_head, &state)?;
        validate_target(&new_head, ctx.repository(), self.head(), &state)?;
        let updated_heads = add_head(self.heads.clone(), new_head, preceding_head, patches)?;
        self.heads = updated_heads;
        state.set_branch(self.clone())
    }

    /// A convenience method just like `add_series`, but adds a new branch on top of the stack.
    pub fn add_series_top_of_stack(
        &mut self,
        ctx: &CommandContext,
        name: String,
        description: Option<String>,
    ) -> Result<()> {
        self.initialized()?;
        let current_top_head = self.heads.last().ok_or(anyhow!(
            "Stack is in an invalid state - heads list is empty"
        ))?;
        let new_head = Branch {
            target: current_top_head.target.clone(),
            name,
            description,
            forge_id: Default::default(),
            archived: Default::default(),
        };
        self.add_series(ctx, new_head, Some(current_top_head.name.clone()))
    }

    /// Removes a branch from the Stack.
    /// The very last branch (reference) cannot be removed (A Stack must always contain at least one reference)
    /// If there were commits/changes that were *only* referenced by the removed branch,
    /// those commits are moved to the branch underneath it (or more accurately, the preceding it)
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    pub fn remove_series(&mut self, ctx: &CommandContext, branch_name: String) -> Result<()> {
        self.initialized()?;
        (self.heads, _) = remove_head(self.heads.clone(), branch_name)?;
        let state = branch_state(ctx);
        state.set_branch(self.clone())
    }

    /// Updates an existing branch in the stack.
    /// The same invariants as `add_branch` apply.
    /// If the branch name is updated, the forge_id will be reset to None.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    pub fn update_series(
        &mut self,
        ctx: &CommandContext,
        branch_name: String,
        update: &PatchReferenceUpdate,
    ) -> Result<()> {
        self.initialized()?;
        if update == &PatchReferenceUpdate::default() {
            return Ok(()); // noop
        }

        let state = branch_state(ctx);
        let patches = self.stack_patches(ctx, true)?;
        let mut updated_heads = self.heads.clone();

        // Handle target updates
        if let Some(target_update) = &update.target_update {
            let mut new_head = updated_heads
                .clone()
                .into_iter()
                .find(|h| h.name == branch_name)
                .ok_or_else(|| anyhow!("Series with name {} not found", branch_name))?;
            new_head.target = target_update.target.clone();
            validate_target(&new_head, ctx.repository(), self.head(), &state)?;
            let preceding_head = if let Some(preceding_head_name) = update
                .target_update
                .clone()
                .and_then(|update| update.preceding_head_name)
            {
                let (_, preceding_head) = get_head(&self.heads, &preceding_head_name)
                    .context("The specified preceding_head could not be found")?;
                Some(preceding_head)
            } else {
                None
            };

            // drop the old head and add the new one
            let (idx, _) = get_head(&updated_heads, &branch_name)?;
            updated_heads.remove(idx);
            if patches.last() != updated_heads.last().map(|h| &h.target) {
                bail!("This update would cause orphaned patches, which is disallowed");
            }
            updated_heads = add_head(
                updated_heads,
                new_head.clone(),
                preceding_head,
                patches.clone(),
            )?;
        }

        // Handle name updates
        if let Some(name) = update.name.clone() {
            let head = updated_heads
                .iter_mut()
                .find(|h: &&mut Branch| h.name == branch_name);
            if let Some(head) = head {
                head.name = name;
                validate_name(head, &state)?;
                head.forge_id = None; // reset forge_id
            }
        }

        // Handle description updates
        if let Some(description) = update.description.clone() {
            let head = updated_heads.iter_mut().find(|h| h.name == branch_name);
            if let Some(head) = head {
                head.description = description;
            }
        }
        self.heads = updated_heads;
        state.set_branch(self.clone())
    }

    /// Updates the most recent series of the stack to point to a new patch (commit or change ID).
    /// This will set the
    /// - `head` of the stack to the new commit
    /// - the target of the most recent series to the new commit
    /// - the timestamp of the stack to the current time
    /// - the tree of the stack to the new tree (if provided)
    pub fn set_stack_head(
        &mut self,
        ctx: &CommandContext,
        commit_id: git2::Oid,
        tree: Option<git2::Oid>,
    ) -> Result<()> {
        self.initialized()?;
        self.updated_timestamp_ms = gitbutler_time::time::now_ms();
        #[allow(deprecated)] // this is the only place where this is allowed
        self.set_head(commit_id);
        if let Some(tree) = tree {
            self.tree = tree;
        }
        let commit = ctx.repository().find_commit(commit_id)?;
        // let patch: CommitOrChangeId = commit.into();

        let state = branch_state(ctx);
        let stack_head = self.head();
        let head = self
            .heads
            .last_mut()
            .ok_or_else(|| anyhow!("Invalid state: no heads found"))?;
        head.target = commit.into();
        validate_target(head, ctx.repository(), stack_head, &state)?;
        state.set_branch(self.clone())
    }

    /// Removes any heads that are refering to commits that are no longer between the stack head and the merge base
    pub fn archive_integrated_heads(&mut self, ctx: &CommandContext) -> Result<()> {
        self.initialized()?;
        self.updated_timestamp_ms = gitbutler_time::time::now_ms();
        let state = branch_state(ctx);
        let commit_ids = self.stack_patches(ctx, true)?;
        for head in self.heads.iter_mut() {
            if !commit_ids.contains(&head.target) {
                head.archived = true;
            }
        }
        state.set_branch(self.clone())
    }

    /// Prepares push details according to the series to be pushed (picking out the correct sha and remote refname)
    /// This operation will error out if the target has no push remote configured.
    pub fn push_details(&self, ctx: &CommandContext, branch_name: String) -> Result<PushDetails> {
        self.initialized()?;
        let (_, reference) = get_head(&self.heads, &branch_name)?;
        let commit = commit_by_oid_or_change_id(
            &reference.target,
            ctx.repository(),
            self.head(),
            self.merge_base(ctx)?.id(),
        )?
        .head;
        let remote_name = branch_state(ctx).get_default_target()?.push_remote_name();
        let upstream_refname =
            RemoteRefname::from_str(&reference.remote_reference(remote_name.as_str())?)
                .context("Failed to parse the remote reference for branch")?;
        Ok(PushDetails {
            head: commit.id(),
            remote_refname: upstream_refname,
        })
    }

    /// Returns a list of all branches/series in the stack.
    /// This operation will compute the current list of local and remote commits that belong to each series.
    /// The first entry is the newest in the Stack (i.e. the top of the stack).
    pub fn list_series<'a>(&self, ctx: &'a CommandContext) -> Result<Vec<Series<'a>>> {
        self.initialized()?;
        let state = branch_state(ctx);
        let mut all_series: Vec<Series> = vec![];
        let repo = ctx.repository();
        let default_target = state.get_default_target()?;
        let merge_base = self.merge_base(ctx)?.id();
        let mut previous_head = merge_base;
        for head in self.heads.clone() {
            let head_commit =
                commit_by_oid_or_change_id(&head.target, repo, self.head(), merge_base);
            if head.archived || head_commit.is_err() {
                all_series.push(Series {
                    head: head.clone(),
                    local_commits: vec![],
                    remote_commits: vec![],
                    upstream_only_commits: vec![],
                    remote_commit_ids_by_change_id: HashMap::new(),
                    archived: head.archived,
                });
                continue;
            }
            let head_commit = head_commit?.head.id();

            let mut local_patches = vec![];
            for commit in repo
                .log(head_commit, LogUntil::Commit(previous_head), false)?
                .into_iter()
                .rev()
            {
                local_patches.push(commit);
            }

            let mut remote_patches: Vec<Commit<'_>> = vec![];
            let mut remote_commit_ids_by_change_id: HashMap<String, git2::Oid> = HashMap::new();
            let remote_name = default_target.push_remote_name();
            if head.pushed(&remote_name, ctx).unwrap_or_default() {
                let head_commit = repo
                    .find_reference(&head.remote_reference(&remote_name)?)?
                    .peel_to_commit()?;
                let target_commit = repo
                    .find_reference(default_target.branch.to_string().as_str())?
                    .peel_to_commit()?;
                let merge_base = repo.merge_base(head_commit.id(), target_commit.id())?;
                repo.log(head_commit.id(), LogUntil::Commit(merge_base), false)?
                    .into_iter()
                    .rev()
                    .for_each(|c| {
                        if let Some(change_id) = c.change_id() {
                            remote_commit_ids_by_change_id.insert(change_id.to_string(), c.id());
                        }
                        remote_patches.push(c);
                    });
            }

            // compute the commits that are only in the upstream
            let local_patches_including_merge = repo
                .log(head_commit, LogUntil::Commit(merge_base), true)?
                .into_iter()
                .rev() // oldest commit first
                .collect_vec();
            let mut upstream_only = vec![];
            for patch in remote_patches.iter() {
                if !local_patches_including_merge.contains_by_commit_or_change_id(patch) {
                    upstream_only.push(patch.clone());
                }
            }

            all_series.push(Series {
                head: head.clone(),
                local_commits: local_patches,
                remote_commits: remote_patches,
                upstream_only_commits: upstream_only,
                remote_commit_ids_by_change_id,
                archived: head.archived,
            });
            previous_head = head_commit;
        }
        Ok(all_series)
    }

    /// Updates all heads in the stack that point to the `from` commit to point to the `to` commit.
    /// If there is nothing pointing to the `from` commit, this operation is a no-op.
    /// If the `from` and `to` commits have the same change_id, this operation is also a no-op.
    ///
    /// In the case that the `from` commit is the head of the stack, this operation delegates to `set_stack_head`.
    ///
    /// Every time a commit/patch is moved / removed / updated, this method needs to be invoked to maintain the integrity of the stack.
    /// Typically, in this case the `to` Commit would be `from`'s parent.
    ///
    /// The `to` commit must be between the Stack head, and it's merge base otherwise this operation will error out.
    pub fn replace_head(
        &mut self,
        ctx: &CommandContext,
        from: &Commit<'_>,
        to: &Commit<'_>,
    ) -> Result<()> {
        self.initialized()?;
        // find all heads matching the 'from' target (there can be multiple heads pointing to the same commit)
        let matching_heads = self
            .heads
            .iter()
            .filter(|h| match from.change_id() {
                Some(change_id) => h.target == CommitOrChangeId::ChangeId(change_id.clone()),
                None => h.target == CommitOrChangeId::CommitId(from.id().to_string()),
            })
            .cloned()
            .collect_vec();

        if from.change_id() == to.change_id() {
            // there is nothing to do
            return Ok(());
        }

        let state = branch_state(ctx);
        let mut updated_heads: Vec<Branch> = vec![];

        for head in matching_heads {
            if self.heads.last().cloned() == Some(head.clone()) {
                // the head is the stack head - update it accordingly
                self.set_stack_head(ctx, to.id(), None)?;
            } else {
                // new head target from the 'to' commit
                let mut new_head = head.clone();
                new_head.target = to.clone().into();
                // validate the updated head
                validate_target(&new_head, ctx.repository(), self.head(), &state)?;
                // add it to the list of updated heads
                updated_heads.push(new_head);
            }
        }

        if !updated_heads.is_empty() {
            for updated_head in updated_heads {
                if let Some(head) = self.heads.iter_mut().find(|h| h.name == updated_head.name) {
                    // find set the corresponding head in the mutable self
                    *head = updated_head;
                }
            }
            self.updated_timestamp_ms = gitbutler_time::time::now_ms();
            // update the persistent state
            state.set_branch(self.clone())?;
        }
        Ok(())
    }

    /// Sets the stack heads to the provided commits.
    /// This is useful multiple heads are updated and the intermediate states are not valid while the final state is.
    pub fn set_all_heads(
        &mut self,
        ctx: &CommandContext,
        new_heads: HashMap<String, Commit<'_>>,
    ) -> Result<()> {
        let state = branch_state(ctx);

        // same heads, just differente commits
        if self.heads.iter().map(|h| &h.name).collect::<HashSet<_>>()
            != new_heads.keys().collect::<HashSet<_>>()
        {
            return Err(anyhow!("The new head names do not match the current heads"));
        }
        let stack_head = self.head();
        for head in self.heads.iter_mut() {
            validate_target(head, ctx.repository(), stack_head, &state)?;
            if let Some(commit) = new_heads.get(&head.name).cloned() {
                head.target = commit.clone().into();
            }
        }
        state.set_branch(self.clone())?;
        Ok(())
    }

    /// Sets the forge identifier for a given series/branch.
    /// Existing value is overwritten - passing `None` sets the forge identifier to `None`.
    ///
    /// # Errors
    /// If the series does not exist, this method will return an error.
    /// If the stack has not been initialized, this method will return an error.
    pub fn set_forge_id(
        &mut self,
        ctx: &CommandContext,
        series_name: &str,
        new_forge_id: Option<ForgeIdentifier>,
    ) -> Result<()> {
        self.initialized()?;
        match self.heads.iter_mut().find(|r| r.name == series_name) {
            Some(head) => {
                head.forge_id = new_forge_id;
                branch_state(ctx).set_branch(self.clone())
            }
            None => bail!(
                "Series {} does not exist on stack {}",
                series_name,
                self.name
            ),
        }
    }

    pub fn set_legacy_compatible_stack_reference(&mut self, ctx: &CommandContext) -> Result<()> {
        // self.upstream is only set if this is a branch that was created & manipulated by the legacy flow
        let legacy_refname = match self.upstream.clone().map(|r| r.branch().to_owned()) {
            Some(legacy_refname) => legacy_refname,
            None => return Ok(()), // noop
        };
        // update the reference only if there is exactly one series in the stack
        if self.heads.len() != 1 {
            return Ok(()); // noop
        }
        let head = match self.heads.first() {
            Some(head) => head,
            None => return Ok(()), // noop
        };
        if legacy_refname == head.name {
            return Ok(()); // noop
        }
        let default_target = branch_state(ctx).get_default_target()?;
        let update = PatchReferenceUpdate {
            name: Some(legacy_refname),
            ..Default::default()
        };
        // modify the stack reference only if it has not been pushed yet
        if !head
            .pushed(&default_target.push_remote_name(), ctx)
            .unwrap_or_default()
        {
            // set the stack reference to the legacy refname
            self.update_series(ctx, head.name.clone(), &update)?;
        }
        Ok(())
    }

    pub fn heads(&self) -> Vec<String> {
        self.heads.iter().map(|h| h.name.clone()).collect()
    }

    /// Returns the list of patches between the stack head and the merge base.
    /// The most recent patch is at the top of the 'stack' (i.e. the last element in the vector)
    fn stack_patches(
        &self,
        ctx: &CommandContext,
        include_merge_base: bool,
    ) -> Result<Vec<CommitOrChangeId>> {
        let commits = if include_merge_base {
            self.commits_with_merge_base(ctx)?
        } else {
            self.commits(ctx)?
        };
        let patches: Vec<CommitOrChangeId> = commits.into_iter().rev().map(|c| c.into()).collect();
        Ok(patches)
    }
}

/// Request to update a PatchReference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct PatchReferenceUpdate {
    pub target_update: Option<TargetUpdate>,
    pub name: Option<String>,
    /// If present, this sets the value of the description field.
    /// It is possible to set this to Some(None) which will remove an existing description.
    pub description: Option<Option<String>>,
}

/// Request to update the target of a PatchReference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TargetUpdate {
    /// The new patch (commit or change ID) that the reference should point to.
    pub target: CommitOrChangeId,
    /// If there are multiple heads that point to the same patch, the order can be disambiguated by specifying the `preceding_head`.
    /// Leaving this field empty will make the new head first in relation to other references pointing to this commit.
    pub preceding_head_name: Option<String>,
}

/// Push details to be supplied to `RepoActionsExt`'s `push` method.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PushDetails {
    /// The commit that is being pushed.
    pub head: git2::Oid,
    /// A remote refname to push to.
    pub remote_refname: RemoteRefname,
}

impl TryFrom<&Stack> for VirtualRefname {
    type Error = anyhow::Error;

    fn try_from(value: &Stack) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            branch: normalize_branch_name(&value.name)?,
        })
    }
}

/// Validates that the commit in the reference target
///  - exists
///  - is between the stack (formerly vbranch) head (inclusive) and base (inclusive)
///
/// If the patch reference is a commit ID, it must be the case that the commit has no change ID associated with it.
/// In other words, change IDs are enforced to be preferred over commit IDs when available.
fn validate_target(
    reference: &Branch,
    repo: &git2::Repository,
    stack_head: git2::Oid,
    state: &VirtualBranchesHandle,
) -> Result<()> {
    let default_target = state.get_default_target()?;
    let merge_base = repo.merge_base(stack_head, default_target.sha)?;
    let commit = commit_by_oid_or_change_id(&reference.target, repo, stack_head, merge_base)?.head;

    let merge_base = repo.merge_base(stack_head, default_target.sha)?;
    let mut stack_commits = repo
        .log(stack_head, LogUntil::Commit(merge_base), false)?
        .iter()
        .map(|c| c.id())
        .collect_vec();
    stack_commits.insert(0, merge_base);
    if !stack_commits.contains(&commit.id()) {
        return Err(anyhow!(
            "The commit {} is not between the stack head and the stack base",
            commit.id()
        ));
    }
    // Enforce that change ids are used when available
    if let CommitOrChangeId::CommitId(_) = reference.target {
        if commit.change_id().is_some() {
            return Err(anyhow!(
                "The commit {} has a change id associated with it. Use the change id instead",
                commit.id()
            ));
        }
    }
    Ok(())
}

/// Validates the name of the stack head.
/// The name must be:
///  - unique within all stacks
///  - not the same as any existing local git reference (it is permitted for the name to match an existing remote reference)
///  - not including the `refs/heads/` prefix
fn validate_name(reference: &Branch, state: &VirtualBranchesHandle) -> Result<()> {
    if reference.name.starts_with("refs/heads") {
        return Err(anyhow!("Stack head name cannot start with 'refs/heads'"));
    }
    // assert that the name is a valid branch name
    name_partial(reference.name.as_str().into()).context("Invalid branch name")?;
    // assert that there are no existing patch references with this name
    if patch_reference_exists(state, &reference.name)? {
        return Err(anyhow!(
            "A patch reference with the name {} exists",
            &reference.name
        ));
    }

    Ok(())
}

/// Given a branch id and a change id, returns the commit associated with the change id.
// TODO: We need a more efficient way of getting a commit by change id.
// NB: There can be multiple commits with the same change id on the same branch id.
// This is an error condition but we must handle it.
// If there are multiple commits, they are ordered newest to oldest.
fn commit_by_branch_id_and_change_id<'a>(
    repo: &'a git2::Repository,
    stack_head: git2::Oid, // branch.head
    merge_base: git2::Oid,
    change_id: &str,
) -> Result<CommitsForId<'a>> {
    let commits = if stack_head == merge_base {
        vec![repo.find_commit(stack_head)?]
    } else {
        repo.log(stack_head, LogUntil::Commit(merge_base), false)?
    };
    let commits = commits
        .into_iter()
        .filter(|c| c.change_id().as_deref() == Some(change_id))
        .collect_vec();
    if let Some(head) = commits.first() {
        let commits_for_id = CommitsForId {
            head: head.clone(),
            tail: commits.iter().skip(1).cloned().collect_vec(),
        };
        Ok(commits_for_id)
    } else {
        Err(anyhow!("No commit with change id {} found", change_id))
    }
}

fn branch_state(ctx: &CommandContext) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(ctx.project().gb_dir())
}

// NB: There can be multiple commits with the same change id on the same branch id.
// This is an error condition but we must handle it.
// If there are multiple commits, they are ordered newest to oldest.
pub fn commit_by_oid_or_change_id<'a>(
    reference_target: &'a CommitOrChangeId,
    repo: &'a git2::Repository,
    stack_head: git2::Oid,
    merge_base: git2::Oid,
) -> Result<CommitsForId<'a>> {
    Ok(match reference_target {
        CommitOrChangeId::CommitId(commit_id) => CommitsForId {
            head: repo.find_commit(commit_id.parse()?)?,
            tail: vec![],
        },
        CommitOrChangeId::ChangeId(change_id) => {
            commit_by_branch_id_and_change_id(repo, stack_head, merge_base, change_id)?
        }
    })
}

/// Returns the commits associated with a id.
/// In most cases this is exactly one commit. Hoever there is an error state where it is possible to have
/// multiple commits with the same change id on the same stack.
#[derive(Debug, Clone)]
pub struct CommitsForId<'a> {
    /// The newest commit with the change id.
    pub head: Commit<'a>,
    /// There may be multiple commits with the same change id - if so they are ordered newest to oldest.
    /// The tails does not include the head.
    pub tail: Vec<Commit<'a>>,
}

fn patch_reference_exists(state: &VirtualBranchesHandle, name: &str) -> Result<bool> {
    Ok(state
        .list_branches_in_workspace()?
        .iter()
        .flat_map(|b| b.heads.iter())
        .any(|r| r.name == name))
}

fn generate_branch_name(author: git2::Signature) -> Result<String> {
    let mut initials = decompose(author.name().unwrap_or_default().into())
        .chars()
        .filter(|c| c.is_ascii_alphabetic() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .map(|word| word.chars().next().unwrap_or_default())
        .collect::<String>()
        .to_lowercase();
    if !initials.is_empty() {
        initials.push('-');
    }
    let branch_name = format!("{}{}-1", initials, "branch");
    normalize_branch_name(&branch_name)
}

fn local_reference_exists(ctx: &CommandContext, name: &str) -> Result<bool> {
    let gix_repo = ctx.gix_repository()?;
    Ok(gix_repo.find_reference(name_partial(name.into())?).is_ok())
}

fn remote_reference_exists(
    ctx: &CommandContext,
    state: &VirtualBranchesHandle,
    reference: &Branch,
) -> Result<bool> {
    Ok(reference
        .remote_reference(state.get_default_target()?.push_remote_name().as_str())
        .and_then(|reference| local_reference_exists(ctx, &reference))
        .ok()
        .unwrap_or(false))
}

#[cfg(test)]
mod test {
    use super::*;
    use git2::{Signature, Time};

    #[test]
    fn gen_name() -> Result<()> {
        let author = Signature::new("Foo Bar", "fb@example.com", &Time::new(0, 0))?;
        assert_eq!(generate_branch_name(author)?, "fb-branch-1");
        Ok(())
    }
    #[test]
    fn gen_name_with_some_umlauts_and_accents() -> Result<()> {
        // handles accents
        let author = Signature::new("Äx Öx Åx Üx Éx Áx", "fb@example.com", &Time::new(0, 0))?;
        assert_eq!(generate_branch_name(author)?, "aoauea-branch-1");
        // bails on norwegian characters
        let author = Signature::new("Æx Øx", "fb@example.com", &Time::new(0, 0))?;
        assert_eq!(generate_branch_name(author)?, "xx-branch-1");
        Ok(())
    }

    #[test]
    fn gen_name_emojis() -> Result<()> {
        // only emoji gets ignored
        let author = Signature::new("🍑", "fb@example.com", &Time::new(0, 0))?;
        assert_eq!(generate_branch_name(author)?, "branch-1");
        // if there is a latin character, it gets included
        let author = Signature::new("🍑x", "fb@example.com", &Time::new(0, 0))?;
        assert_eq!(generate_branch_name(author)?, "x-branch-1");

        let author = Signature::new("🍑 Foo", "fb@example.com", &Time::new(0, 0))?;
        assert_eq!(generate_branch_name(author)?, "f-branch-1");
        Ok(())
    }

    #[test]
    fn gen_name_chinese_character() -> Result<()> {
        // igrnore all
        let author = Signature::new("吉特·巴特勒", "fb@example.com", &Time::new(0, 0))?;
        assert_eq!(generate_branch_name(author)?, "branch-1");
        Ok(())
    }
}

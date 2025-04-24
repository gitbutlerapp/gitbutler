use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use but_core::Reference;
use but_rebase::ReferenceSpec;
use git2::Commit;
use gitbutler_command_context::CommandContext;
use gitbutler_id::id::Id;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_oxidize::OidExt;
use gitbutler_oxidize::RepoExt;
use gitbutler_reference::{normalize_branch_name, Refname, RemoteRefname, VirtualRefname};
use gitbutler_repo::logging::LogUntil;
use gitbutler_repo::logging::RepositoryExt as _;
use gitbutler_repo::RepositoryExt;
use gix::validate::reference::name_partial;
use gix_utils::str::decompose;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::heads::add_head;
use crate::heads::get_head;
use crate::heads::remove_head;
use crate::stack_branch::remote_reference;
use crate::stack_branch::CommitOrChangeId;
use crate::StackBranch;
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
    /// It will be normalized except to be a valid ref-name if named `refs/gitbutler/<normalize(name)>`.
    pub name: String,
    pub notes: String,
    /// If set, this means this virtual branch was originally created from `Some(branch)`.
    /// It can be *any* branch.
    pub source_refname: Option<Refname>,
    /// Upstream tracking branch reference, added when creating a stack from a branch.
    /// Used e.g. when listing commits from a fork.
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
    tree: git2::Oid,
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
    pub heads: Vec<StackBranch>,
    #[serde(default = "default_false")]
    pub post_commits: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
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
            post_commits: false,
        }
    }

    pub fn new_with_just_heads(
        heads: Vec<StackBranch>,
        created_ms: u128,
        order: usize,
        in_workspace: bool,
    ) -> Self {
        Stack {
            id: StackId::default(),
            created_timestamp_ms: created_ms,
            updated_timestamp_ms: created_ms,
            order,
            allow_rebasing: true, //  default in V2
            in_workspace,
            heads,

            // Don't keep redundant information
            tree: git2::Oid::zero(),
            head: git2::Oid::zero(),
            source_refname: None,
            upstream: None,
            upstream_head: None,

            // Unused - everything is defined by the top-most branch name.
            name: "".to_string(),
            notes: "".to_string(),

            // Related to ownership, obsolete.
            selected_for_changes: None,
            // unclear, obsolete
            not_in_workspace_wip_change_id: None,
            // unclear
            post_commits: false,
            ownership: Default::default(),
        }
    }

    pub fn refname(&self) -> anyhow::Result<VirtualRefname> {
        self.try_into()
    }

    // TODO: derive this from the last head
    pub fn head(&self, repo: &gix::Repository) -> Result<gix::ObjectId> {
        self.heads
            .last()
            .map(|head| head.head_oid(repo))
            .ok_or_else(|| anyhow!("Stack is uninitialized"))?
    }

    // This should not be needed in v3
    pub fn set_tree(&mut self, tree: git2::Oid) {
        self.tree = tree;
    }

    pub fn tree(&self, ctx: &CommandContext) -> Result<git2::Oid> {
        if ctx.app_settings().feature_flags.v3 {
            ctx.gix_repo()?
                .find_commit(self.head(&ctx.gix_repo()?)?)?
                .tree()
                .map(|tree| tree.id.to_git2())
                .map_err(Into::into)
        } else {
            Ok(self.tree)
        }
    }

    fn set_head(&mut self, head: git2::Oid) {
        self.head = head;
    }

    /// This is the name of the top-most branch, provided by the API for convinience
    pub fn derived_name(&self) -> Result<String> {
        self.heads
            .last()
            .map(|head| head.name.clone())
            .ok_or_else(|| anyhow!("Stack is uninitialized"))
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
    ) -> Result<Self> {
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
        branch.initialize(ctx, allow_duplicate_refs)?;
        Ok(branch)
    }

    /// Returns the commits between the stack head (including) and the merge base (not including) for the stack.
    /// The commits are ordered from most recent to oldest.
    ///
    /// E.g. `[ 3, 2, 1 ]` where `3` is the branch head, and `1` is the oldest commit with the merge base as it's parent
    ///
    /// # Errors
    /// - If a merge base cannot be found
    /// - If logging between the head and merge base fails
    pub fn commits(&self, ctx: &CommandContext) -> Result<Vec<git2::Oid>> {
        let repo = ctx.repo();
        let stack_commits = repo.l(
            self.head(&repo.to_gix()?)?.to_git2(),
            LogUntil::Commit(self.merge_base(ctx)?.to_git2()),
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
    fn commits_with_merge_base(&self, ctx: &CommandContext) -> Result<Vec<git2::Oid>> {
        let mut commits = self.commits(ctx)?;
        let base_commit = self.merge_base(ctx)?;
        commits.push(base_commit.to_git2());
        Ok(commits)
    }

    /// Returns the merge base of the stack head and the project's target branch.
    /// The merge base is the common ancestor of the stack head and the project's target branch.
    ///
    /// # Errors
    /// - If a target is not set for the project
    /// - If the head commit of the stack is not found
    pub fn merge_base(&self, ctx: &CommandContext) -> Result<gix::ObjectId> {
        let virtual_branch_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
        let target = virtual_branch_state.get_default_target()?;
        let gix_repo = ctx.gix_repo()?;
        let merge_base = gix_repo.merge_base(self.head(&gix_repo)?, target.sha.to_gix())?;
        Ok(merge_base.detach())
    }

    /// An initialized stack has at least one head (branch).
    ///
    /// # Errors
    /// - If the stack has not been initialized
    fn ensure_initialized(&self) -> Result<()> {
        if !self.is_initialized() {
            bail!("Stack has not been initialized")
        }

        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        !self.heads.is_empty()
    }

    /// Initializes a new stack.
    /// An initialized stack means that the heads will always have at least one entry.
    /// When initialized, first stack head will point to the "Branch" head.
    /// Errors out if the stack has already been initialized.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    pub fn initialize(&mut self, ctx: &CommandContext, allow_duplicate_refs: bool) -> Result<()> {
        // If the branch is already initialized, don't do anything
        if self.is_initialized() {
            return Ok(());
        }

        let empty_reference = self.make_new_empty_reference(ctx, allow_duplicate_refs)?;

        self.heads = vec![empty_reference];
        let state = branch_state(ctx);
        state.set_stack(self.clone())
    }

    fn make_new_empty_reference(
        &mut self,
        ctx: &CommandContext,
        allow_duplicate_refs: bool,
    ) -> Result<StackBranch> {
        let state = branch_state(ctx);
        let repo = ctx.gix_repo()?;
        // If the stack is created for the first time, this will be the default target sha
        let head = if self.heads.is_empty() {
            self.head
        } else {
            self.head(&repo)?.to_git2()
        };
        let commit = ctx.repo().find_commit(head)?;

        let name = if let Some(refname) = self.upstream.as_ref() {
            refname.branch().to_string()
        } else if ctx.app_settings().feature_flags.v3 {
            self.name.clone()
        } else {
            canned_branch_name(ctx.repo())?
        };

        let name = Stack::next_available_name(&repo, &state, name, allow_duplicate_refs)?;

        validate_name(&name, &state)?;
        let reference = StackBranch::new(commit, name, None, &repo)?;

        Ok(reference)
    }

    pub fn next_available_name(
        repo: &gix::Repository,
        state: &VirtualBranchesHandle,
        mut name: String,
        allow_duplicate_refs: bool,
    ) -> Result<String> {
        let is_duplicate = |name: &String| -> Result<bool> {
            Ok(if allow_duplicate_refs {
                patch_reference_exists(state, name)?
            } else {
                patch_reference_exists(state, name)?
                    || local_reference_exists(repo, name)?
                    || remote_reference_exists(repo, state, name)?
            })
        };
        while is_duplicate(&name)? {
            // keep incrementing the suffix until the name is unique
            // let name = reference.name();
            let mut split = name.split('-');
            let left = split.clone().take(split.clone().count() - 1).join("-");
            name = split
                .next_back()
                .and_then(|last| last.parse::<u32>().ok())
                .map(|last| format!("{}-{}", left, last + 1)) //take everything except last, and append last + 1
                .unwrap_or_else(|| format!("{}-1", name));
        }
        Ok(name)
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
        new_head: StackBranch,
        preceding_head_name: Option<String>,
    ) -> Result<()> {
        self.ensure_initialized()?;
        let preceding_head = if let Some(preceding_head_name) = preceding_head_name {
            let (_, preceding_head) = get_head(&self.heads, &preceding_head_name)
                .context("The specified preceding_head could not be found")?;
            Some(preceding_head)
        } else {
            None
        };
        let state = branch_state(ctx);
        let patches = self.stack_patches(ctx, true)?;
        validate_name(new_head.name(), &state)?;
        let gix_repo = ctx.gix_repo()?;
        validate_target(
            new_head.head_oid(&gix_repo)?.to_git2(),
            ctx.repo(),
            self.head(&gix_repo)?.to_git2(),
            &state,
        )?;
        let updated_heads = add_head(
            self.heads.clone(),
            new_head,
            preceding_head,
            patches,
            &gix_repo,
        )?;
        self.heads = updated_heads;
        state.set_stack(self.clone())
    }

    /// A convenience method just like `add_series`, but adds a new branch on top of the stack.
    pub fn add_series_top_of_stack(
        &mut self,
        ctx: &CommandContext,
        name: String,
        description: Option<String>,
    ) -> Result<()> {
        self.ensure_initialized()?;
        let current_top_head = self.heads.last().ok_or(anyhow!(
            "Stack is in an invalid state - heads list is empty"
        ))?;
        let repo = ctx.gix_repo()?;
        let new_head =
            StackBranch::new(current_top_head.head_oid(&repo)?, name, description, &repo)?;
        self.add_series(ctx, new_head, Some(current_top_head.name().clone()))
    }

    /// Removes a branch from the Stack.
    /// The very last branch (reference) cannot be removed (A Stack must always contain at least one reference)
    /// If there were commits/changes that were *only* referenced by the removed branch,
    /// those commits are moved to the branch underneath it (or more accurately, the preceding it)
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    pub fn remove_branch(&mut self, ctx: &CommandContext, branch_name: String) -> Result<()> {
        self.ensure_initialized()?;
        (self.heads, _) = remove_head(self.heads.clone(), branch_name, &ctx.gix_repo()?)?;
        let state = branch_state(ctx);
        state.set_stack(self.clone())
    }

    /// Updates an existing branch in the stack.
    /// The same invariants as `add_branch` apply.
    /// If the branch name is updated, the pr_number will be reset to None.
    ///
    /// This operation mutates the gitbutler::Branch.heads list and updates the state in `virtual_branches.toml`
    pub fn update_branch(
        &mut self,
        ctx: &CommandContext,
        branch_name: String,
        update: &PatchReferenceUpdate,
    ) -> Result<()> {
        self.ensure_initialized()?;
        if update == &PatchReferenceUpdate::default() {
            return Ok(()); // noop
        }

        let state = branch_state(ctx);
        let mut updated_heads = self.heads.clone();

        // Handle name updates
        if let Some(name) = update.name.clone() {
            let head = updated_heads
                .iter_mut()
                .find(|h: &&mut StackBranch| *h.name() == branch_name);
            if let Some(head) = head {
                validate_name(&name, &state)?;
                head.set_name(name, &ctx.gix_repo()?)?;
                head.pr_number = None; // reset pr_number
            }
        }

        // Handle description updates
        if let Some(description) = update.description.clone() {
            let head = updated_heads.iter_mut().find(|h| *h.name() == branch_name);
            if let Some(head) = head {
                head.description = description;
            }
        }
        self.heads = updated_heads;
        state.set_stack(self.clone())
    }

    /// This will go over the stack heads and will ensure that the heads are consistent with the respective git reference.
    /// If a head is not the same as the reference, it will be updated to match the git reference.
    /// This operation should not really be needed since references are always updated.
    /// However, this function exists to be called before an oplog snapshot of the virtual_branches.toml is taken because
    /// upon snapshot restore, git references will be updated to match the stack heads from the toml file
    /// TODO: is there a performace implication of this?
    pub fn sync_heads_with_references(
        &mut self,
        state: &VirtualBranchesHandle,
        gix_repo: &gix::Repository,
    ) -> Result<()> {
        if self
            .heads
            .iter_mut()
            .any(|head| head.sync_with_reference(gix_repo).unwrap_or(false))
        {
            state.set_stack(self.clone())?;
        }
        Ok(())
    }

    /// Updates the most recent series of the stack to point to a new patch (commit or change ID).
    /// This will set the
    /// - `head` of the stack to the new commit
    /// - the target of the most recent series to the new commit
    /// - the timestamp of the stack to the current time
    /// - the tree of the stack to the new tree (if provided)
    pub fn set_stack_head(
        &mut self,
        state: &VirtualBranchesHandle,
        gix_repo: &gix::Repository,
        commit_id: git2::Oid,
        tree: Option<git2::Oid>,
    ) -> Result<()> {
        self.set_stack_head_inner(Some(state), gix_repo, commit_id, tree)
    }

    pub fn set_stack_head_without_persisting(
        &mut self,
        gix_repo: &gix::Repository,
        commit_id: git2::Oid,
        tree: Option<git2::Oid>,
    ) -> Result<()> {
        self.set_stack_head_inner(None, gix_repo, commit_id, tree)
    }

    fn set_stack_head_inner(
        &mut self,
        state: Option<&VirtualBranchesHandle>,
        gix_repo: &gix::Repository,
        commit_id: git2::Oid,
        tree: Option<git2::Oid>,
    ) -> Result<()> {
        self.ensure_initialized()?;
        self.updated_timestamp_ms = gitbutler_time::time::now_ms();
        #[allow(deprecated)] // this is the only place where this is allowed
        self.set_head(commit_id);
        if let Some(tree) = tree {
            self.tree = tree;
        }

        let commit = gix_repo.find_commit(commit_id.to_gix())?;

        let head = self
            .heads
            .last_mut()
            .ok_or_else(|| anyhow!("Invalid state: no heads found"))?;

        head.set_head(commit.id, gix_repo)?;
        if let Some(state) = state {
            state.set_stack(self.clone())?;
        }
        Ok(())
    }

    /// Removes any heads that are refering to commits that are no longer between the stack head and the merge base
    pub fn archive_integrated_heads(
        &mut self,
        ctx: &CommandContext,
        repo: &gix::Repository,
        for_archival: &[Reference],
        delete_local_refs: bool,
    ) -> Result<(Vec<String>, Vec<String>)> {
        self.ensure_initialized()?;

        let mut newly_archived_branches = vec![];
        let mut review_ids_to_close = vec![];

        self.updated_timestamp_ms = gitbutler_time::time::now_ms();
        let state = branch_state(ctx);
        for head in self.heads.iter_mut() {
            let full_name = head.full_name()?;
            if for_archival.iter().any(|reference| match reference {
                Reference::Git(r) => r == &full_name,
                Reference::Virtual(r) => r == head.name(),
            }) {
                head.archived = true;
                newly_archived_branches.push(head.name().clone());
                if let Some(review_id) = head.review_id.clone() {
                    review_ids_to_close.push(review_id);
                }

                if delete_local_refs {
                    head.delete_reference(repo).ok(); // Fail silently because interrupting this is worse
                }
            }
        }

        if self.heads.iter().all(|branch| branch.archived) {
            // Remove previous PR information so it looks like we started a new stack
            for head in self.heads.iter_mut() {
                head.pr_number = None;
            }

            let new_head = self.make_new_empty_reference(ctx, false)?;
            self.heads.push(new_head);
        }

        state.set_stack(self.clone())?;

        Ok((newly_archived_branches, review_ids_to_close))
    }

    /// Prepares push details according to the series to be pushed (picking out the correct sha and remote refname)
    /// This operation will error out if the target has no push remote configured.
    pub fn push_details(&self, ctx: &CommandContext, branch_name: String) -> Result<PushDetails> {
        self.ensure_initialized()?;
        let (_, reference) = get_head(&self.heads, &branch_name)?;
        let oid = reference.head_oid(&ctx.gix_repo()?)?.to_git2();
        let commit = ctx.repo().find_commit(oid)?;
        let remote_name = branch_state(ctx).get_default_target()?.push_remote_name();
        let upstream_refname =
            RemoteRefname::from_str(&reference.remote_reference(remote_name.as_str()))?;
        Ok(PushDetails {
            head: commit.id(),
            remote_refname: upstream_refname,
        })
    }

    /// Returns the branch that precedes the given branch in the stack, if any.
    pub(crate) fn branch_predacessor(&self, branch: &StackBranch) -> Option<&StackBranch> {
        self.heads.iter().take_while(|head| *head != branch).last()
    }

    /// Returns a list of all branches/series in the stack.
    /// Ordered from oldest to newest (most recent)
    pub fn branches(&self) -> Vec<StackBranch> {
        self.heads.clone()
    }

    /// Sets the stack heads to the provided commits.
    /// This is useful multiple heads are updated and the intermediate states are not valid while the final state is.
    fn set_all_heads(
        &mut self,
        ctx: &CommandContext,
        new_heads: HashMap<String, Commit<'_>>,
    ) -> Result<()> {
        let state = branch_state(ctx);

        // same heads, just differente commits
        if self
            .heads
            .iter()
            .filter(|h| !h.archived)
            .map(|h| h.name())
            .collect::<HashSet<_>>()
            != new_heads.keys().collect::<HashSet<_>>()
        {
            return Err(anyhow!("The new head names do not match the current heads"));
        }
        let gix_repo = ctx.gix_repo()?;
        for head in &mut self.heads {
            if let Some(commit) = new_heads.get(head.name()) {
                head.set_head(commit.clone(), &gix_repo)?;
            }
        }
        state.set_stack(self.clone())?;
        Ok(())
    }

    /// Sets the stack heads according to the output from the rebase of a `but-rebase` rebase operation
    pub fn set_heads_from_rebase_output(
        &mut self,
        ctx: &CommandContext,
        references: Vec<ReferenceSpec>,
    ) -> anyhow::Result<()> {
        let mut new_heads: HashMap<String, Commit<'_>> = HashMap::new();
        for spec in &references {
            let commit = ctx.repo().find_commit(spec.commit_id.to_git2())?;
            new_heads.insert(spec.reference.to_string(), commit);
        }

        self.set_all_heads(ctx, new_heads)
    }

    /// Migrates all change IDs in stack heads to commit IDs.
    #[allow(deprecated)]
    pub fn migrate_change_ids(&mut self, ctx: &CommandContext) -> Result<()> {
        // If all of the heads are already commit IDs, there is nothing to do
        if self.heads.iter().all(|h| !h.uses_change_id()) {
            return Ok(());
        }

        let stack_head = self.head; // Use the field directly because here the stack heads have not been migrated yet
        let virtual_branch_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
        let target = virtual_branch_state.get_default_target()?;
        let merge_base = ctx.repo().merge_base(stack_head, target.sha)?;

        for head in self.heads.iter_mut() {
            head.migrate_change_id(ctx.repo(), stack_head, merge_base);
        }

        let state = branch_state(ctx);
        state.set_stack(self.clone())
    }

    /// Sets the forge identifier for a given series/branch.
    /// Existing value is overwritten - passing `None` sets the forge identifier to `None`.
    ///
    /// # Errors
    /// If the series does not exist, this method will return an error.
    /// If the stack has not been initialized, this method will return an error.
    pub fn set_pr_number(
        &mut self,
        ctx: &CommandContext,
        branch_name: &str,
        new_pr_number: Option<usize>,
    ) -> Result<()> {
        self.ensure_initialized()?;
        match self.heads.iter_mut().find(|r| r.name() == branch_name) {
            Some(head) => {
                head.pr_number = new_pr_number;
                branch_state(ctx).set_stack(self.clone())
            }
            None => bail!(
                "Series {} does not exist on stack {}",
                branch_name,
                self.name
            ),
        }
    }

    pub fn heads(&self, exclude_archived: bool) -> Vec<String> {
        if exclude_archived {
            self.heads.iter().map(|h| h.name().clone()).collect()
        } else {
            self.heads
                .iter()
                .filter(|h| !h.archived)
                .map(|h| h.name().clone())
                .collect()
        }
    }

    pub fn heads_by_commit(&self, commit: Commit<'_>, repo: &gix::Repository) -> Vec<String> {
        // let id: CommitOrChangeId = commit.into();
        self.heads
            .iter()
            .filter(|h| h.head_oid(repo).ok() == Some(commit.id().to_gix()))
            .map(|h| h.name().clone())
            .collect_vec()
    }

    /// Returns the list of patches between the stack head and the merge base.
    /// The most recent patch is at the top of the 'stack' (i.e. the last element in the vector)
    fn stack_patches(
        &self,
        ctx: &CommandContext,
        include_merge_base: bool,
    ) -> Result<Vec<CommitOrChangeId>> {
        let repo = ctx.repo();

        let commits = if include_merge_base {
            self.commits_with_merge_base(ctx)?
        } else {
            self.commits(ctx)?
        };
        let patches: Vec<CommitOrChangeId> = commits
            .into_iter()
            .rev()
            .filter_map(
                |oid| {
                    repo.find_commit(oid)
                        .ok()
                        .map(|c| CommitOrChangeId::CommitId(c.id().to_string()))
                }, // repository.lookup_change_id_or_oid(commit).ok()
            )
            .collect();
        Ok(patches)
    }
}

/// Request to update a PatchReference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct PatchReferenceUpdate {
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
    reference: git2::Oid,
    repo: &git2::Repository,
    stack_head: git2::Oid,
    state: &VirtualBranchesHandle,
) -> Result<()> {
    let default_target = state.get_default_target()?;

    let merge_base = repo.merge_base(stack_head, default_target.sha)?;
    let mut stack_commits = repo
        .log(stack_head, LogUntil::Commit(merge_base), false)?
        .iter()
        .map(|c| c.id())
        .collect_vec();
    stack_commits.insert(0, merge_base);
    if !stack_commits.contains(&reference) {
        return Err(anyhow!(
            "The commit {} is not between the stack head and the stack base",
            reference
        ));
    }
    Ok(())
}

/// Validates the name of the stack head.
/// The name must be:
///  - unique within all stacks
///  - not the same as any existing local git reference (it is permitted for the name to match an existing remote reference)
///  - not including the `refs/heads/` prefix
fn validate_name(name: &str, state: &VirtualBranchesHandle) -> Result<()> {
    if name.starts_with("refs/heads") {
        return Err(anyhow!("Stack head name cannot start with 'refs/heads'"));
    }
    // assert that the name is a valid branch name
    name_partial(name.into()).context("Invalid branch name")?;
    // assert that there are no existing patch references with this name
    if patch_reference_exists(state, name)? {
        return Err(anyhow!("A patch reference with the name {} exists", name));
    }

    Ok(())
}

fn branch_state(ctx: &CommandContext) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(ctx.project().gb_dir())
}

fn patch_reference_exists(state: &VirtualBranchesHandle, name: &str) -> Result<bool> {
    Ok(state
        .list_stacks_in_workspace()?
        .iter()
        .flat_map(|b| b.heads.iter())
        .any(|r| r.name() == name))
}

pub fn canned_branch_name(repo: &git2::Repository) -> Result<String> {
    if let Ok((author, _committer)) = repo.signatures() {
        generate_branch_name(author)
    } else {
        let author = git2::Signature::now("Firstname Lastname", "name@example.com")?;
        generate_branch_name(author)
    }
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

fn local_reference_exists(repo: &gix::Repository, name: &str) -> Result<bool> {
    Ok(repo.find_reference(name_partial(name.into())?).is_ok())
}

fn remote_reference_exists(
    repo: &gix::Repository,
    state: &VirtualBranchesHandle,
    name: &String,
) -> Result<bool> {
    let remote_ref = remote_reference(
        name,
        state.get_default_target()?.push_remote_name().as_str(),
    );
    local_reference_exists(repo, &remote_ref)
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

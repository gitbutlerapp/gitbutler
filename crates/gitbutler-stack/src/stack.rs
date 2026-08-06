use anyhow::{Context as _, Result, anyhow};
pub(crate) use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_error::bail_precondition;
use but_meta::virtual_branches_legacy_types;
use gitbutler_reference::{Refname, RemoteRefname, VirtualRefname, normalize_branch_name};
use gix::validate::reference::name_partial;
use itertools::Itertools;

use crate::{
    StackBranch,
    stack_branch::remote_reference,
    target::{default_target_base_oid, default_target_push_remote_name},
};

/// Legacy stack state persisted in virtual-branches metadata.
#[derive(Debug, PartialEq, Clone)]
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
    pub in_workspace: bool,
    /// Patch references ordered from oldest to newest.
    pub heads: Vec<StackBranch>,
}

impl From<virtual_branches_legacy_types::Stack> for Stack {
    fn from(
        virtual_branches_legacy_types::Stack {
            id,
            source_refname,
            upstream,
            order,
            in_workspace,
            heads,
            ..
        }: virtual_branches_legacy_types::Stack,
    ) -> Self {
        Stack {
            id,
            source_refname,
            upstream,
            order,
            in_workspace,
            heads: heads.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Stack> for virtual_branches_legacy_types::Stack {
    fn from(
        Stack {
            id,
            source_refname,
            upstream,
            order,
            in_workspace,
            heads,
        }: Stack,
    ) -> Self {
        virtual_branches_legacy_types::Stack {
            id,
            source_refname,
            upstream,
            order,
            in_workspace,
            heads: heads.into_iter().map(Into::into).collect(),
            // Dummy values for backwards compatibility
            #[expect(deprecated)]
            notes: String::new(),
            #[expect(deprecated)]
            ownership: virtual_branches_legacy_types::BranchOwnershipClaims::default(),
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

impl Stack {
    /// The name of the stack, defined as the name of the first head (branch) in the stack.
    /// The usage of this is discouraged
    pub fn name(&self) -> String {
        self.heads
            .first()
            .map(|head| head.name.clone())
            .unwrap_or_default()
    }

    pub fn refname(&self) -> anyhow::Result<VirtualRefname> {
        self.try_into()
    }

    pub fn head_oid(&self, ctx: &Context) -> Result<gix::ObjectId> {
        let repo = ctx.repo.get()?;
        if let Some(branch) = self.heads.last() {
            branch.head_oid(&repo)
        } else {
            default_target_base_oid(ctx)
        }
    }

    /// This is the name of the top-most branch, provided by the API for convenience
    pub fn derived_name(&self) -> Result<String> {
        self.heads
            .last()
            .map(|head| head.name.clone())
            .ok_or_else(|| anyhow!("Stack::derived_name: Stack is uninitialized"))
    }

    pub fn new_from_existing(
        ctx: &Context,
        name: String,
        source_refname: Option<Refname>,
        upstream: Option<RemoteRefname>,
        head: gix::ObjectId,
        order: usize,
    ) -> Result<Self> {
        let repo = ctx.repo.get()?;
        let push_remote_name = default_target_push_remote_name(ctx)?;
        let name = Stack::new_name(&repo, &push_remote_name, upstream.clone(), name, true)?;
        let stack_branch = Stack::create_stack_branch(&repo, head, name.clone())?;
        Ok(Self {
            id: StackId::generate(),
            source_refname,
            upstream,
            order,
            in_workspace: true,
            heads: vec![stack_branch],
        })
    }

    pub fn new_empty(
        ctx: &Context,
        name: String,
        head: gix::ObjectId,
        order: usize,
    ) -> Result<Self> {
        let repo = ctx.repo.get()?;
        let push_remote_name = default_target_push_remote_name(ctx)?;
        let name = Stack::new_name(&repo, &push_remote_name, None, name, false)?;
        let stack_branch = Stack::create_stack_branch(&repo, head, name.clone())?;
        Ok(Self {
            id: StackId::generate(),
            source_refname: None,
            upstream: None,
            order,
            in_workspace: true,
            heads: vec![stack_branch],
        })
    }

    /// Returns the merge base of the stack head and the project's target branch.
    /// The merge base is the common ancestor of the stack head and the project's target branch.
    ///
    /// # Errors
    /// - If a target is not set for the project
    /// - If the head commit of the stack is not found
    pub fn merge_base(&self, ctx: &Context) -> Result<gix::ObjectId> {
        let target_base_oid = default_target_base_oid(ctx)?;
        let repo = ctx.repo.get()?;
        let merge_base = repo.merge_base(self.head_oid(ctx)?, target_base_oid)?;
        Ok(merge_base.detach())
    }

    fn new_name(
        repo: &gix::Repository,
        push_remote_name: &str,
        upstream: Option<RemoteRefname>,
        fallback: String,
        allow_duplicate_refs: bool,
    ) -> Result<String> {
        let name = if let Some(refname) = upstream.as_ref() {
            refname.branch().to_string()
        } else {
            fallback
        };
        let name = Stack::next_available_name(repo, push_remote_name, name, allow_duplicate_refs)?;
        validate_name(&name, repo, allow_duplicate_refs)?;
        Ok(name)
    }

    /// Creates a new StackBranch pointing to the given head commit with the given name.
    /// This also creates a git reference in the repository.
    fn create_stack_branch(
        repo: &gix::Repository,
        head: gix::ObjectId,
        name: String,
    ) -> Result<StackBranch> {
        let commit = repo.find_commit(head)?;
        let reference = StackBranch::new(commit.id, name, repo)?;
        Ok(reference)
    }

    fn next_available_name(
        repo: &gix::Repository,
        push_remote_name: &str,
        mut name: String,
        allow_duplicate_refs: bool,
    ) -> Result<String> {
        let is_duplicate = |name: &String| -> Result<bool> {
            Ok(if allow_duplicate_refs {
                false
            } else {
                local_reference_exists(repo, name)?
                    || remote_reference_exists(repo, push_remote_name, name)?
            })
        };
        while is_duplicate(&name)? {
            // keep incrementing the suffix until the name is unique
            let mut split = name.split('-');
            let left = split.clone().take(split.clone().count() - 1).join("-");
            name = split
                .next_back()
                .and_then(|last| last.parse::<u32>().ok())
                .map(|last| format!("{}-{}", left, last + 1)) //take everything except last, and append last + 1
                .unwrap_or_else(|| format!("{name}-1"));
        }
        Ok(name)
    }
}

impl TryFrom<&Stack> for VirtualRefname {
    type Error = anyhow::Error;

    fn try_from(value: &Stack) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            branch: normalize_branch_name(&value.name())?,
        })
    }
}

/// Validates the name of the stack head.
/// The name must be:
///  - not the same as any existing local git reference unless it names the source ref
///  - not including the `refs/heads/` prefix
fn validate_name(name: &str, repo: &gix::Repository, allow_existing_ref: bool) -> Result<()> {
    if name.starts_with("refs/heads") {
        return Err(anyhow!("Stack head name cannot start with 'refs/heads'"));
    }
    // assert that the name is a valid branch name
    name_partial(name.into()).context("Invalid branch name")?;
    if !allow_existing_ref && local_reference_exists(repo, name)? {
        bail_precondition!("A patch reference with the name {name} exists");
    }

    Ok(())
}

fn local_reference_exists(repo: &gix::Repository, name: &str) -> Result<bool> {
    Ok(repo.find_reference(name_partial(name.into())?).is_ok())
}

fn remote_reference_exists(
    repo: &gix::Repository,
    push_remote_name: &str,
    name: &String,
) -> Result<bool> {
    let remote_ref = remote_reference(name, push_remote_name);
    local_reference_exists(repo, &remote_ref)
}

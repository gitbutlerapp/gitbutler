use anyhow::Context as _;
use bstr::{BStr, BString, ByteSlice};
use but_core::ref_metadata::StackId;
use gitbutler_stack::Stack;
use serde::Serialize;

/// The information about the branch inside a stack
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./workspace/legacy/index.ts"))]
pub struct StackHeadInfo {
    /// The name of the branch.
    #[serde(with = "but_serde::bstring_lossy")]
    #[cfg_attr(feature = "export-ts", ts(type = "string"))]
    pub name: BString,
    /// The tip of the branch.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(feature = "export-ts", ts(type = "string"))]
    pub tip: gix::ObjectId,
    /// The associated forge review with this branch, e.g. GitHub PRs or GitLab MRs
    pub review_id: Option<usize>,
    /// If `true`, then this head is checked directly so `HEAD` points to it, and this is only ever `true` for a single head.
    /// This is `false` if the worktree is checked out.
    pub is_checked_out: bool,
}

impl StackHeadInfo {
    /// Delete the reference for this head from the repository if it exists and matches the expected OID.
    pub fn delete_reference(&self, repo: &gix::Repository) -> anyhow::Result<()> {
        let ref_name = format!("refs/heads/{}", self.name.to_str()?.trim_matches('/'));
        let current_name: BString = ref_name.into();
        if let Some(reference) = repo.try_find_reference(&current_name)? {
            but_core::branch::SafeDelete::new(repo)?.delete_reference(&reference)?;
        }
        Ok(())
    }
}

/// Represents a lightweight version of a [`Stack`] for listing.
/// NOTE: this is a UI type mostly because it's still modeled after the legacy stack with StackId, something that doesn't exist anymore.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./workspace/legacy/index.ts"))]
pub struct StackEntry {
    /// The ID of the stack.
    #[cfg_attr(feature = "export-ts", ts(type = "string | null"))]
    pub id: Option<StackId>,
    /// The list of the branch information that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub heads: Vec<StackHeadInfo>,
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(feature = "export-ts", ts(type = "string"))]
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
    #[serde(with = "but_serde::object_id")]
    pub tip: gix::ObjectId,
    /// The zero-based index for sorting stacks.
    pub order: Option<usize>,
    /// If `true`, then any head in this stack is checked directly so `HEAD` points to it, and this is only ever `true` for a single stack.
    pub is_checked_out: bool,
}

impl From<StackEntryNoOpt> for crate::commit::Stack {
    fn from(value: StackEntryNoOpt) -> Self {
        crate::commit::Stack {
            tip: value.tip,
            name: value.name().map(ToOwned::to_owned),
        }
    }
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

impl StackEntryNoOpt {
    pub(crate) fn try_new(repo: &gix::Repository, stack: &Stack) -> anyhow::Result<Self> {
        let ctx = but_ctx::Context::try_from(repo.clone())?;
        Ok(StackEntryNoOpt {
            id: stack.id,
            heads: crate::legacy::stacks::stack_heads_info(stack, repo)?,
            tip: stack.head_oid(&ctx)?,
            order: Some(stack.order),
            is_checked_out: false,
        })
    }
}

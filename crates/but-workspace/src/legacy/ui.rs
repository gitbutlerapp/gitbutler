use bstr::BString;
use but_core::ref_metadata::StackId;
use serde::Serialize;

/// The information about the branch inside a stack
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StackHeadInfo {
    /// The name of the branch.
    #[serde(with = "but_serde::bstring_lossy")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_lossy")
    )]
    pub name: BString,
    /// The tip of the branch.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub tip: gix::ObjectId,
    /// The associated forge review with this branch, e.g. GitHub PRs or GitLab MRs
    pub review_id: Option<usize>,
    /// If `true`, then this head is checked directly so `HEAD` points to it, and this is only ever `true` for a single head.
    /// This is `false` if the worktree is checked out.
    pub is_checked_out: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackHeadInfo);

/// Represents a lightweight version of a legacy stack for listing.
/// NOTE: this is a UI type mostly because it's still modeled after the legacy stack with StackId, something that doesn't exist anymore.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StackEntry {
    /// The ID of the stack.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::stack_id_opt")
    )]
    pub id: Option<StackId>,
    /// The list of the branch information that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub heads: Vec<StackHeadInfo>,
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub tip: gix::ObjectId,
    /// The zero-based index for sorting stacks.
    pub order: Option<usize>,
    /// If `true`, then any head in this stack is checked directly so `HEAD` points to it, and this is only ever `true` for a single stack.
    pub is_checked_out: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackEntry);

/// **Temporary type to help transitioning to the optional version of stack-entry** and ultimately, to [`crate::RefInfo`].
/// WARNING: for use by parts in the code that can rely on having a non-optional `stack_id`. The goal is to have none of these.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StackEntryNoOpt {
    /// The ID of the stack.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::stack_id")
    )]
    pub id: StackId,
    /// The list of the branch information that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub heads: Vec<StackHeadInfo>,
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub tip: gix::ObjectId,
    /// The zero-based index for sorting stacks.
    pub order: Option<usize>,
    /// If `true`, then any head in this stack is checked directly so `HEAD` points to it, and this is only ever `true` for a single stack.
    pub is_checked_out: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackEntryNoOpt);

impl StackEntry {
    /// Get the associated reviews in the stack. Top to bottom.
    ///
    /// If there are no reviews associated with any of the branches, they'll be skipped.
    /// An empty vector would mean no reviews associated with any of the stacked branches or an empty stack.
    /// A vector of a different length than the amount of branches in the stack would indicate that only
    /// some branches have associated reviews.
    pub fn review_ids(&self) -> Vec<usize> {
        self.heads
            .iter()
            .filter_map(|head| head.review_id)
            .collect()
    }
}

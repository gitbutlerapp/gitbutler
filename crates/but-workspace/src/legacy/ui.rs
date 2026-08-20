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

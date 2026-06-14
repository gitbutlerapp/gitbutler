//! Debug-string helpers for shortening ref names, used by the display projection's `Debug` impls
//! (`StackSegment`, `Workspace`). These format names only — they don't touch any graph.

use bstr::ByteSlice;
use gix::reference::Category;

use crate::{RefInfo, Worktree};

/// Shorten `ref_name` so it's still clear whether it is a special ref (like a tag) or not.
pub(crate) fn ref_debug_string(
    ref_name: &gix::refs::FullNameRef,
    worktree: Option<&Worktree>,
) -> String {
    ref_debug_string_inner(ref_name, worktree, false)
}

pub(crate) fn ref_debug_string_inner(
    ref_name: &gix::refs::FullNameRef,
    worktree: Option<&Worktree>,
    show_owned_by_repo: bool,
) -> String {
    let (cat, sn) = ref_name.category_and_short_name().expect("valid refs");
    // Only shorten those that look good and are unambiguous enough.
    format!(
        "{}{ws}",
        if matches!(cat, Category::LocalBranch | Category::RemoteBranch) {
            sn
        } else {
            ref_name
                .as_bstr()
                .strip_prefix(b"refs/")
                .map(|n| n.as_bstr())
                .unwrap_or(ref_name.as_bstr())
        },
        ws = worktree
            .map(|wt| wt.debug_string_with_graph_context(ref_name, show_owned_by_repo))
            .unwrap_or_default()
    )
}

/// A one-line string showing the relationship between a ref, its `remote_ref_name`, and how they are
/// linked via `sibling_id` and `remote_tracking_branch_id`.
pub(crate) fn ref_and_remote_debug_string(
    ref_info: Option<&RefInfo>,
    remote_ref_name: Option<&gix::refs::FullName>,
    sibling_id: Option<usize>,
    remote_tracking_branch_id: Option<usize>,
) -> String {
    ref_and_remote_debug_string_inner(
        ref_info,
        remote_ref_name,
        sibling_id,
        remote_tracking_branch_id,
        false,
    )
}

pub(crate) fn ref_and_remote_debug_string_inner(
    ref_info: Option<&RefInfo>,
    remote_ref_name: Option<&gix::refs::FullName>,
    sibling_id: Option<usize>,
    remote_tracking_branch_id: Option<usize>,
    show_owned_by_repo: bool,
) -> String {
    format!(
        "{ref_name}{remote}",
        ref_name = ref_info
            .as_ref()
            .map(|ri| format!(
                "{}{maybe_id}",
                ref_debug_string_inner(
                    ri.ref_name.as_ref(),
                    ri.worktree.as_ref(),
                    show_owned_by_repo
                ),
                maybe_id = sibling_id
                    .filter(|_| remote_ref_name.is_none())
                    .map(|id| format!(" →:{}:", id))
                    .unwrap_or_default()
            ))
            .unwrap_or_else(|| format!(
                "anon:{maybe_id}",
                maybe_id = sibling_id
                    .map(|id| format!(" →:{}:", id))
                    .unwrap_or_default()
            )),
        remote = remote_ref_name
            .as_ref()
            .map(|remote_ref_name| format!(
                " <> {remote_name}{maybe_id}",
                remote_name = ref_debug_string(remote_ref_name.as_ref(), None),
                maybe_id = remote_tracking_branch_id
                    .or(sibling_id)
                    .map(|id| format!(" →:{}:", id))
                    .unwrap_or_default()
            ))
            .unwrap_or_default()
    )
}

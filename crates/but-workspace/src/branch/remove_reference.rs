use anyhow::bail;
use but_core::RefMetadata;
use but_rebase::graph_rebase::{Editor, LookupStep as _, Selector, Step, SuccessfulRebase};
use gix::{bstr::ByteSlice as _, refs::Category};

use crate::editor_workspace::{
    leave_unnecessary_workspace, remove_lane_without_tip, switch_to_workspace, workspace_tips,
};

/// Remove the local branch `ref_name` from the step graph of `editor`, keeping its commits.
/// GitButler's own branches are refused, as is a branch the step graph doesn't have.
///
/// A branch on top of its lane, or that `HEAD` is on alone, gives its commits to the next branch
/// below, which takes its place, along with `HEAD`.
///
/// Otherwise, a lane that it leaves with nothing but the target beneath is taken out of the
/// workspace commit, and a workspace that is unnecessary afterwards is left, see
/// [`leave_unnecessary_workspace()`](crate::editor_workspace::leave_unnecessary_workspace()).
/// With a target, `HEAD` on `ref_name` alone enters the workspace where it is rather than being
/// left detached, so it's left again for the local target or the branch below if either has the
/// same files.
pub fn drop_reference<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    ref_name: &gix::refs::FullNameRef,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    if ref_name.category() != Some(Category::LocalBranch) {
        bail!(
            "Can only delete local branches under refs/heads, got '{}'",
            ref_name.as_bstr()
        );
    }
    if but_core::branch::is_technical_branch_name(ref_name.shorten()) {
        bail!(
            "Cannot delete GitButler technical branch '{}'",
            ref_name.shorten()
        );
    }
    let reference = editor.select_reference(ref_name)?;
    let on_top = editor.head() == Some(reference) || workspace_tips(&editor)?.contains(&reference);
    if on_top && let Some(below) = branch_below_its_commits(&editor, reference)? {
        let Step::Reference { refname, .. } = editor.replace_with_none(below)? else {
            bail!("BUG: the branch below '{ref_name}' isn't a reference");
        };
        editor.replace_reference(ref_name, refname)?;
        return editor.rebase();
    }

    let head_was_on_it = editor.head() == Some(reference) && editor.target_ref().is_some();
    editor.replace_with_none(reference)?;
    remove_lane_without_tip(&mut editor, reference)?;
    if head_was_on_it {
        editor = switch_to_workspace(editor)?.0.into_editor();
    }
    leave_unnecessary_workspace(editor.rebase()?.into_editor())
}

/// The next branch below `reference` before the target, if commits lie between them.
fn branch_below_its_commits<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    reference: Selector,
) -> anyhow::Result<Option<Selector>> {
    let target = editor.target_selector();
    let mut has_commits = false;
    let mut step = first_parent(editor, reference)?;
    while let Some(selector) = step.filter(|selector| Some(*selector) != target) {
        match editor.lookup_step(selector)? {
            Step::Pick(_) => has_commits = true,
            Step::Reference { refname, .. }
                if refname.category() == Some(Category::LocalBranch)
                    && editor.target_ref() != Some(refname.as_ref())
                    && !refname.as_bstr().starts_with_str("refs/heads/gitbutler/") =>
            {
                return Ok(has_commits.then_some(selector));
            }
            Step::Reference { .. } | Step::None => {}
        }
        step = first_parent(editor, selector)?;
    }
    Ok(None)
}

fn first_parent<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selector: Selector,
) -> anyhow::Result<Option<Selector>> {
    Ok(editor
        .direct_parents(selector)?
        .into_iter()
        .min_by_key(|(_, order)| *order)
        .map(|(parent, _)| parent))
}

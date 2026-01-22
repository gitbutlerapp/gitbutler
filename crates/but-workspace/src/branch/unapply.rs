//! Backwards apply

use anyhow::{Result, bail};
use but_core::Commit;
use but_graph::projection::{Stack, Workspace};
use but_rebase::graph_rebase::{Editor, GraphExt, Selector, SuccessfulRebase};
use gitbutler_stack::StackId;

/// This is a draft, there are several things I dont like about it:
/// - Takes a repo
/// - Creates it's own editor. Is this necesary?
pub fn unapply(
    repo: &gix::Repository,
    workspace: &Workspace,
    stack_id: StackId,
) -> Result<SuccessfulRebase> {
    let Some(integration_commit) = workspace.graph.managed_entrypoint_commit(repo)? else {
        bail!("Unapply only operates in a managed workspace");
    };
    // This is gross.
    let editor = workspace.graph.to_editor(repo)?;

    // This is horrible.
    let Some(stack) = workspace.stacks.iter().find(|s| s.id == Some(stack_id)) else {
        bail!("Failed to find stack by ID");
    };

    if let Some(parent_selector) = select_parent(&editor, stack)? {
        let integration_selector = editor.select_commit(integration_commit.id)?;
    }

    todo!()
}

/// We want to remove an edge (if it exists) between the integration commit and
/// stack we want to remove.
fn select_parent(editor: &Editor, stack: &Stack) -> Result<Option<Selector>> {
    // If the stack has no tip, then there is nothing we need to remove because
    // the stack is being defiend by metadata, not it's commits or integration
    // commit parentage. ... probably. Tests would be nice
    if stack.tip_skip_empty().is_none() {
        return Ok(None);
    }

    if let Some(reference) = stack.ref_name() {
        return Ok(Some(editor.select_reference(reference)?));
    }

    if let Some(tip) = stack.tip_skip_empty() {
        return Ok(Some(editor.select_commit(tip)?));
    }

    bail!("Unable to select parent")
}

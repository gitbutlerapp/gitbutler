use std::{fs, path::PathBuf};

use anyhow::{Context as _, Result, bail};
use bstr::BString;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_serde::BStringForFrontend;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

/// The reference the app will checkout when the workspace is open
pub const WORKSPACE_BRANCH_REF: &str = "refs/heads/gitbutler/workspace";

/// Previous workspace reference, delete after transition.
pub const INTEGRATION_BRANCH_REF: &str = "refs/heads/gitbutler/integration";

/// To prevent clients hitting the "looks like you've moved away from..."
/// after upgrading to a version using the new gitbutler/workspace branch
/// name we need some transition period during which both are accepted.
/// The new branch will be checked out as soon as any modification is made
/// that triggers `update_workspace_commit`.
pub const OPEN_WORKSPACE_REFS: [&str; 2] = [INTEGRATION_BRANCH_REF, WORKSPACE_BRANCH_REF];

/// The reference the app will checkout when in edit mode
pub const EDIT_BRANCH_REF: &str = "refs/heads/gitbutler/edit";

fn edit_mode_metadata_path(ctx: &Context) -> PathBuf {
    ctx.project_data_dir().join("edit_mode_metadata.toml")
}

#[doc(hidden)]
pub fn read_edit_mode_metadata(ctx: &Context) -> Result<EditModeMetadata> {
    let edit_mode_metadata = fs::read_to_string(edit_mode_metadata_path(ctx).as_path())
        .context("Failed to read edit mode metadata")?;

    toml::from_str(&edit_mode_metadata).context("Failed to parse edit mode metadata")
}

#[doc(hidden)]
pub fn write_edit_mode_metadata(
    ctx: &Context,
    edit_mode_metadata: &EditModeMetadata,
) -> Result<()> {
    let serialized_edit_mode_metadata =
        toml::to_string(edit_mode_metadata).context("Failed to serialize edit mode metadata")?;
    but_fs::write(
        edit_mode_metadata_path(ctx).as_path(),
        serialized_edit_mode_metadata,
    )
    .context("Failed to write edit mode metadata")?;

    Ok(())
}

/// Holds relevant state required to switch to and from edit mode
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EditModeMetadata {
    /// The sha of the commit getting edited.
    #[serde(with = "but_serde::oid")]
    pub commit_oid: git2::Oid,
    /// The ref of the vbranch which owns this commit.
    pub stack_id: StackId,
}

#[derive(Debug, Default, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OutsideWorkspaceMetadata {
    /// The name of the currently checked out branch or None if in detached head state.
    #[serde(with = "but_serde::bstring_opt_lossy")]
    pub branch_name: Option<BString>,
    /// The paths of any files that would conflict with the workspace as it currently is
    pub worktree_conflicts: Vec<BStringForFrontend>,
}

#[derive(PartialEq, Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject")]
pub enum OperatingMode {
    /// The typical app state when it's on the gitbutler/workspace branch
    OpenWorkspace,
    /// When the user has chosen to leave the gitbutler/workspace branch
    OutsideWorkspace(OutsideWorkspaceMetadata),
    /// When the app is off of gitbutler/workspace and in edit mode
    Edit(EditModeMetadata),
}

pub fn operating_mode(ctx: &Context) -> OperatingMode {
    let repo = ctx.git2_repo.get().unwrap();
    let Ok(head_ref) = repo.head() else {
        return OperatingMode::OutsideWorkspace(
            outside_workspace_metadata(ctx).unwrap_or_default(),
        );
    };

    let Some(head_ref_name) = head_ref.name() else {
        return OperatingMode::OutsideWorkspace(
            outside_workspace_metadata(ctx).unwrap_or_default(),
        );
    };

    if OPEN_WORKSPACE_REFS.contains(&head_ref_name) {
        OperatingMode::OpenWorkspace
    } else if head_ref_name == EDIT_BRANCH_REF {
        let edit_mode_metadata = read_edit_mode_metadata(ctx);

        match edit_mode_metadata {
            Ok(edit_mode_metadata) => OperatingMode::Edit(edit_mode_metadata),
            Err(error) => {
                tracing::warn!(
                    "Failed to open in edit mode, falling back to outside workspace {}",
                    error
                );
                OperatingMode::OutsideWorkspace(outside_workspace_metadata(ctx).unwrap_or_default())
            }
        }
    } else {
        OperatingMode::OutsideWorkspace(outside_workspace_metadata(ctx).unwrap_or_default())
    }
}

fn outside_workspace_metadata(ctx: &Context) -> Result<OutsideWorkspaceMetadata> {
    // We do a virtual-merge, extracting conflicts.
    let gix_repo = ctx.clone_repo_for_merging_non_persisting()?;

    let head = gix_repo.head()?;
    let branch_name = head
        .referent_name()
        .map(|r| r.as_partial_name().as_bstr().to_owned());

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let applied_stacks = vb_state.list_stacks_in_workspace()?;

    if vb_state.maybe_get_default_target()?.is_none() || applied_stacks.is_empty() {
        // Nothing to conflict
        return Ok(OutsideWorkspaceMetadata {
            branch_name,
            worktree_conflicts: vec![],
        });
    }

    let (outcome, conflict_kind) =
        but_workspace::legacy::merge_worktree_with_workspace(ctx, &gix_repo)?;
    let worktree_conflicts = outcome
        .conflicts
        .iter()
        .filter(|c| c.is_unresolved(conflict_kind))
        .map(|c| c.ours.location().into())
        .collect::<Vec<BStringForFrontend>>();

    Ok(OutsideWorkspaceMetadata {
        branch_name,
        worktree_conflicts,
    })
}

pub fn in_open_workspace_mode(ctx: &Context) -> bool {
    operating_mode(ctx) == OperatingMode::OpenWorkspace
}

pub fn ensure_open_workspace_mode(ctx: &Context) -> Result<()> {
    if in_open_workspace_mode(ctx) {
        Ok(())
    } else {
        bail!("Expected to be in open workspace mode")
    }
}

pub fn in_edit_mode(ctx: &Context) -> bool {
    matches!(operating_mode(ctx), OperatingMode::Edit(_))
}

pub fn ensure_edit_mode(ctx: &Context) -> Result<EditModeMetadata> {
    match operating_mode(ctx) {
        OperatingMode::Edit(edit_mode_metadata) => Ok(edit_mode_metadata),
        _ => bail!("Expected to be in edit mode"),
    }
}

pub fn in_outside_workspace_mode(ctx: &Context) -> bool {
    matches!(operating_mode(ctx), OperatingMode::OutsideWorkspace(_))
}

pub fn ensure_outside_workspace_mode(ctx: &Context) -> Result<()> {
    if in_outside_workspace_mode(ctx) {
        Ok(())
    } else {
        bail!("Expected to be in outside workspace mode")
    }
}

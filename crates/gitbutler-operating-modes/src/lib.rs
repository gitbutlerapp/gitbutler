use std::{fs, path::PathBuf};

use anyhow::{bail, Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_reference::ReferenceName;
use serde::{Deserialize, Serialize};

pub mod commands;

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

fn edit_mode_metadata_path(ctx: &CommandContext) -> PathBuf {
    ctx.project().gb_dir().join("edit_mode_metadata.toml")
}

#[doc(hidden)]
pub fn read_edit_mode_metadata(ctx: &CommandContext) -> Result<EditModeMetadata> {
    let edit_mode_metadata = fs::read_to_string(edit_mode_metadata_path(ctx).as_path())
        .context("Failed to read edit mode metadata")?;

    toml::from_str(&edit_mode_metadata).context("Failed to parse edit mode metadata")
}

#[doc(hidden)]
pub fn write_edit_mode_metadata(
    ctx: &CommandContext,
    edit_mode_metadata: &EditModeMetadata,
) -> Result<()> {
    let serialized_edit_mode_metadata =
        toml::to_string(edit_mode_metadata).context("Failed to serialize edit mode metadata")?;
    gitbutler_fs::write(
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
    #[serde(with = "gitbutler_serde::oid")]
    pub commit_oid: git2::Oid,
    /// The ref of the vbranch which owns this commit.
    pub branch_reference: ReferenceName,
}

#[derive(PartialEq, Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject")]
pub enum OperatingMode {
    /// The typical app state when its on the gitbutler/workspace branch
    OpenWorkspace,
    /// When the user has chosen to leave the gitbutler/workspace branch
    OutsideWorkspace,
    /// When the app is off of gitbutler/workspace and in edit mode
    Edit(EditModeMetadata),
}

pub fn operating_mode(ctx: &CommandContext) -> OperatingMode {
    let Ok(head_ref) = ctx.repo().head() else {
        return OperatingMode::OutsideWorkspace;
    };

    let Some(head_ref_name) = head_ref.name() else {
        return OperatingMode::OutsideWorkspace;
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
                OperatingMode::OutsideWorkspace
            }
        }
    } else {
        OperatingMode::OutsideWorkspace
    }
}

pub fn in_open_workspace_mode(ctx: &CommandContext) -> bool {
    operating_mode(ctx) == OperatingMode::OpenWorkspace
}

pub fn assure_open_workspace_mode(ctx: &CommandContext) -> Result<()> {
    if in_open_workspace_mode(ctx) {
        Ok(())
    } else {
        bail!("Expected to be in open workspace mode")
    }
}

pub fn in_edit_mode(ctx: &CommandContext) -> bool {
    matches!(operating_mode(ctx), OperatingMode::Edit(_))
}

pub fn assure_edit_mode(ctx: &CommandContext) -> Result<EditModeMetadata> {
    match operating_mode(ctx) {
        OperatingMode::Edit(edit_mode_metadata) => Ok(edit_mode_metadata),
        _ => bail!("Expected to be in edit mode"),
    }
}

pub fn in_outside_workspace_mode(ctx: &CommandContext) -> bool {
    operating_mode(ctx) == OperatingMode::OutsideWorkspace
}

pub fn assure_outside_workspace_mode(ctx: &CommandContext) -> Result<()> {
    if in_outside_workspace_mode(ctx) {
        Ok(())
    } else {
        bail!("Expected to be in outside workspace mode")
    }
}

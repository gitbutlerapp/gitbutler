use std::{fs, path::PathBuf};

use anyhow::{Context as _, Result, bail};
use bstr::BString;
use but_core::{ref_metadata::StackId, sync::RepoShared};
use but_ctx::Context;
use but_serde::BStringForFrontend;
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
const OPEN_WORKSPACE_REFS: [&str; 2] = [WORKSPACE_BRANCH_REF, INTEGRATION_BRANCH_REF];

/// The reference the app will checkout when in edit mode
pub const EDIT_BRANCH_REF: &str = "refs/heads/gitbutler/edit";

/// Return `true` if `ref_name` is one of the well-known refs used for open workspace mode
/// as identified by [`WORKSPACE_BRANCH_REF`] or [`INTEGRATION_BRANCH_REF`]
pub fn is_well_known_workspace_ref(ref_name: &gix::refs::FullNameRef) -> bool {
    OPEN_WORKSPACE_REFS
        .iter()
        .any(|workspace_ref| ref_name.as_bstr() == *workspace_ref)
}

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
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditModeMetadata {
    /// The sha of the commit getting edited.
    #[serde(with = "but_serde::object_id")]
    #[schemars(with = "String")]
    pub commit_oid: gix::ObjectId,
    /// The ref of the vbranch which owns this commit.
    #[schemars(with = "String")]
    pub stack_id: StackId,
}

but_schemars::register_sdk_type!(EditModeMetadata);

#[derive(Debug, Default, Serialize, PartialEq, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutsideWorkspaceMetadata {
    /// The name of the currently checked out branch or None if in detached head state.
    #[serde(with = "but_serde::bstring_lossy_opt")]
    #[schemars(with = "Option<String>")]
    pub branch_name: Option<BString>,
    /// The paths of any files that would conflict with the workspace as it currently is
    #[schemars(with = "Vec<String>")]
    pub worktree_conflicts: Vec<BStringForFrontend>,
}

but_schemars::register_sdk_type!(OutsideWorkspaceMetadata);

#[derive(PartialEq, Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(tag = "type", content = "subject")]
pub enum OperatingMode {
    /// The typical app state when it's on the gitbutler/workspace branch
    OpenWorkspace,
    /// When the user has chosen to leave the gitbutler/workspace branch
    OutsideWorkspace(OutsideWorkspaceMetadata),
    /// When the app is off of gitbutler/workspace and in edit mode
    Edit(EditModeMetadata),
}
but_schemars::register_sdk_type!(OperatingMode);

pub fn operating_mode(ctx: &Context, perm: &RepoShared) -> Result<OperatingMode> {
    let repo = ctx.repo.get()?;
    let Ok(head_ref) = repo.head() else {
        return Ok(OperatingMode::OutsideWorkspace(
            outside_workspace_metadata(ctx, perm).unwrap_or_default(),
        ));
    };

    let Some(head_ref_name) = head_ref.referent_name() else {
        return Ok(OperatingMode::OutsideWorkspace(
            outside_workspace_metadata(ctx, perm).unwrap_or_default(),
        ));
    };
    if is_well_known_workspace_ref(head_ref_name) {
        Ok(OperatingMode::OpenWorkspace)
    } else if head_ref_name.as_bstr() == EDIT_BRANCH_REF {
        let edit_mode_metadata = read_edit_mode_metadata(ctx);

        match edit_mode_metadata {
            Ok(edit_mode_metadata) => Ok(OperatingMode::Edit(edit_mode_metadata)),
            Err(error) => {
                tracing::warn!(
                    "Failed to open in edit mode, falling back to outside workspace {}",
                    error
                );
                Ok(OperatingMode::OutsideWorkspace(
                    outside_workspace_metadata(ctx, perm).unwrap_or_default(),
                ))
            }
        }
    } else {
        Ok(OperatingMode::OutsideWorkspace(
            outside_workspace_metadata(ctx, perm).unwrap_or_default(),
        ))
    }
}

fn outside_workspace_metadata(
    ctx: &Context,
    perm: &RepoShared,
) -> Result<OutsideWorkspaceMetadata> {
    // We do a virtual-merge, extracting conflicts.
    let gix_repo = ctx.clone_repo_for_merging_non_persisting()?;

    let head = gix_repo.head()?;
    let branch_name = head
        .referent_name()
        .map(|r| r.as_partial_name().as_bstr().to_owned());

    let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
    if ws.target_ref.is_none() || ws.stacks.is_empty() {
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

pub fn in_open_workspace_mode(ctx: &Context, perm: &RepoShared) -> Result<bool> {
    Ok(operating_mode(ctx, perm)? == OperatingMode::OpenWorkspace)
}

pub fn ensure_open_workspace_mode(ctx: &Context, perm: &RepoShared) -> Result<()> {
    if in_open_workspace_mode(ctx, perm)? {
        Ok(())
    } else {
        bail!("Expected to be in open workspace mode")
    }
}

pub fn in_edit_mode(ctx: &Context, perm: &RepoShared) -> Result<bool> {
    Ok(matches!(operating_mode(ctx, perm)?, OperatingMode::Edit(_)))
}

pub fn ensure_edit_mode(ctx: &Context, perm: &RepoShared) -> Result<EditModeMetadata> {
    match operating_mode(ctx, perm)? {
        OperatingMode::Edit(edit_mode_metadata) => Ok(edit_mode_metadata),
        _ => bail!("Expected to be in edit mode"),
    }
}

pub fn in_outside_workspace_mode(ctx: &Context, perm: &RepoShared) -> Result<bool> {
    Ok(matches!(
        operating_mode(ctx, perm)?,
        OperatingMode::OutsideWorkspace(_)
    ))
}

pub fn ensure_outside_workspace_mode(ctx: &Context, perm: &RepoShared) -> Result<()> {
    if in_outside_workspace_mode(ctx, perm)? {
        Ok(())
    } else {
        bail!("Expected to be in outside workspace mode")
    }
}

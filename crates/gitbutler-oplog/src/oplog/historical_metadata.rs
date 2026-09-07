//! Read historical snapshot payloads directly into current reference metadata.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result};
use but_core::ref_metadata::{
    Branch, Review, StackId, Workspace, WorkspaceCommitRelation, WorkspaceStack,
    WorkspaceStackBranch,
};

use super::{SnapshotRefMetadata, SnapshotReference, SnapshotTarget};

pub(super) fn read(
    tree: &gix::Tree<'_>,
    repo: &gix::Repository,
) -> Result<(SnapshotRefMetadata, Option<SnapshotTarget>)> {
    let entry = tree
        .lookup_entry_by_path("virtual_branches.toml")?
        .context("snapshot has no reference metadata")?;
    let blob = repo.find_blob(entry.id())?;
    let data: toml::Value = toml::from_str(std::str::from_utf8(&blob.data)?)
        .context("failed to parse historical snapshot metadata")?;
    let target = data
        .get("default_target")
        .cloned()
        .map(toml::Value::try_into)
        .transpose()?;
    let mut stacks = data
        .get("branches")
        .and_then(toml::Value::as_table)
        .context("historical snapshot has no branches table")?
        .iter()
        .collect::<Vec<_>>();
    stacks.sort_by_key(|(id, stack)| {
        (
            stack
                .get("order")
                .and_then(toml::Value::as_integer)
                .unwrap_or_default(),
            *id,
        )
    });
    let mut workspace = Workspace::default();
    // The previous metadata projection used this stable marker for managed workspaces.
    workspace.ref_info.created_at = Some(gix::date::Time::new(1675176957, 0));
    let mut branches = BTreeMap::new();
    let mut references = BTreeMap::new();
    for (id, stack) in stacks {
        let id: StackId = id.parse().context("invalid historical stack ID")?;
        let in_workspace = stack
            .get("in_workspace")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        let mut stack_branches = Vec::new();
        let mut seen = BTreeSet::new();
        for head in stack
            .get("heads")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .rev()
        {
            let name = head
                .get("name")
                .and_then(toml::Value::as_str)
                .context("historical snapshot branch has no name")?;
            let name = gix::refs::Category::LocalBranch.to_full_name(name.trim_matches('/'))?;
            if !seen.insert(name.clone()) {
                continue;
            }
            stack_branches.push(WorkspaceStackBranch {
                ref_name: name.clone(),
                archived: head
                    .get("archived")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or_default(),
            });
            let pull_request = head
                .get("pr_number")
                .map(|number| -> Result<_> {
                    usize::try_from(
                        number
                            .as_integer()
                            .context("invalid historical pull request number")?,
                    )
                    .context("negative historical pull request number")
                })
                .transpose()?;
            branches.entry(name.clone()).or_insert_with(|| Branch {
                review: Review {
                    pull_request,
                    review_id: head
                        .get("review_id")
                        .and_then(toml::Value::as_str)
                        .map(ToOwned::to_owned),
                },
                ..Default::default()
            });
            // Old ChangeId payloads have no recoverable object ID. Preserve their metadata
            // without treating unavailable target information as an absent reference.
            if in_workspace
                && let Some(id) = head
                    .get("head")
                    .or_else(|| head.get("target"))
                    .and_then(|value| value.get("CommitId"))
                    .and_then(toml::Value::as_str)
            {
                let id: gix::ObjectId =
                    id.parse().context("invalid historical branch commit ID")?;
                if !id.is_null() {
                    references.entry(name).or_insert(id);
                }
            }
        }
        if !stack_branches.is_empty() {
            workspace.stacks.push(WorkspaceStack {
                id,
                branches: stack_branches,
                workspacecommit_relation: if in_workspace {
                    WorkspaceCommitRelation::Merged
                } else {
                    WorkspaceCommitRelation::Outside
                },
            });
        }
    }
    let workspaces = if workspace.stacks.is_empty() {
        Vec::new()
    } else {
        vec![(but_core::WORKSPACE_REF_NAME.try_into()?, workspace)]
    };
    let metadata =
        but_db::Metadata::from_parts(workspaces, branches.into_iter().collect(), Vec::new());
    metadata
        .validate()
        .context("invalid historical reference metadata")?;
    Ok((
        SnapshotRefMetadata {
            metadata,
            references: references
                .into_iter()
                .map(|(ref_name, target)| SnapshotReference {
                    ref_name,
                    target: Some(target),
                })
                .collect(),
        },
        target,
    ))
}

//! Conversions for archived virtual-branch metadata and explicit fixture imports.

use std::{collections::HashMap, str::FromStr};

use anyhow::{Context as _, anyhow};
use but_core::ref_metadata::StackId;
use but_db::{VbStack, VbStackHead, VbState, VirtualBranchesSnapshot};
use gitbutler_reference::{Refname, RemoteRefname};

use crate::virtual_branches_legacy_types::{Stack, StackBranch, VirtualBranches};

/// Convert an archived virtual-branch payload into normalized database rows.
pub fn legacy_to_snapshot(vb: &VirtualBranches) -> anyhow::Result<VirtualBranchesSnapshot> {
    let initialized = true;
    let last_pushed_base_sha = vb.last_pushed_base.map(|oid| oid.to_string());

    let mut stacks: Vec<_> = vb.branches.iter().collect();
    stacks.sort_by_key(|(sid, stack)| (stack.order, **sid));

    let mut out_stacks = Vec::with_capacity(stacks.len());
    let mut out_heads = Vec::new();
    for (stack_id, stack) in stacks {
        let Stack {
            id: _, // should match with `stack_id`, but we effectively normalise on the unique key here
            source_refname,
            upstream,
            order,
            in_workspace,
            heads,
            #[expect(deprecated)]
            notes,
            #[expect(deprecated)]
            ownership,
            #[expect(deprecated)]
            allow_rebasing,
            #[expect(deprecated)]
            post_commits,
            #[expect(deprecated)]
            tree,
            #[expect(deprecated)]
            created_timestamp_ms,
            #[expect(deprecated)]
            updated_timestamp_ms,
            #[expect(deprecated)]
            name,
            #[expect(deprecated)]
            head,
        } = stack;
        out_stacks.push(VbStack {
            id: stack_id.to_string(),
            source_refname: source_refname.as_ref().map(ToString::to_string),
            upstream_remote_name: upstream.as_ref().map(|up| up.remote().to_owned()),
            upstream_branch_name: upstream.as_ref().map(|up| up.branch().to_owned()),
            sort_order: i64::try_from(*order).context("Stack order exceeds i64")?,
            in_workspace: *in_workspace,
            legacy_name: name.clone(),
            legacy_notes: notes.clone(),
            legacy_ownership: ownership.to_string(),
            legacy_allow_rebasing: *allow_rebasing,
            legacy_post_commits: *post_commits,
            legacy_tree_sha: tree.to_string(),
            legacy_head_sha: head.to_string(),
            legacy_created_timestamp_ms: created_timestamp_ms.to_string(),
            legacy_updated_timestamp_ms: updated_timestamp_ms.to_string(),
        });

        for (position, head) in heads.iter().enumerate() {
            let StackBranch {
                head,
                name,
                pr_number,
                archived,
                review_id,
            } = head;
            out_heads.push(VbStackHead {
                stack_id: stack_id.to_string(),
                position: i64::try_from(position).context("Head position exceeds i64")?,
                name: name.clone(),
                head_sha: head.to_string(),
                pr_number: pr_number
                    .map(i64::try_from)
                    .transpose()
                    .context("PR number exceeds i64")?,
                archived: *archived,
                review_id: review_id.clone(),
            });
        }
    }

    Ok(VirtualBranchesSnapshot {
        state: VbState {
            last_pushed_base_sha,
            initialized,
            ..Default::default()
        },
        stacks: out_stacks,
        heads: out_heads,
    })
}

/// Convert normalized database rows into the legacy payload retained in oplog snapshots.
pub fn snapshot_to_legacy(snapshot: &VirtualBranchesSnapshot) -> anyhow::Result<VirtualBranches> {
    let VirtualBranchesSnapshot {
        state,
        stacks,
        heads,
    } = snapshot;
    let last_pushed_base = state
        .last_pushed_base_sha
        .as_ref()
        .map(|value| {
            gix::ObjectId::from_str(value)
                .with_context(|| format!("Invalid last_pushed_base sha: {value}"))
        })
        .transpose()?;

    let mut branches = HashMap::new();
    for stack in stacks {
        let VbStack {
            id,
            source_refname,
            upstream_remote_name,
            upstream_branch_name,
            sort_order,
            in_workspace,
            legacy_name,
            legacy_notes,
            legacy_ownership,
            legacy_allow_rebasing,
            legacy_post_commits,
            legacy_tree_sha,
            legacy_head_sha,
            legacy_created_timestamp_ms,
            legacy_updated_timestamp_ms,
        } = stack;
        let stack_id = StackId::from_str(id).with_context(|| format!("Invalid stack id '{id}'"))?;
        let source_refname = source_refname
            .as_ref()
            .map(|name| {
                Refname::from_str(name).with_context(|| format!("Invalid source_refname '{name}'"))
            })
            .transpose()?;
        let upstream = match (upstream_remote_name, upstream_branch_name) {
            (Some(remote), Some(branch)) => Some(RemoteRefname::new(remote, branch)),
            _ => None,
        };

        branches.insert(
            stack_id,
            Stack {
                id: stack_id,
                source_refname,
                upstream,
                order: usize::try_from(*sort_order)
                    .with_context(|| format!("Invalid stack sort order '{sort_order}'"))?,
                in_workspace: *in_workspace,
                heads: Vec::new(),
                #[expect(deprecated)]
                notes: legacy_notes.clone(),
                #[expect(deprecated)]
                ownership: legacy_ownership
                    .parse()
                    .with_context(|| format!("Invalid ownership claims for '{stack_id}'"))?,
                #[expect(deprecated)]
                allow_rebasing: *legacy_allow_rebasing,
                #[expect(deprecated)]
                post_commits: *legacy_post_commits,
                #[expect(deprecated)]
                tree: gix::ObjectId::from_str(legacy_tree_sha).with_context(|| {
                    format!("Invalid legacy tree sha '{legacy_tree_sha}' for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                created_timestamp_ms: legacy_created_timestamp_ms.parse().with_context(|| {
                    format!("Invalid legacy created timestamp for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                updated_timestamp_ms: legacy_updated_timestamp_ms.parse().with_context(|| {
                    format!("Invalid legacy updated timestamp for '{stack_id}'")
                })?,
                #[expect(deprecated)]
                name: legacy_name.clone(),
                #[expect(deprecated)]
                head: gix::ObjectId::from_str(legacy_head_sha).with_context(|| {
                    format!("Invalid legacy head sha '{legacy_head_sha}' for '{stack_id}'")
                })?,
            },
        );
    }

    for head in heads {
        let VbStackHead {
            stack_id,
            position: _, // previously set based on vec position
            name,
            head_sha,
            pr_number,
            archived,
            review_id,
        } = head;
        let stack_id = StackId::from_str(stack_id)
            .with_context(|| format!("Invalid stack id '{stack_id}'"))?;
        let stack = branches
            .get_mut(&stack_id)
            .ok_or_else(|| anyhow!("Missing stack '{stack_id}' for head '{name}'"))?;
        stack.heads.push(StackBranch {
            head: gix::ObjectId::from_str(head_sha)
                .with_context(|| format!("Invalid head sha '{head_sha}' on '{name}'"))?,
            name: name.clone(),
            pr_number: pr_number
                .map(usize::try_from)
                .transpose()
                .with_context(|| {
                    format!(
                        "Invalid pr_number '{}' on stack '{stack_id}'",
                        pr_number.unwrap_or_default(),
                    )
                })?,
            archived: *archived,
            review_id: review_id.clone(),
        });
    }

    Ok(VirtualBranches {
        branches,
        last_pushed_base,
    })
}

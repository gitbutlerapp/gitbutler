use anyhow::{Context as _, bail};
use gitbutler_stack::{StackId, VirtualBranchesState};
use gix::{prelude::ObjectIdExt, revision::walk::Sorting};

use crate::legacy::commit_engine::ReferenceFrame;

/// How to infer the reference frame.
pub enum InferenceMode {
    /// Use the commit ID that is assumed to be part of any stack, the stack whose tip we are to find.
    CommitIdInStack(gix::ObjectId),
    /// A specific stack is known by ID, and we should use its tip directly.
    StackId(StackId),
}

impl ReferenceFrame {
    /// Create a reference frame using the information in `vb` and `mode`.
    pub fn infer(
        repo: &gix::Repository,
        vb: &VirtualBranchesState,
        mode: InferenceMode,
    ) -> anyhow::Result<Self> {
        let head_id = repo.head_id()?;
        let workspace_commit = head_id.object()?.into_commit().decode()?.to_owned();

        let cache = repo.commit_graph_if_enabled()?;
        let mut graph = repo.revision_graph(cache.as_ref());
        let default_target_tip = vb
            .default_target
            .as_ref()
            .map(|target| -> anyhow::Result<_> {
                let r = repo.find_reference(&target.branch.to_string())?;
                Ok(r.try_id())
            })
            .and_then(Result::ok)
            .flatten();

        let merge_base = if default_target_tip.is_none() {
            Some(repo.merge_base_octopus(workspace_commit.parents)?)
        } else {
            None
        };
        match mode {
            InferenceMode::StackId(stack_id) => {
                let stack = vb
                    .branches
                    .get(&stack_id)
                    .context("Didn't find stack - was it deleted just now?")?;
                Ok(ReferenceFrame {
                    workspace_tip: Some(head_id.detach()),
                    branch_tip: Some(stack.head_oid(repo)?),
                })
            }
            InferenceMode::CommitIdInStack(commit_id) => {
                for stack in vb.branches.values() {
                    let stack_tip = stack.head_oid(repo)?;
                    if stack_tip
                        .attach(repo)
                        .ancestors()
                        .with_boundary(match default_target_tip {
                            Some(target_tip) => {
                                Some(repo.merge_base_with_graph(stack_tip, target_tip, &mut graph)?)
                            }
                            None => merge_base,
                        })
                        .sorting(Sorting::BreadthFirst)
                        .all()?
                        .filter_map(Result::ok)
                        .any(|info| info.id == commit_id)
                    {
                        return Ok(ReferenceFrame {
                            workspace_tip: Some(head_id.detach()),
                            branch_tip: Some(stack_tip),
                        });
                    }
                }
                bail!("Could not find stack that includes parent-id at {commit_id}")
            }
        }
    }
}

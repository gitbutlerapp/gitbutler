use std::borrow::Cow;

use anyhow::Context;

/// For use in [`Anchor`].
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Position {
    /// The new dependent branch will appear above its anchor.
    Above,
    /// The new dependent branch will appear below its anchor.
    Below,
}

struct MinimalCommit<'a> {
    id: gix::ObjectId,
    parent_ids: &'a [gix::ObjectId],
}

impl<'a> From<&'a but_graph::Commit> for MinimalCommit<'a> {
    fn from(value: &'a but_graph::Commit) -> Self {
        MinimalCommit {
            id: value.id,
            parent_ids: &value.parent_ids,
        }
    }
}

impl<'a> From<&'a but_graph::projection::StackCommit> for MinimalCommit<'a> {
    fn from(value: &'a but_graph::projection::StackCommit) -> Self {
        MinimalCommit {
            id: value.id,
            parent_ids: &value.parent_ids,
        }
    }
}

impl Position {
    fn resolve_commit(
        &self,
        commit: MinimalCommit<'_>,
        ws_base: Option<gix::ObjectId>,
    ) -> anyhow::Result<gix::ObjectId> {
        if Some(commit.id) == ws_base {
            return Ok(commit.id);
        }
        Ok(match self {
            Position::Above => commit.id,
            Position::Below => commit.parent_ids.iter().cloned().next().with_context(|| {
                format!(
                    "Commit {id} is the first in history and no branch can point below it",
                    id = commit.id
                )
            })?,
        })
    }
}

/// For use in [`super::create_reference()`].
///
/// *Note* that even though it's possible to resolve any ref as commit-id, making this
/// type *seem redundant*, it's not possible to unambiguously describe where a ref should
/// go just by commit. We must be specifying it in terms of above/below ref-name when possible,
/// or else they will always go on top.
#[derive(Debug, Clone)]
pub enum Anchor<'a> {
    /// Use a commit as position, which means we always need unambiguous placement
    /// without a way to stack references on top of other references - only on top
    /// of commits their segments may own.
    AtCommit {
        /// The commit to use as reference point for `position`.
        commit_id: gix::ObjectId,
        /// `Above` means the reference will point at `commit_id`, `Below` means it points at its
        /// parent if possible.
        position: Position,
    },
    /// Use a segment as reference for positioning the new reference.
    /// Without a workspace, this is the same as saying 'the commit that the segment points to'.
    AtSegment {
        /// The name of the segment to use as reference point for `position`.
        ref_name: Cow<'a, gix::refs::FullNameRef>,
        /// `Above` means the reference will be right above the segment with `ref_name` even
        /// if it points to the same commit.
        /// `Below` means the reference will be right below the segment with `ref_name` even
        /// if it points to the same commit.
        position: Position,
    },
}

impl<'a> Anchor<'a> {
    /// Create a new instance with an object ID as anchor.
    pub fn at_id(commit_id: impl Into<gix::ObjectId>, position: Position) -> Self {
        Anchor::AtCommit {
            commit_id: commit_id.into(),
            position,
        }
    }

    /// Create a new instance with a segment name as anchor.
    pub fn at_segment(ref_name: &'a gix::refs::FullNameRef, position: Position) -> Self {
        Anchor::AtSegment {
            ref_name: Cow::Borrowed(ref_name),
            position,
        }
    }
}

pub(super) mod function {
    #![expect(clippy::indexing_slicing)]

    use std::borrow::{Borrow, Cow};

    use crate::branch::create_reference::{Anchor, Position};
    use anyhow::{Context, bail};
    use but_core::ref_metadata::WorkspaceCommitRelation::Merged;
    use but_core::{
        RefMetadata, ref_metadata,
        ref_metadata::{
            StackId, StackKind::AppliedAndUnapplied, WorkspaceStack, WorkspaceStackBranch,
        },
    };
    use gix::refs::transaction::PreviousValue;

    /// Create a new reference named `ref_name` to point at a commit relative to `anchor`.
    /// If `anchor` is `None` this means the branch should be placed above the lower bound of the workspace, effectively
    /// creating an independent branch.
    /// The resulting reference will be created in `repo` and `meta` will be updated for `ref_name` so the workspace
    /// contains it, but only if it's a managed workspace, along with branch metadata.
    /// Use `new_stack_id` just with `Stack::generate()`, it's mainly used to be able to control the stack-id when needed in testing.
    ///
    /// Fail if the reference already exists *and* points somewhere else.
    ///
    ///  - if there is no managed workspace, then dependent branches must be exclusive on each commit to identify ordering
    ///  - if there is a workspace, we store the order in workspace metadata and expect an `anchor` that names a segment.
    ///
    /// Return a regenerated Graph that contains the new reference, and from which a new workspace can be derived.
    pub fn create_reference<'name, T: RefMetadata>(
        ref_name: impl Borrow<gix::refs::FullNameRef>,
        anchor: impl Into<Option<Anchor<'name>>>,
        repo: &gix::Repository,
        workspace: &but_graph::projection::Workspace<'_>,
        meta: &mut T,
        new_stack_id: impl FnOnce(&gix::refs::FullNameRef) -> StackId,
    ) -> anyhow::Result<but_graph::Graph> {
        let anchor = anchor.into();

        let ws_base = workspace.lower_bound;
        // Note that we will never create metadata for a workspace!
        let mut existing_ws_meta = workspace
            .ref_name()
            .and_then(|ws_ref| meta.workspace_opt(ws_ref).transpose())
            .transpose()?;
        let ref_name = ref_name.borrow();

        let (check_if_id_in_workspace, ref_target_id, instruction): (
            _,
            _,
            Option<Instruction<'_>>,
        ) = {
            match anchor {
                None => {
                    // The new ref exists already in the workspace, do nothing.
                    if workspace
                        .find_segment_and_stack_by_refname(ref_name)
                        .is_some()
                    {
                        return Ok(workspace.graph.clone());
                    }
                    let base = ws_base.with_context(|| {
                        format!(
                            "workspace at {} is missing a base",
                            workspace.ref_name_display()
                        )
                    })?;
                    (
                        // do not validate, as the base is expectedly outside of workspace
                        false,
                        base,
                        Some(Instruction::Independent),
                    )
                }
                Some(Anchor::AtCommit {
                    commit_id,
                    position,
                }) => {
                    let mut validate_id = true;
                    let indexes = workspace.try_find_owner_indexes_by_commit_id(commit_id)?;
                    let ref_target_id = position
                        .resolve_commit(workspace.lookup_commit(indexes).into(), ws_base)?;
                    let id_out_of_workspace = Some(ref_target_id) == ws_base;
                    if id_out_of_workspace {
                        validate_id = false
                    }

                    let instruction = existing_ws_meta
                        .as_ref()
                        .filter(|_| !id_out_of_workspace)
                        .map(|_| instruction_by_named_anchor_for_commit(workspace, commit_id))
                        .or_else(|| {
                            let (stack_idx, _seg_idx, _cidx) = indexes;
                            workspace.stacks[stack_idx]
                                .id
                                .map(Instruction::DependentInStack)
                                .map(Ok)
                        })
                        .transpose()?;

                    (validate_id, ref_target_id, instruction)
                }
                Some(Anchor::AtSegment { ref_name, position }) => {
                    let mut validate_id = true;
                    let ref_target_id = if workspace.has_metadata() {
                        let (stack_idx, seg_idx) = workspace
                            .try_find_segment_owner_indexes_by_refname(ref_name.as_ref())?;
                        let segment = &workspace.stacks[stack_idx].segments[seg_idx];

                        let id = workspace
                            .graph
                            .tip_skip_empty(segment.id)
                            .map(|commit| position.resolve_commit(commit.into(), ws_base))
                            .context(
                                "BUG: we should always see through to the base or eligible commits",
                            )??;
                        if Some(id) == ws_base {
                            validate_id = false
                        }
                        id
                    } else {
                        let Some((_stack, segment)) =
                            workspace.find_segment_and_stack_by_refname(ref_name.as_ref())
                        else {
                            bail!(
                                "Could not find a segment named '{}' in workspace",
                                ref_name.shorten()
                            );
                        };
                        position.resolve_commit(
                            segment
                                .commits
                                .first()
                                .context("Cannot create reference on unborn branch")?
                                .into(),
                            ws_base,
                        )?
                    };
                    (
                        validate_id,
                        ref_target_id,
                        Some(Instruction::Dependent { ref_name, position }),
                    )
                }
            }
        };

        let updated_ws_meta = existing_ws_meta
            .take()
            .zip(instruction)
            .map(|(mut existing, instruction)| {
                update_workspace_metadata(&mut existing, ref_name, instruction, new_stack_id)
                    .map(|()| existing)
            })
            .transpose()?;
        // Assure this commit is in the workspace as well.
        if check_if_id_in_workspace {
            workspace.try_find_owner_indexes_by_commit_id(ref_target_id)?;
        }

        let graph_with_new_ref = {
            // Always update the metadata, this may help disambiguating.
            let mut branch_md = meta.branch(ref_name)?;
            update_branch_metadata(ref_name, repo, &mut branch_md)?;

            workspace.graph.redo_traversal_with_overlay(
                repo,
                meta,
                but_graph::init::Overlay::default()
                    .with_references_if_new(Some(gix::refs::Reference {
                        name: ref_name.into(),
                        target: gix::refs::Target::Object(ref_target_id),
                        peeled: None,
                    }))
                    .with_branch_metadata_override(Some((
                        branch_md.as_ref().to_owned(),
                        (*branch_md).clone(),
                    )))
                    .with_workspace_metadata_override(
                        updated_ws_meta
                            .as_ref()
                            .map(|ws| (ws.as_ref().to_owned(), (*ws).clone())),
                    ),
            )?
        };

        let updated_workspace = graph_with_new_ref.to_workspace()?;
        let has_new_ref_as_standalone_segment = updated_workspace
            .find_segment_and_stack_by_refname(ref_name)
            .is_some();
        if !has_new_ref_as_standalone_segment {
            // TODO: this should probably be easier to understand for the UI, with error codes maybe?
            bail!(
                "Reference '{}' cannot be created as segment at {ref_target_id}",
                ref_name.shorten()
            )
        }

        // Actually apply the changes
        repo.reference(
            ref_name,
            ref_target_id,
            PreviousValue::ExistingMustMatch(gix::refs::Target::Object(ref_target_id)),
            "Dependent branch by GitButler",
        )
        .map_err(|err| {
            let code = match err {
                gix::reference::edit::Error::FileTransactionCommit(
                    gix::refs::file::transaction::commit::Error::CreateOrUpdateRefLog(
                        gix::refs::file::log::create_or_update::Error::MissingCommitter,
                    ),
                ) => Some(gitbutler_error::error::Code::AuthorMissing),
                _ => None,
            };
            let err = anyhow::Error::from(err);
            if let Some(code) = code {
                err.context(code)
            } else {
                err
            }
        })?;
        // Important to first update the workspace so we have the correct stack setup.
        if let Some(ws_meta) = updated_ws_meta {
            meta.set_workspace(&ws_meta)?;
        } else if let Some(existing) = existing_ws_meta {
            // TODO: overwrite stored information with reality in new graph.
            meta.set_workspace(&existing)?;
        }

        // Always re-obtain the branch as `set_workspace` has created another version of it, possibly.
        // To avoid duplication, fetch the 'real' one and do the update again.
        // TODO: remove this in favor of keeping the previous handle once we have a sane `meta` impl
        let mut branch_md = meta.branch(ref_name)?;
        update_branch_metadata(ref_name, repo, &mut branch_md)?;
        meta.set_branch(&branch_md)?;

        Ok(graph_with_new_ref)
    }

    fn update_branch_metadata(
        ref_name: &gix::refs::FullNameRef,
        repo: &gix::Repository,
        md: &mut ref_metadata::Branch,
    ) -> anyhow::Result<()> {
        let is_new_ref = repo.try_find_reference(ref_name)?.is_none();
        md.update_times(is_new_ref);
        Ok(())
    }

    fn update_workspace_metadata(
        ws_meta: &mut ref_metadata::Workspace,
        new_ref: &gix::refs::FullNameRef,
        instruction: Instruction<'_>,
        new_stack_id: impl FnOnce(&gix::refs::FullNameRef) -> StackId,
    ) -> anyhow::Result<()> {
        if let Some((stack_idx, _)) =
            ws_meta.find_owner_indexes_by_name(new_ref, AppliedAndUnapplied)
        {
            // Just pretend its applied, and if it really is reachable, this will assure the
            // created ref name can be found.
            ws_meta.stacks[stack_idx].workspacecommit_relation = Merged;
            return Ok(());
        }
        match instruction {
            // Create new in known stack
            Instruction::DependentInStack(stack_id) => {
                ws_meta
                    .stacks
                    .iter_mut()
                    .find(|s| s.id == stack_id)
                    .with_context(|| {
                        format!(
                            "Couldn't find stack with id {stack_id} to place '{}' in",
                            new_ref.as_bstr()
                        )
                    })?
                    .branches
                    .push(WorkspaceStackBranch {
                        ref_name: new_ref.to_owned(),
                        archived: false,
                    });
            }
            // create new
            Instruction::Independent => ws_meta.stacks.push(WorkspaceStack {
                id: new_stack_id(new_ref),
                workspacecommit_relation: Merged,
                branches: vec![WorkspaceStackBranch {
                    ref_name: new_ref.to_owned(),
                    archived: false,
                }],
            }),
            // insert dependent branch at anchor
            Instruction::Dependent {
                ref_name: anchor_ref,
                position,
            } => {
                let (stack_idx, branch_idx) = ws_meta
                    .find_owner_indexes_by_name(anchor_ref.as_ref(), AppliedAndUnapplied)
                    .with_context(|| {
                        format!(
                            "Couldn't find anchor '{}' in workspace metadata - it's not consolidated",
                            anchor_ref.shorten()
                        )
                    })?;
                let stack = &mut ws_meta.stacks[stack_idx];
                // Just assure it's there, to facilitate the new branch actually shows up.
                stack.workspacecommit_relation = Merged;
                let branches = &mut stack.branches;
                branches.insert(
                    match position {
                        Position::Above => branch_idx,
                        Position::Below => branch_idx + 1,
                    },
                    WorkspaceStackBranch {
                        ref_name: new_ref.to_owned(),
                        archived: false,
                    },
                );
            }
        };
        Ok(())
    }

    /// Create the instruction that would be needed to insert the new ref-name into workspace data
    /// so that it represents the `position` of `anchor_id`.
    /// `position` indicates where, in relation to `anchor_id`, the ref name should be inserted.
    /// The first name that is also in `ws_meta` will be used.
    fn instruction_by_named_anchor_for_commit(
        ws: &but_graph::projection::Workspace<'_>,
        anchor_id: gix::ObjectId,
    ) -> anyhow::Result<Instruction<'static>> {
        use Position::*;
        let (anchor_stack_idx, anchor_seg_idx, _anchor_commit_idx) = ws
            .find_owner_indexes_by_commit_id(anchor_id)
            .with_context(|| {
                format!(
                    "No segment in workspace at '{}' that holds {anchor_id}",
                    ws.ref_name_display()
                )
            })?;

        let stack = &ws.stacks[anchor_stack_idx];
        // Find first non-empty segment in this stack upward and downward.
        let instruction = (0..anchor_seg_idx + 1)
            .rev()
            .find_map(|seg_idx| {
                let s = &stack.segments[seg_idx];
                s.ref_name
                    .as_ref()
                    .map(|rn| (rn.as_ref(), Below))
                    .filter(|_| s.metadata.is_some())
            })
            .or_else(|| {
                (anchor_seg_idx + 1..stack.segments.len()).find_map(|seg_idx| {
                    let s = &stack.segments[seg_idx];
                    s.ref_name
                        .as_ref()
                        .map(|rn| (rn.as_ref(), Above))
                        .filter(|_| s.metadata.is_some())
                })
            })
            .map(|(anchor_ref, position)| Instruction::Dependent {
                ref_name: Cow::Owned(anchor_ref.to_owned()),
                position,
            })
            .unwrap_or(
                // Not a single name? It's empty, or branch metadata is missing.
                // Create the first branch (then with metadata) directly.
                match stack.id {
                    None => Instruction::Independent,
                    Some(id) => Instruction::DependentInStack(id),
                },
            );
        Ok(instruction)
    }

    #[derive(Debug)]
    enum Instruction<'a> {
        Independent,
        DependentInStack(StackId),
        Dependent {
            ref_name: Cow<'a, gix::refs::FullNameRef>,
            position: Position,
        },
    }
}

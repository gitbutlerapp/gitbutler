use std::borrow::Cow;

use anyhow::Context as _;

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

impl<'a> From<&'a but_graph::workspace::StackCommit> for MinimalCommit<'a> {
    fn from(value: &'a but_graph::workspace::StackCommit) -> Self {
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
    /// Use another reference for positioning the new reference, which will point to the
    /// same commit as the reference named `ref_name`.
    /// Unlike [`Anchor::AtSegment`], `position` never affects which commit is used - it only
    /// determines the ordering of the two references.
    /// This requires a managed workspace, as only its metadata can order multiple references
    /// on the same commit.
    AtReference {
        /// The name of the reference to use as reference point for `position`.
        ref_name: Cow<'a, gix::refs::FullNameRef>,
        /// `Above` means the new reference will be right above `ref_name` as empty segment.
        /// `Below` means the new reference will be right below `ref_name`, taking ownership
        /// of its commits and leaving `ref_name` as empty segment.
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

    /// Create a new instance with a reference name as anchor.
    pub fn at_reference(ref_name: &'a gix::refs::FullNameRef, position: Position) -> Self {
        Anchor::AtReference {
            ref_name: Cow::Borrowed(ref_name),
            position,
        }
    }
}

pub(super) mod function {
    #![expect(clippy::indexing_slicing)]

    use std::borrow::{Borrow, Cow};

    use anyhow::{Context as _, bail};
    use bstr::ByteSlice;
    use but_core::{
        RefMetadata, ref_metadata,
        ref_metadata::{
            StackId, StackKind::AppliedAndUnapplied, WorkspaceCommitRelation::Merged,
            WorkspaceStack, WorkspaceStackBranch,
        },
    };
    use but_error::bail_precondition;
    use gix::refs::transaction::PreviousValue;

    use crate::branch::create_reference::{Anchor, Position};

    /// The resolved placement of a new reference, produced by matching on the [`Anchor`].
    ///
    /// Splitting this out keeps each match arm declarative: it states only the fields that differ
    /// from the common case (see [`AnchorResolution::positioned`]).
    struct AnchorResolution<'a> {
        /// The commit the new reference will point at.
        target_id: gix::ObjectId,
        /// Whether to assert `target_id` is inside the workspace before re-projecting.
        validate_in_workspace: bool,
        /// Workspace-metadata mutation to apply. Managed workspaces only; `None` in ad-hoc mode,
        /// where ordering lives in `branch_stack_order` instead.
        instruction: Option<Instruction<'a>>,
        /// Tip-to-base ad-hoc branch order to persist, when this placement changes it.
        branch_stack_order: Option<Vec<gix::refs::FullName>>,
        /// In ad-hoc mode, the reference that should become the projection entrypoint (the new
        /// tip). Set when creating a branch above the checked-out branch, so it is projected
        /// rather than rejected as unprojected. `HEAD` is *not* moved here; see
        /// [`create_reference`] — the caller is responsible for the checkout.
        new_tip: Option<gix::refs::FullName>,
    }

    impl<'a> AnchorResolution<'a> {
        /// A placement that merely points the new reference at `target_id`, with no ad-hoc branch
        /// order to persist and no change of entrypoint.
        fn positioned(
            target_id: gix::ObjectId,
            validate_in_workspace: bool,
            instruction: Option<Instruction<'a>>,
        ) -> Self {
            AnchorResolution {
                target_id,
                validate_in_workspace,
                instruction,
                branch_stack_order: None,
                new_tip: None,
            }
        }
    }

    /// Create a new reference named `ref_name` to point at a commit relative to `anchor`.
    /// If `anchor` is `None` this means the branch should be placed above the lower bound of the workspace, effectively
    /// creating an independent branch.
    /// The resulting reference will be created in `repo` and `meta` will be updated for `ref_name` so the workspace
    /// contains it, but only if it's a managed workspace, along with branch metadata.
    /// Use `new_stack_id` just with `Stack::generate()`, it's mainly used to be able to control the stack-id when needed in testing.
    /// The `order` parameter specifies where to insert a new independent stack (ignored for dependent branches).
    /// If `None`, appends to the end (using push). If `Some(n)`, inserts at position `n`.
    ///
    /// Fail if the reference already exists *and* points somewhere else.
    ///
    ///  - if there is no managed workspace, then dependent branches must be exclusive on each commit to identify ordering
    ///  - if there is a workspace, we store the order in workspace metadata and expect an `anchor` that names a segment.
    ///
    /// # Ad-hoc (single-branch) workspaces
    ///
    /// With [`Anchor::AtReference`] and no managed workspace, the new branch is positioned relative
    /// to a *local* branch by writing the tip-to-base order to `branch_stack_order`. This requires a
    /// backend where [`RefMetadata::can_persist_branch_stack_order`] is `true`.
    ///
    /// When the new branch is placed [`Position::Above`] the *currently checked-out* branch it
    /// becomes the new tip, and the returned workspace is projected **as if it were already checked
    /// out**. This function does *not* move `HEAD`; the caller is responsible for checking the new
    /// branch out so the real `HEAD` matches the projection (the API layer does this via
    /// checkout-after-create). Placing above a branch that is not the entrypoint leaves the
    /// entrypoint untouched.
    ///
    /// Return a regenerated Graph that contains the new reference, and from which a new workspace can be derived.
    pub fn create_reference<'ws, 'name, T: RefMetadata>(
        ref_name: impl Borrow<gix::refs::FullNameRef>,
        anchor: impl Into<Option<Anchor<'name>>>,
        repo: &gix::Repository,
        workspace: &'ws but_graph::Workspace,
        meta: &mut T,
        new_stack_id: impl FnOnce(&gix::refs::FullNameRef) -> StackId,
        order: impl Into<Option<usize>>,
    ) -> anyhow::Result<Cow<'ws, but_graph::Workspace>> {
        let anchor = anchor.into();
        let order = order.into();

        let ws_base = workspace.lower_bound;
        // Only update workspace metadata when the projection came from a persisted record.
        let existing_ws_meta = workspace
            .ref_name()
            .filter(|_| workspace.has_metadata())
            .map(|ws_ref| meta.workspace(ws_ref))
            .transpose()?;
        let ref_name = ref_name.borrow();
        let existing_ref_target_id = repo
            .try_find_reference(ref_name)?
            .map(|mut reference| reference.peel_to_id().map(|id| id.detach()))
            .transpose()?;
        let existing_ref_target_in_workspace = existing_ref_target_id
            .filter(|id| workspace.find_owner_indexes_by_commit_id(*id).is_some());

        let AnchorResolution {
            target_id: ref_target_id,
            validate_in_workspace: check_if_id_in_workspace,
            instruction,
            branch_stack_order,
            new_tip: ad_hoc_new_tip,
        } = match anchor {
            None => {
                // The new ref exists already in the workspace, do nothing.
                if workspace
                    .find_segment_and_stack_by_refname(ref_name)
                    .is_some()
                {
                    return Ok(Cow::Borrowed(workspace));
                }
                if let Some(existing_ref_target_id) = existing_ref_target_in_workspace {
                    let instruction = existing_ws_meta
                        .as_ref()
                        .map(|_| {
                            instruction_by_named_anchor_for_commit(
                                workspace,
                                existing_ref_target_id,
                            )
                        })
                        .transpose()?;
                    // Expect the target id to be in the workspace.
                    AnchorResolution::positioned(existing_ref_target_id, true, instruction)
                } else {
                    // The target tip (e.g. `origin/main`) can be advanced *past* the
                    // workspace, i.e. outside it. Anchoring a new independent branch there
                    // would stop re-projection from surfacing it as a standalone segment.
                    // Anchor at the merge-base of the target tip and the workspace commit
                    // instead — the fork point, always inside the workspace.
                    let target_tip = workspace
                        .resolved_target_commit_id()
                        .or(ws_base)
                        .with_context(|| {
                            format!(
                                "workspace at {} is missing a base",
                                workspace.ref_name_display()
                            )
                        })?;
                    // The merge-base needs the workspace commit. Without it (e.g. a headless,
                    // unmanaged workspace) there is no fork point and thus no insertion point
                    // inside the workspace, so refuse rather than silently anchor at the
                    // target tip — the very commit we know may sit outside the workspace.
                    let ws_commit_id = workspace
                        .ref_name()
                        .and_then(|ws_ref| repo.try_find_reference(ws_ref).ok().flatten())
                        .and_then(|mut ws_ref| ws_ref.peel_to_id().ok())
                        .map(|id| id.detach())
                        .with_context(|| {
                            format!(
                                "Cannot create independent branch: workspace at {} has no commit to anchor against",
                                workspace.ref_name_display()
                            )
                        })?;
                    let base = repo.merge_base(target_tip, ws_commit_id)?.detach();
                    // Don't validate: the merge-base is the workspace's lower bound (the
                    // fork point), not a commit owned by any segment.
                    AnchorResolution::positioned(base, false, Some(Instruction::Independent))
                }
            }
            Some(Anchor::AtCommit {
                commit_id,
                position,
            }) => {
                let mut validate_id = true;
                let indexes = workspace.try_find_owner_indexes_by_commit_id(commit_id)?;
                let ref_target_id =
                    position.resolve_commit(workspace.lookup_commit(indexes).into(), ws_base)?;
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

                AnchorResolution::positioned(ref_target_id, validate_id, instruction)
            }
            Some(Anchor::AtSegment { ref_name, position }) => {
                let mut validate_id = true;
                let ref_target_id = if workspace.has_metadata() {
                    let (stack_idx, seg_idx) =
                        workspace.try_find_segment_owner_indexes_by_refname(ref_name.as_ref())?;
                    let segment = &workspace.stacks[stack_idx].segments[seg_idx];

                    let id = workspace
                        .tip_commit_by_segment_id(segment.id)
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
                AnchorResolution::positioned(
                    ref_target_id,
                    validate_id,
                    Some(Instruction::Dependent { ref_name, position }),
                )
            }
            // Position relative to another *reference* on the same commit. Managed workspaces
            // order these in workspace metadata; ad-hoc workspaces record the order in the
            // `branch_order` table (see `resolve_ad_hoc_at_reference`).
            Some(Anchor::AtReference {
                ref_name: anchor_ref,
                position,
            }) => {
                if ref_name == anchor_ref.as_ref() {
                    bail_precondition!(
                        "Cannot position '{new}' relative to itself",
                        new = ref_name.shorten()
                    );
                }
                if workspace.has_metadata() {
                    let (stack_idx, seg_idx) =
                        workspace.try_find_segment_owner_indexes_by_refname(anchor_ref.as_ref())?;
                    let segment = &workspace.stacks[stack_idx].segments[seg_idx];
                    let ref_target_id = workspace
                        .tip_commit_by_segment_id(segment.id)
                        .map(|commit| commit.id)
                        .context(
                            "BUG: we should always see through to the base or eligible commits",
                        )?;
                    AnchorResolution::positioned(
                        ref_target_id,
                        Some(ref_target_id) != ws_base,
                        Some(Instruction::Dependent {
                            ref_name: anchor_ref,
                            position,
                        }),
                    )
                } else if anchor_ref.category() == Some(gix::refs::Category::LocalBranch) {
                    resolve_ad_hoc_at_reference(
                        ref_name,
                        anchor_ref.as_ref(),
                        position,
                        repo,
                        workspace,
                        meta,
                    )?
                } else {
                    bail_precondition!(
                        "Cannot position '{new}' relative to non-local reference '{anchor}' without a managed workspace",
                        new = ref_name.shorten(),
                        anchor = anchor_ref.shorten()
                    );
                }
            }
        };

        let updated_ws_meta = existing_ws_meta
            .zip(instruction)
            .map(|(mut existing, instruction)| {
                update_workspace_metadata(&mut existing, ref_name, instruction, new_stack_id, order)
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

            let mut overlay = but_graph::init::Overlay::default()
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
                );
            if let Some(branch_stack_order) = branch_stack_order.clone() {
                overlay = overlay.with_branch_stack_order_override(branch_stack_order);
            }
            if let Some(new_tip) = ad_hoc_new_tip {
                overlay = overlay.with_entrypoint(ref_target_id, Some(new_tip));
            }

            workspace
                .graph
                .redo_traversal_with_overlay(repo, meta, overlay)?
        };

        let updated_workspace = graph_with_new_ref.into_workspace()?;
        let has_new_ref_as_standalone_segment = updated_workspace
            .find_segment_and_stack_by_refname(ref_name)
            .is_some();
        let existing_ref_is_in_workspace = existing_ref_target_in_workspace.is_some();
        if !has_new_ref_as_standalone_segment {
            if existing_ref_target_id.is_some()
                && !existing_ref_is_in_workspace
                && !workspace.refname_is_segment(ref_name)
                && workspace.ref_name() != Some(ref_name)
            {
                bail_precondition!(
                    "Reference '{}' already exists and is outside the workspace",
                    ref_name.shorten()
                );
            }
            bail!(
                "Branch '{}' cannot be created: the target commit ({}) already \
                 belongs to another branch in the workspace. Each commit can only \
                 belong to one branch at a time.",
                ref_name.shorten(),
                ref_target_id,
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
            if is_not_a_directory_ref_edit_error(&err)
                && let Ok(Some(colliding_ref)) = find_colliding_ref_ancestor(repo, ref_name)
            {
                return anyhow::anyhow!(
                    "Branch name '{}' collides with existing branch '{}'",
                    ref_name.shorten(),
                    colliding_ref.shorten()
                );
            }
            let code = match err {
                gix::reference::edit::Error::FileTransactionCommit(
                    gix::refs::file::transaction::commit::Error::CreateOrUpdateRefLog(
                        gix::refs::file::log::create_or_update::Error::MissingCommitter,
                    ),
                ) => Some(but_error::Code::AuthorMissing),
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
        }
        if let Some(branch_stack_order) = branch_stack_order
            && let Err(err) = meta.set_branch_stack_order(&branch_stack_order)
        {
            // Keep the operation atomic from the caller's perspective: if we just created the ref
            // but can't persist its ordering, roll the ref back (best-effort) so we don't leave an
            // unordered same-commit branch that can't be projected consistently.
            if existing_ref_target_id.is_none()
                && let Ok(Some(reference)) = repo.try_find_reference(ref_name)
            {
                reference.delete().ok();
            }
            return Err(err);
        }

        // Always re-obtain the branch as `set_workspace` has created another version of it, possibly.
        // To avoid duplication, fetch the 'real' one and do the update again.
        // TODO: remove this in favor of keeping the previous handle once we have a sane `meta` impl
        let mut branch_md = meta.branch(ref_name)?;
        update_branch_metadata(ref_name, repo, &mut branch_md)?;
        meta.set_branch(&branch_md)?;

        Ok(Cow::Owned(updated_workspace))
    }

    /// Resolve an [`Anchor::AtReference`] in an ad-hoc (single-branch) workspace against a local
    /// `anchor_ref`, recording the tip-to-base order in `branch_stack_order`.
    ///
    /// The new reference points at the same commit as `anchor_ref`. See [`create_reference`] for
    /// the `new_tip` / checkout contract.
    fn resolve_ad_hoc_at_reference<'a, T: RefMetadata>(
        new_ref: &gix::refs::FullNameRef,
        anchor_ref: &gix::refs::FullNameRef,
        position: Position,
        repo: &gix::Repository,
        workspace: &but_graph::Workspace,
        meta: &T,
    ) -> anyhow::Result<AnchorResolution<'a>> {
        // Callers (the `AtReference` arm) guarantee `new_ref != anchor_ref`, which keeps the
        // `insert_into_branch_stack_order` invariant (the anchor survives `retain`) sound.
        if !meta.can_persist_branch_stack_order() {
            bail_precondition!(
                "Cannot position '{new}' relative to local reference '{anchor}' without branch order metadata",
                new = new_ref.shorten(),
                anchor = anchor_ref.shorten()
            );
        }
        let Some(mut anchor_reference) = repo.try_find_reference(anchor_ref)? else {
            bail_precondition!(
                "Cannot position '{new}' relative to '{anchor}': the anchor reference does not exist",
                new = new_ref.shorten(),
                anchor = anchor_ref.shorten()
            );
        };
        let target_id = anchor_reference.peel_to_id()?.detach();
        let existing_order = meta.branch_stack_order(anchor_ref)?.unwrap_or_default();
        let branch_stack_order =
            insert_into_branch_stack_order(existing_order, anchor_ref, new_ref, position);

        // Creating a branch above the checked-out branch makes it the new tip. The workspace only
        // projects the entrypoint and the segments below it, so without moving the entrypoint the
        // new ref would sit above it, fall outside the projection, and be rejected as unprojected.
        // Anchor the validating re-traversal at the new tip instead; the caller checks it out to
        // make this real (mirroring the API's checkout-after-create).
        let new_tip = (matches!(position, Position::Above)
            && workspace.ref_name() == Some(anchor_ref))
        .then(|| new_ref.to_owned());

        Ok(AnchorResolution {
            target_id,
            validate_in_workspace: false,
            instruction: None,
            branch_stack_order: Some(branch_stack_order),
            new_tip,
        })
    }

    /// Insert `new_ref` into the ad-hoc, tip-to-base `order` relative to `anchor`.
    ///
    /// - `anchor` is seeded into the order if it isn't there yet (a first ordering only has the
    ///   anchor).
    /// - If `new_ref` is already present it is moved (removed then re-inserted), so re-creating or
    ///   reordering the same branch is idempotent.
    /// - [`Position::Above`] takes the anchor's slot, pushing the anchor down; [`Position::Below`]
    ///   goes right after the anchor. This only affects *ordering* — unlike [`Anchor::AtSegment`],
    ///   it never changes which branch owns the commit.
    fn insert_into_branch_stack_order(
        mut order: Vec<gix::refs::FullName>,
        anchor: &gix::refs::FullNameRef,
        new_ref: &gix::refs::FullNameRef,
        position: Position,
    ) -> Vec<gix::refs::FullName> {
        if !order.iter().any(|branch| branch.as_ref() == anchor) {
            order.push(anchor.to_owned());
        }
        order.retain(|branch| branch.as_ref() != new_ref);
        // `anchor` was pushed above if it was missing, and `retain` only drops `new_ref`, so the
        // anchor is guaranteed to still be present here.
        let anchor_idx = order
            .iter()
            .position(|branch| branch.as_ref() == anchor)
            .expect("anchor is always present in the order at this point");
        let insert_idx = match position {
            Position::Above => anchor_idx,
            Position::Below => anchor_idx + 1,
        };
        order.insert(insert_idx, new_ref.to_owned());
        order
    }

    fn is_not_a_directory_ref_edit_error(err: &gix::reference::edit::Error) -> bool {
        matches!(
            err,
            gix::reference::edit::Error::FileTransactionPrepare(
                gix::refs::file::transaction::prepare::Error::Io(io_err)
            ) if io_err.kind() == std::io::ErrorKind::NotADirectory
        )
    }

    fn find_colliding_ref_ancestor(
        repo: &gix::Repository,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<gix::refs::FullName>> {
        let full_name = ref_name.as_bstr().to_str_lossy();
        // Skip the `refs` and `heads` separators so candidates start at branch components.
        for slash_idx in full_name.match_indices('/').map(|(idx, _)| idx).skip(2) {
            let ancestor_ref = gix::refs::FullName::try_from(full_name[..slash_idx].to_owned())?;
            if repo.try_find_reference(ancestor_ref.as_ref())?.is_some() {
                return Ok(Some(ancestor_ref));
            }
        }
        Ok(None)
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
        order: Option<usize>,
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
            Instruction::Independent => {
                let new_stack = WorkspaceStack {
                    id: new_stack_id(new_ref),
                    workspacecommit_relation: Merged,
                    branches: vec![WorkspaceStackBranch {
                        ref_name: new_ref.to_owned(),
                        archived: false,
                    }],
                };

                match order {
                    None => ws_meta.stacks.push(new_stack),
                    Some(index) => {
                        let insertion_index = index.min(ws_meta.stacks.len());
                        ws_meta.stacks.insert(insertion_index, new_stack);
                    }
                }
            }
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
        ws: &but_graph::Workspace,
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
                s.ref_name()
                    .map(|rn| (rn, Below))
                    .filter(|_| s.metadata.is_some())
            })
            .or_else(|| {
                (anchor_seg_idx + 1..stack.segments.len()).find_map(|seg_idx| {
                    let s = &stack.segments[seg_idx];
                    s.ref_name()
                        .map(|rn| (rn, Above))
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

    #[cfg(test)]
    mod tests {
        use super::{Position, insert_into_branch_stack_order};

        fn full(name: &str) -> gix::refs::FullName {
            gix::refs::FullName::try_from(name).expect("valid ref name")
        }

        fn names(order: &[gix::refs::FullName]) -> Vec<String> {
            order.iter().map(|r| r.as_bstr().to_string()).collect()
        }

        #[test]
        fn above_on_empty_order_seeds_the_anchor_below_the_new_ref() {
            let order = insert_into_branch_stack_order(
                Vec::new(),
                full("refs/heads/main").as_ref(),
                full("refs/heads/top").as_ref(),
                Position::Above,
            );
            assert_eq!(names(&order), ["refs/heads/top", "refs/heads/main"]);
        }

        #[test]
        fn below_on_empty_order_seeds_the_anchor_above_the_new_ref() {
            let order = insert_into_branch_stack_order(
                Vec::new(),
                full("refs/heads/main").as_ref(),
                full("refs/heads/bottom").as_ref(),
                Position::Below,
            );
            assert_eq!(names(&order), ["refs/heads/main", "refs/heads/bottom"]);
        }

        #[test]
        fn above_takes_the_anchor_slot_in_an_existing_order() {
            let order = insert_into_branch_stack_order(
                vec![full("refs/heads/top"), full("refs/heads/main")],
                full("refs/heads/main").as_ref(),
                full("refs/heads/middle").as_ref(),
                Position::Above,
            );
            assert_eq!(
                names(&order),
                ["refs/heads/top", "refs/heads/middle", "refs/heads/main"]
            );
        }

        #[test]
        fn reinserting_an_existing_ref_moves_it_and_is_idempotent() {
            // `top` already sits above `main`; re-inserting it below `main` moves it there.
            let order = insert_into_branch_stack_order(
                vec![full("refs/heads/top"), full("refs/heads/main")],
                full("refs/heads/main").as_ref(),
                full("refs/heads/top").as_ref(),
                Position::Below,
            );
            assert_eq!(names(&order), ["refs/heads/main", "refs/heads/top"]);

            // Applying the same insertion again is a no-op.
            let order = insert_into_branch_stack_order(
                order,
                full("refs/heads/main").as_ref(),
                full("refs/heads/top").as_ref(),
                Position::Below,
            );
            assert_eq!(names(&order), ["refs/heads/main", "refs/heads/top"]);
        }
    }
}

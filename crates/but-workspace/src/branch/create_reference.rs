use std::borrow::Cow;

/// For use in [`Anchor`].
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Position {
    /// The new dependent branch will appear above its anchor.
    Above,
    /// The new dependent branch will appear below its anchor.
    Below,
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
    use anyhow::Context as _;
    use but_core::{
        RefMetadata,
        branch::{find_unique_refname_excluding, refuse_colliding_refname},
    };
    use but_error::bail_precondition;
    use but_rebase::graph_rebase::{
        Editor, LookupStep, Selector, Step, SuccessfulRebase, mutate::InsertSide,
    };

    use crate::{
        branch::create_reference::{Anchor, Position},
        editor_workspace::ensure_workspace_commit,
    };

    enum Placement {
        Independent { base: gix::ObjectId },
        Relative { target: Selector, side: InsertSide },
    }

    /// Insert a new reference named `ref_name` into the step graph of `editor` relative to `anchor`,
    /// or as a new independent branch on the workspace base if `anchor` is `None`.
    ///
    /// An independent branch becomes a parent of the workspace commit at `order`, or its last parent
    /// if `order` is `None`, and can't be created outside of a workspace. A workspace commit is
    /// created if `HEAD` is on the workspace reference without one.
    ///
    /// Fail if `ref_name` is already taken, in the repository or in the editor.
    pub fn create_reference<'ws, 'meta, 'anchor, M: RefMetadata>(
        editor: Editor<'ws, 'meta, M>,
        ref_name: &gix::refs::FullNameRef,
        anchor: impl Into<Option<Anchor<'anchor>>>,
        order: impl Into<Option<usize>>,
    ) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
        ensure_name_is_available(&editor, ref_name)?;
        if editor.repo().head()?.is_unborn() {
            bail_precondition!("Cannot create reference on unborn branch");
        }
        let rebase = editor.rebase()?;
        let workspace = rebase.overlayed_graph()?.into_workspace()?;
        let mut editor = rebase.into_editor();
        let placement = placement(&editor, &workspace, anchor.into())?;
        let reference = Step::new_reference(ref_name.to_owned());
        match placement {
            Placement::Independent { base } => {
                let Some(workspace_reference) = editor.workspace_reference() else {
                    bail_precondition!(
                        "Cannot create independent branch '{}' outside of a workspace",
                        ref_name.shorten()
                    );
                };
                let (successful_rebase, workspace_commit) =
                    ensure_workspace_commit(editor, workspace_reference)?;
                editor = successful_rebase.into_editor();
                if workspace.stacks.is_empty() {
                    for (placeholder, _) in editor.direct_parents(workspace_commit)? {
                        editor.remove_edges(workspace_commit, placeholder)?;
                    }
                }
                let reference = editor.add_step(reference)?;
                editor.add_edge(reference, base, 0)?;
                editor.insert_parent(workspace_commit, reference, order.into())?;
            }
            Placement::Relative { target, side } => {
                editor.insert(target, reference, side)?;
            }
        }
        editor.rebase()
    }

    pub(in crate::branch) fn ensure_name_is_available<M: RefMetadata>(
        editor: &Editor<'_, '_, M>,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<()> {
        let available = find_unique_refname_excluding(editor.repo(), ref_name, |name| {
            editor.try_select_reference(name).is_some()
        })?;
        if available.as_ref() != ref_name {
            bail_precondition!("Reference '{}' already exists", ref_name.shorten());
        }
        refuse_colliding_refname(editor.repo(), ref_name, None)
    }

    fn placement<M: RefMetadata>(
        editor: &Editor<'_, '_, M>,
        workspace: &but_graph::Workspace,
        anchor: Option<Anchor<'_>>,
    ) -> anyhow::Result<Placement> {
        let independent = || {
            let base = workspace.highest_base().with_context(|| {
                format!(
                    "workspace at {} is missing a base",
                    workspace.ref_name_display()
                )
            })?;
            anyhow::Ok(Placement::Independent { base })
        };
        let placement = match anchor {
            None => independent()?,
            Some(Anchor::AtCommit { commit_id, .. })
                if Some(commit_id) == workspace.lower_bound =>
            {
                independent()?
            }
            Some(Anchor::AtCommit {
                commit_id,
                position,
            }) => {
                let Some(target) = editor.try_select_commit(commit_id) else {
                    bail_precondition!("Commit {commit_id} isn't part of the workspace");
                };
                Placement::Relative {
                    target,
                    side: side(position),
                }
            }
            Some(
                Anchor::AtReference { ref_name, position }
                | Anchor::AtSegment {
                    ref_name,
                    position: position @ Position::Above,
                },
            ) => Placement::Relative {
                target: select_anchor(editor, &ref_name)?,
                side: side(position),
            },
            Some(Anchor::AtSegment {
                ref_name,
                position: Position::Below,
            }) => {
                let anchor = select_anchor(editor, &ref_name)?;
                let oldest_commit = workspace
                    .try_find_segment_by_refname(&ref_name)?
                    .commits
                    .last()
                    .map(|commit| commit.id);
                Placement::Relative {
                    target: match oldest_commit {
                        Some(commit) => editor.select_commit(commit)?,
                        None => anchor,
                    },
                    side: InsertSide::Below,
                }
            }
        };
        if let Placement::Relative { target, side } = &placement {
            ensure_commit_below(editor, *target, *side)?;
        }
        Ok(placement)
    }

    fn select_anchor<M: RefMetadata>(
        editor: &Editor<'_, '_, M>,
        anchor: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Selector> {
        match editor.try_select_reference(anchor) {
            Some(selector) => Ok(selector),
            None => bail_precondition!(
                "The anchor reference '{}' isn't part of the workspace",
                anchor.shorten()
            ),
        }
    }

    fn ensure_commit_below<M: RefMetadata>(
        editor: &Editor<'_, '_, M>,
        target: Selector,
        side: InsertSide,
    ) -> anyhow::Result<()> {
        if matches!(side, InsertSide::Below) && editor.direct_parents(target)?.is_empty() {
            let id = editor.lookup_pick(target)?;
            match editor.find_commit(id)?.inner.parents.first() {
                Some(parent) => bail_precondition!("Commit {parent} isn't part of the workspace"),
                None => bail_precondition!(
                    "Commit {id} is the first in history and no branch can point below it"
                ),
            }
        }
        Ok(())
    }

    fn side(position: Position) -> InsertSide {
        match position {
            Position::Above => InsertSide::Above,
            Position::Below => InsertSide::Below,
        }
    }
}

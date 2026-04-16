//! Integrating upstream changes
//!
//! We have two pointers that we care about:
//! - target branch (origin/master)
//! - workspace tip (persisted in metadata/database)
//!
//! ---
//!
//! # Forkpoints vs Merge bases
//!
//!

use anyhow::{Result, bail};

use but_core::RefMetadata;
use but_graph::projection::StackCommitFlags;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, GraphEditorOptions, Step, SuccessfulRebase, ToSelector,
        mutate::{InsertSide, RelativeTo, SegmentDelimiter, SelectorSet, SomeSelectors},
    },
};

/// Whether a bottom most commit should be rebased, or a merge commit should be
/// created at the top of the commit run.
#[derive(Clone, Copy, PartialEq)]
pub enum BottomUpdateKind {
    /// Rebase the selected bottom-most commit onto the target branch.
    Rebase,
    /// Create a merge commit at the top of the selected stack.
    Merge,
}

/// Describes a particular bottom node and how it should be updated.
pub struct BottomUpdate {
    /// Describes how the associated branch should be updated.
    pub kind: BottomUpdateKind,
    /// A pointer to one of the bottom most commits in a stack.
    pub selector: RelativeTo,
}

enum UpdateTarget {
    Rebase {
        selector: RelativeTo,
    },
    Merge {
        /// The top of the branch which we'll either place the commit under if
        /// it's a reference, or on top of, if the branch has no top reference
        top: RelativeTo,
    },
}

/// The outcome of integrating upstream
pub struct IntegrateUpstreamOutcome<'ws, 'meta, M: RefMetadata> {
    /// The updated worskpace metadata.
    pub ws_meta: but_core::ref_metadata::Workspace,
    /// The rebased outcome.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
}

/// Is friggin good man!
pub fn integrate_upstream<'ws, 'meta, M: RefMetadata>(
    workspace: &'ws mut but_graph::projection::Workspace,
    meta: &'meta mut M,
    repo: &gix::Repository,
    updates: Vec<BottomUpdate>,
) -> Result<IntegrateUpstreamOutcome<'ws, 'meta, M>> {
    let Some(mut ws_meta) = workspace.metadata.clone() else {
        bail!("Cannot update a workspace with no metadata");
    };

    let updates = resolve_update_targets(workspace, updates)?;

    let Some(target_ref) = workspace.target_ref.clone() else {
        bail!("Cannot update a workspace with no target ref");
    };
    let target_ref_commit = repo
        .find_reference(target_ref.ref_name.as_ref())?
        .id()
        .detach();
    // TODO(CTO): This does not use the same integration check as the old
    // algorithm.
    let integrated_commits = workspace
        .stacks
        .iter()
        .flat_map(|s| &s.segments)
        .flat_map(|s| &s.commits)
        .filter_map(|commit| {
            if commit.flags.contains(StackCommitFlags::Integrated) {
                Some(commit.id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let editor_options = GraphEditorOptions {
        extra_refs: vec![target_ref.ref_name.as_ref()],
        ..GraphEditorOptions::default()
    };
    let mut editor = Editor::create_with_opts(workspace, meta, repo, &editor_options)?;
    let target_ref_selector = editor.select_reference(target_ref.ref_name.as_ref())?;

    for update in updates {
        match update {
            UpdateTarget::Rebase { selector } => {
                let selector = selector.to_selector(&editor)?;
                let replacement_order = if let Some((first_parent, order)) = editor
                    .direct_parents(selector)?
                    .into_iter()
                    .min_by_key(|(_, order)| *order)
                {
                    editor.disconnect_segment_from(
                        SegmentDelimiter {
                            child: selector,
                            parent: selector,
                        },
                        SelectorSet::None,
                        SelectorSet::Some(SomeSelectors::new(vec![first_parent])?),
                        false,
                    )?;
                    order
                } else {
                    0
                };
                editor.add_edge(selector, target_ref_selector, replacement_order)?;
            }
            UpdateTarget::Merge { top, .. } => {
                let mut commit = editor.empty_commit()?;
                let insert_side;
                match &top {
                    RelativeTo::Reference(branch) => {
                        commit.message =
                            format!("Merge {} into {}", target_ref.ref_name, branch).into();
                        insert_side = InsertSide::Below;
                    }
                    RelativeTo::Commit(_) => {
                        commit.message = format!("Merge {}", target_ref.ref_name).into();
                        insert_side = InsertSide::Above;
                    }
                }
                let merge_commit = editor.new_commit(commit, DateMode::CommitterKeepAuthorKeep)?;
                let merge_selector =
                    editor.insert(top, Step::new_pick(merge_commit), insert_side)?;

                let next_parent_order = editor
                    .direct_parents(merge_selector)?
                    .into_iter()
                    .map(|(_, order)| order)
                    .max()
                    .map_or(0, |order| order + 1);
                editor.add_edge(merge_selector, target_ref_selector, next_parent_order)?;

                // let Step::Pick(mut pick) = editor.lookup_step(merge_selector)? else {
                //     bail!("Expected inserted merge commit to be selectable as a pick");
                // };
                // pick.pick_mode = PickMode::Force;
                // editor.replace(merge_selector, Step::Pick(pick))?;
            }
        }
    }

    for commit in integrated_commits {
        let Some(selector) = editor.try_select_commit(commit) else {
            continue;
        };
        editor.replace(selector, Step::None)?;
    }

    ws_meta.target_commit_id = Some(target_ref_commit);
    Ok(IntegrateUpstreamOutcome {
        ws_meta,
        rebase: editor.rebase()?,
    })
}

fn resolve_update_targets(
    workspace: &'_ but_graph::projection::Workspace,
    updates: Vec<BottomUpdate>,
) -> Result<Vec<UpdateTarget>> {
    updates
        .into_iter()
        .map(|update| {
            let selector = update.selector;
            let targets = workspace
                .stacks
                .iter()
                .filter(|stack| stack_matches_bottom(stack, &selector))
                .collect::<Vec<_>>();

            if targets.is_empty() {
                match &selector {
                    RelativeTo::Commit(id) => bail!("Failed to discover desired bottom {id}"),
                    RelativeTo::Reference(reference) => {
                        bail!("Failed to discover desired bottom {reference}")
                    }
                }
            }

            Ok(match update.kind {
                BottomUpdateKind::Rebase => UpdateTarget::Rebase { selector },
                BottomUpdateKind::Merge => UpdateTarget::Merge {
                    top: merge_top_selector(targets.as_slice())?,
                },
            })
        })
        .collect()
}

fn merge_top_selector(targets: &[&but_graph::projection::Stack]) -> Result<RelativeTo> {
    let [target] = targets else {
        bail!("Merge updates require exactly one matching single-segment stack");
    };
    if target.segments.len() != 1 {
        bail!("Merge updates require exactly one matching single-segment stack");
    }

    if let Some(commit) = target.tip() {
        return Ok(RelativeTo::Commit(commit));
    }
    if let Some(reference) = target.ref_name() {
        return Ok(RelativeTo::Reference(reference.to_owned()));
    }

    bail!("Merge updates require a stack with a top reference or commit");
}

fn stack_matches_bottom(stack: &but_graph::projection::Stack, selector: &RelativeTo) -> bool {
    match selector {
        RelativeTo::Commit(id) => {
            stack
                .segments
                .last()
                .and_then(|segment| segment.commits.last())
                .map(|commit| commit.id)
                == Some(*id)
        }
        RelativeTo::Reference(reference) => {
            stack.segments.last().and_then(|segment| {
                if segment.commits.is_empty() {
                    segment.ref_name()
                } else {
                    None
                }
            }) == Some(reference.as_ref())
        }
    }
}

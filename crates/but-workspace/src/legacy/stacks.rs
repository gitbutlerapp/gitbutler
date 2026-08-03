//! Functions related to retrieving stack information.

use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, bail};
use but_core::{
    RefMetadata,
    ref_metadata::{ProjectMeta, StackId, StackKind, Workspace},
};
use gix::date::parse::TimeBuf;
use tracing::instrument;

use crate::{
    RefInfo, branch, head_info,
    legacy::{
        StacksFilter,
        ui::{StackEntry, StackHeadInfo},
    },
    ref_info,
    ref_info::Segment,
    ui,
    ui::StackDetails,
};

fn default_workspace_metadata(meta: &impl RefMetadata) -> anyhow::Result<Option<Workspace>> {
    ref_info::workspace_data_of_default_workspace_branch(meta)
}

/// Build a lookup from workspace branch ref names to their stable stack IDs.
///
/// The mapping covers both applied and unapplied stacks from the default workspace metadata so
/// callers can attach a V3 [`StackId`] to branch-derived UI entries without reaching into legacy
/// TOML-backed metadata.
fn stack_ids_by_ref_name(
    meta: &impl RefMetadata,
) -> anyhow::Result<HashMap<gix::refs::FullName, StackId>> {
    let Some(workspace) = default_workspace_metadata(meta)? else {
        return Ok(HashMap::new());
    };
    Ok(workspace
        .stacks(StackKind::AppliedAndUnapplied)
        .flat_map(|stack| {
            stack
                .branches
                .iter()
                .map(move |branch| (branch.ref_name.clone(), stack.id))
        })
        .collect())
}

/// Build a reverse lookup from stable stack IDs to the branch refs that currently represent them.
///
/// Each entry contains every branch ref recorded for the stack in default workspace metadata. This
/// allows callers to find a surviving repository ref for a stack before asking `ref_info()` to
/// reconstruct the current workspace projection for that stack.
fn branch_names_by_stack_id(
    meta: &impl RefMetadata,
) -> anyhow::Result<HashMap<StackId, Vec<gix::refs::FullName>>> {
    let Some(workspace) = default_workspace_metadata(meta)? else {
        return Ok(HashMap::new());
    };
    Ok(workspace
        .stacks(StackKind::AppliedAndUnapplied)
        .map(|stack| {
            (
                stack.id,
                stack
                    .branches
                    .iter()
                    .map(|branch| branch.ref_name.clone())
                    .collect(),
            )
        })
        .collect())
}

/// Get the associated forge review information out of the metadata, if any.
fn review_id_from_meta(
    name: &gix::refs::FullNameRef,
    meta: &impl RefMetadata,
) -> anyhow::Result<Option<usize>> {
    let pull_request = meta
        .branch_opt(name)?
        .and_then(|ref_meta| ref_meta.review.pull_request);
    Ok(pull_request)
}

fn try_from_stack_v3(
    repo: &gix::Repository,
    stack: branch::Stack,
    stack_ids_by_ref_name: &HashMap<gix::refs::FullName, StackId>,
) -> anyhow::Result<StackEntry> {
    let name = stack
        .name()
        .context("Every V2/V3 stack has a name as long as it's in a gitbutler workspace")?
        .to_owned();
    let heads: Vec<_> = stack
        .segments
        .into_iter()
        .map(|segment| -> anyhow::Result<_> {
            let review_id = segment.metadata.and_then(|meta| meta.review.pull_request);
            let ref_name = segment
                .ref_info
                .context("This type can't represent this state and it shouldn't have to")?
                .ref_name;
            Ok(StackHeadInfo {
                tip: repo
                    .find_reference(ref_name.as_ref())
                    .ok()
                    .and_then(|r| r.try_id())
                    .map(|id| id.detach())
                    .unwrap_or(repo.object_hash().null()),
                review_id,
                name: ref_name.shorten().into(),
                is_checked_out: segment.is_entrypoint,
            })
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(StackEntry {
        id: stack_ids_by_ref_name.get(&name).copied(),
        tip: heads
            .first()
            .map(|h| h.tip)
            .unwrap_or(repo.object_hash().null()),
        is_checked_out: heads.iter().any(|h| h.is_checked_out),
        heads,
        order: None,
    })
}

/// Returns the list of stacks that pass `filter`, in unspecified order.
///
/// Use `repo` and `meta` to read branches data
/// Use `ref_name_override` to read from a specific ref instead of HEAD. Used in production by
/// `stacks_v3_from_ctx` to anchor queries to the workspace ref (during edit mode, HEAD points
/// elsewhere), and in tests to avoid needing multiple fixtures with different HEAD positions.
// TODO: See if the UI can migrate to `head_info()` or a variant of it so the information is only called once.
#[deprecated(
    note = "Use head_info() and the returned RefInfo instead. Callers that already have a Context should prefer ctx.workspace_* helpers."
)]
pub fn stacks_v3(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    project_meta: &ProjectMeta,
    traversal: but_graph::init::Options,
    filter: StacksFilter,
    ref_name_override: Option<&gix::refs::FullNameRef>,
) -> anyhow::Result<Vec<StackEntry>> {
    // TODO: See if this works at all once VirtualBranches.toml isn't the backing anymore.
    //       Probably needs to change, maybe even alongside the notion of 'unapplied'.
    //       In future, unapplied stacks could just be stacks, either with one segment, or multiple ones - any branch with another branch
    //       found while traversing its commits to some base becomes a stack in that very sense.
    fn unapplied_stacks(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        applied_stacks: &[branch::Stack],
        stack_ids_by_ref_name: &HashMap<gix::refs::FullName, StackId>,
    ) -> anyhow::Result<Vec<StackEntry>> {
        let mut out = Vec::new();
        for item in meta.iter() {
            let (ref_name, ref_meta) = item?;
            if !ref_meta.is::<but_core::ref_metadata::Branch>() {
                continue;
            };
            let is_applied = applied_stacks.iter().any(|stack| {
                stack.segments.iter().any(|segment| {
                    segment
                        .ref_info
                        .as_ref()
                        .is_some_and(|ri| ri.ref_name == ref_name)
                })
            });
            if is_applied {
                continue;
            }

            let Some(reference) = repo.try_find_reference(ref_name.as_ref())? else {
                continue;
            };
            let tip = reference
                .try_id()
                .with_context(|| format!("Encountered symbolic reference: {ref_name}"))?
                .detach();
            out.push(StackEntry {
                id: stack_ids_by_ref_name.get(&ref_name).copied(),
                // TODO: this is just a simulation and such a thing doesn't really exist in the V3 world, let's see how it goes.
                //       Thus, we just pass ourselves as first segment, similar to having no other segments.
                heads: vec![StackHeadInfo {
                    name: ref_name.shorten().into(),
                    tip,
                    review_id: review_id_from_meta(ref_name.as_ref(), meta)?,
                    is_checked_out: false,
                }],
                tip,
                order: None,
                is_checked_out: false,
            })
        }
        Ok(out)
    }

    let options = ref_info::Options {
        project_meta: project_meta.clone(),
        expensive_commit_info: false,
        traversal,
        ..Default::default()
    };
    let info = match ref_name_override {
        None => head_info(repo, meta, options),
        Some(ref_name) => ref_info(repo.find_reference(ref_name)?, meta, options),
    }?;
    let stack_ids_by_ref_name = stack_ids_by_ref_name(meta)?;

    fn into_ui_stacks(
        repo: &gix::Repository,
        stacks: Vec<branch::Stack>,
        stack_ids_by_ref_name: &HashMap<gix::refs::FullName, StackId>,
    ) -> Vec<StackEntry> {
        stacks
            .into_iter()
            .filter_map(|stack| try_from_stack_v3(repo, stack, stack_ids_by_ref_name).ok())
            .collect()
    }

    let mut stacks = match filter {
        StacksFilter::InWorkspace => into_ui_stacks(repo, info.stacks, &stack_ids_by_ref_name),
        StacksFilter::All => {
            let unapplied_stacks =
                unapplied_stacks(repo, meta, &info.stacks, &stack_ids_by_ref_name)?;
            let mut all_stacks = unapplied_stacks;
            all_stacks.extend(into_ui_stacks(repo, info.stacks, &stack_ids_by_ref_name));
            all_stacks
        }
        StacksFilter::Unapplied => {
            unapplied_stacks(repo, meta, &info.stacks, &stack_ids_by_ref_name)?
        }
    };

    let needs_filtering_to_hide_segments_not_checked_out = stacks
        .iter()
        .any(|s| s.is_checked_out || s.heads.iter().any(|h| h.is_checked_out));
    if needs_filtering_to_hide_segments_not_checked_out {
        stacks.retain(|s| s.is_checked_out);
        // Segments can be reachable from multiple tips, we keep only one
        stacks.truncate(1);
        let mut saw_checked_out = false;
        stacks
            .first_mut()
            .context("BUG: we should always have at least one stack")?
            .heads
            .retain(|h| {
                saw_checked_out |= h.is_checked_out;
                saw_checked_out
            });
    }

    Ok(stacks)
}

/// Get additional information for the stack identified by `stack_id`. If `None`, it's the first available stack
/// and we expect it to have no ID.
// TODO: StackId shouldn't be used, instead use the ref-name or stack index as universal tip identifier.
//       It's notable that there isn't always a ref-name available right now in case the ref advanced, but maybe this is something
//       we can pull out of the metadata information.
#[deprecated(
    note = "Use head_info() and the returned RefInfo instead. Callers that already have a Context should prefer ctx.workspace_* helpers."
)]
#[instrument(level = "debug", skip(meta), err(Debug))]
pub fn stack_details_v3(
    stack_id: Option<StackId>,
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    project_meta: &ProjectMeta,
    traversal: but_graph::init::Options,
) -> anyhow::Result<ui::StackDetails> {
    // Prefer the current `HEAD` projection if it can still see the requested stack, and only fall
    // back to resolving from a surviving ref when that stack is no longer reachable from `HEAD`.
    fn stack_by_id(head_info: RefInfo, stack_id: StackId) -> Option<branch::Stack> {
        head_info
            .stacks
            .into_iter()
            .find(|stack| stack.id == Some(stack_id))
    }
    fn new_ref_info_options(
        project_meta: &ProjectMeta,
        traversal: &but_graph::init::Options,
    ) -> ref_info::Options<'static> {
        ref_info::Options {
            project_meta: project_meta.clone(),
            expensive_commit_info: true,
            traversal: traversal.clone(),
            ..Default::default()
        }
    }
    let mut ref_info_options = new_ref_info_options(project_meta, &traversal);
    let mut stack = match stack_id {
        None => {
            // assume single-branch mode.
            // Make sure the UI isn't overwhelmed, this currently happens easily on some repos where a lot of commits
            // would otherwise be returned. The problem is that then the workspace might not be correct, but there isn't
            // another way that still allows to extend the range via gas-stations. Maybe one day we won't need this.
            ref_info_options.traversal.hard_limit = Some(500);
            let mut info = head_info(repo, meta, ref_info_options)?;
            if info.is_entrypoint {
                if info.stacks.len() != 1 {
                    bail!(
                        "BUG(opt-stack-id): should have gotten exactly one stack, got {}",
                        info.stacks.len()
                    );
                }
                info.stacks.pop().unwrap()
            } else {
                info.stacks
                    .iter()
                    .find(|stack| stack.segments.iter().any(|segment| segment.is_entrypoint))
                    .cloned()
                    .context("BUG: expected to find one segment with entrypoint")?
            }
        }
        Some(stack_id) => {
            if let Some(stack) = stack_by_id(
                head_info(repo, meta, new_ref_info_options(project_meta, &traversal))?,
                stack_id,
            ) {
                stack
            } else {
                let branch_names_by_stack_id = branch_names_by_stack_id(meta)?;
                let branch_names = branch_names_by_stack_id
                    .get(&stack_id)
                    .with_context(|| format!("Couldn't find {stack_id} in workspace metadata"))?;
                let existing_ref = branch_names
                    .iter()
                    .find_map(|ref_name| repo.find_reference(ref_name.as_ref()).ok())
                    .with_context(|| {
                        format!("Couldn't find any refs for stack {stack_id} in the repository")
                    })?;
                let ref_info = ref_info(
                    existing_ref,
                    meta,
                    new_ref_info_options(project_meta, &traversal),
                )?;
                stack_by_id(ref_info, stack_id).with_context(|| {
                    format!("Really couldn't find {stack_id} in the current workspace projection")
                })?
            }
        }
    };

    // This is more of a badly tested hack to quickly filter parts of a stack that aren't checked out.
    // Better to switch over to the new data-structured for proper handling of detached heads, and anonymous segments.
    if let Some(head_ref) = repo.head_ref()? {
        let needs_filtering_to_hide_segments_not_checked_out =
            stack.segments.iter().position(|s| {
                s.ref_info.as_ref().map(|ri| ri.ref_name.as_ref()) == Some(head_ref.name())
            });
        if let Some(stack_pos) = needs_filtering_to_hide_segments_not_checked_out {
            stack.segments.drain(..stack_pos);
        }
    } else if let Ok(head_id) = repo.head_id() {
        // For now, keep the whole segment, don't cut it down to the actual commit. This code should be thrown out,
        // and probably has to move to the frontend anyway if/when 'solo'-ing becomes a thing.
        let needs_filtering_to_hide_segments_and_commits_not_checked_out = stack
            .segments
            .iter()
            .position(|s| s.commits.iter().any(|c| c.id == head_id));
        if let Some(stack_pos) = needs_filtering_to_hide_segments_and_commits_not_checked_out {
            stack.segments.drain(..stack_pos);
            if let Some(segment) = stack.segments.first_mut() {
                let mut saw_commit = false;
                segment.commits.retain(|c| {
                    saw_commit |= c.id == head_id;
                    saw_commit
                })
            }
        }
    }
    let branch_details = stack
        .segments
        .iter()
        .map(ui::BranchDetails::from_segment)
        .collect::<Result<Vec<_>, _>>()?;

    let topmost_branch = branch_details
        .first()
        .context("Stacks should never be empty")?;
    Ok(StackDetails {
        derived_name: topmost_branch.name.to_string(),
        push_status: topmost_branch.push_status,
        is_conflicted: topmost_branch.is_conflicted,
        branch_details,
    })
}

impl ui::BranchDetails {
    fn from_segment(
        Segment {
            id: _,
            ref_info,
            commits: commits_unique_from_tip,
            commits_on_remote: commits_unique_in_remote_tracking_branch,
            remote_tracking_ref_name,
            remote_tracking_branch_segment_id: _,
            // There is nothing equivalent
            commits_outside,
            metadata,
            push_status,
            is_entrypoint: _,
            base,
        }: &Segment,
    ) -> anyhow::Result<Self> {
        let ref_info = ref_info
            .clone()
            .context("Can't handle a stack yet whose tip isn't pointed to by a ref")?;
        if let Some(commits_outside) = commits_outside
            .as_ref()
            .filter(|commits| !commits.is_empty())
        {
            tracing::warn!(
                ignored_outside_commits = commits_outside.len(),
                stack_segment_ref = %ref_info.ref_name,
                "Legacy StackDetails drops commits_outside for this stack segment"
            );
        }
        let (updated_at, review_id, pr_number) = metadata
            .clone()
            .map(|meta| {
                (
                    meta.ref_info.updated_at,
                    meta.review.review_id,
                    meta.review.pull_request,
                )
            })
            .unwrap_or_default();
        let base_commit = base.unwrap_or(gix::hash::Kind::Sha1.null());
        Ok(ui::BranchDetails {
            is_remote_head: ref_info
                .ref_name
                .category()
                .is_some_and(|c| matches!(c, gix::refs::Category::RemoteBranch)),
            name: ref_info.ref_name.shorten().into(),
            reference: ref_info.ref_name,
            linked_worktree_id: ref_info.worktree.and_then(|ws| match ws.kind {
                but_graph::WorktreeKind::Main => None,
                but_graph::WorktreeKind::LinkedId(id) => Some(id),
            }),
            remote_tracking_branch: remote_tracking_ref_name
                .as_ref()
                .map(|full_name| full_name.as_bstr().into()),
            pr_number,
            review_id,
            tip: commits_unique_from_tip
                .first()
                .map(|commit| commit.id)
                .unwrap_or(base_commit),
            base_commit,
            push_status: *push_status,
            last_updated_at: updated_at.map(|time| time.seconds as i128 * 1_000),
            authors: {
                let mut authors = HashSet::<ui::Author>::new();
                let all_commits = commits_unique_from_tip
                    .iter()
                    .map(|c| &c.inner)
                    .chain(commits_unique_in_remote_tracking_branch.iter());
                for commit in all_commits {
                    authors.insert((commit.author.to_ref(&mut TimeBuf::default())).into());
                }
                let mut authors: Vec<_> = authors.into_iter().collect();
                authors.sort_by(|a, b| a.name.cmp(&b.name));
                authors
            },
            commits: commits_unique_from_tip.iter().map(Into::into).collect(),
            is_conflicted: commits_unique_from_tip.iter().any(|c| c.has_conflicts),
            upstream_commits: commits_unique_in_remote_tracking_branch
                .iter()
                .map(Into::into)
                .collect(),
        })
    }
}

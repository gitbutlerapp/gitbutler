use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use anyhow::{Result, bail};
use but_core::{RefMetadata, commit::Headers};
use but_rebase::graph_rebase::{
    Editor, LookupStep, SuccessfulRebase, ToSelector,
    mutate::{SegmentDelimiter, SelectorSet},
};

use crate::graph_manipulation::{EdgeSelection, connect_segment_to_edges, selected_edges_from_set};
use crate::{
    branch::integrate_branch_upstream::plan::{
        editable_local_commits, initial_integration_steps, integration_steps_into_segment_nodes,
    },
    divergence::{
        BranchMergeBaseCommits, classify_selectors_against_target_ref, commit_ids_from_selectors,
        get_commits_until_merge_base,
    },
};
use crate::{graph_manipulation::determine_parent_selector, resolve_tracking_branch_ref_name};

mod display;
mod parsing;
mod plan;

pub use display::{
    IntegrationDivergenceCommit, IntegrationDivergenceDisplay, IntegrationDivergenceTargetRelation,
};
use display::{add_ref_label, divergence_commit, relation_for};
pub use parsing::{parse_integration_steps_script, render_integration_steps_script};
pub use plan::BranchIntegrationStrategy;
use plan::prepare_integration_steps_for_editor;
/// The steps to be followed when integrating upstream changes into the local one.
#[derive(Debug)]
pub enum InteractiveIntegrationStep {
    /// Pick a commit, keeping it in the branch.
    Pick {
        /// The SHA of the commit being picked.
        commit_id: gix::ObjectId,
    },
    /// Squash the commits into one.
    Squash {
        /// The SHAs of the commits to squash.
        commits: Vec<gix::ObjectId>,
        /// Optionally, the message to use for the squash commit.
        message: Option<String>,
    },
    /// Merge a commit into the previous one.
    Merge {
        /// The SHA of the commit to merge.
        commit_id: gix::ObjectId,
    },
}

impl fmt::Display for InteractiveIntegrationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pick { commit_id } => write!(f, "pick {commit_id}"),
            Self::Merge { commit_id } => write!(f, "merge {commit_id}"),
            Self::Squash { commits, message } => {
                write!(f, "squash")?;
                for commit_id in commits {
                    write!(f, " {commit_id}")?;
                }
                if let Some(message) = message {
                    write!(f, " | message={message:?}")?;
                }
                Ok(())
            }
        }
    }
}

/// The necessary information about the integration to be performed.
#[derive(Debug)]
pub struct InteractiveIntegration {
    /// The list of steps to follow in order to integrate the upstream changes into the local.
    pub steps: Vec<InteractiveIntegrationStep>,
    /// Merge base between the upstream and the local reference.
    pub merge_base: gix::ObjectId,
    /// The first parent-to-child local commit that is not historically integrated into target.
    pub first_local_not_integrated: Option<gix::ObjectId>,
}

impl fmt::Display for InteractiveIntegration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "merge-base {}", self.merge_base)?;
        for step in &self.steps {
            writeln!(f, "{step}")?;
        }
        Ok(())
    }
}

/// The initial integration proposal for a branch.
#[derive(Debug)]
pub struct InitialBranchIntegration {
    /// The editable execution plan for integrating the branch upstream.
    pub integration: InteractiveIntegration,
    /// The current divergence between local branch and upstream for display.
    pub divergence: IntegrationDivergenceDisplay,
}

/// Integrate the upstream changes in the order of the provided steps.
///
/// `ref_name` - The full reference name of the local branch we're integrating the upstream changes into.
///
/// `steps` - The vector of steps in the application order (parent to child) that describe the actions to perform
///   for the integration of the changes.
pub fn integrate_branch_with_steps<'ws, 'meta, M: RefMetadata>(
    ref_name: &gix::refs::FullNameRef,
    integration: InteractiveIntegration,
    workspace: &'ws mut but_graph::Workspace,
    meta: &'meta mut M,
    repo: &gix::Repository,
) -> Result<SuccessfulRebase<'ws, 'meta, M>> {
    if integration.steps.is_empty() {
        bail!("Integration steps cannot be empty")
    }
    // The editor maps every segment in the graph, including the remote
    // reference of the branch we're integrating.
    let mut editor = Editor::create(workspace, meta, repo)?;
    // Step 1: We prepare the steps before building.
    // At this point, we construct the commits for the squash steps in memory.
    let prepared_steps = prepare_integration_steps_for_editor(&editor, &integration.steps)?;

    let delimiter_child = editor.select_reference(ref_name)?;
    let delimiter_parent =
        if let Some(first_local_not_integrated_commit) = integration.first_local_not_integrated {
            editor.select_commit(first_local_not_integrated_commit)?
        } else {
            editor.select_reference(ref_name)?
        };
    // Segment, from local-ref to the parent-most non-integrated local commit.
    // This represents the bounds of the commit chain we're about to manipulate and rebuild.
    let segment_delimiter = SegmentDelimiter {
        child: delimiter_child,
        parent: delimiter_parent,
    };

    // Step 2: We determine which children and parents to disconnect from the segment above.
    // We keep track of them so that we can reconnect them after we've done the chain-rebuild.
    let children_to_disconnect = SelectorSet::All;
    let parents_to_disconnect = determine_parent_selector(&editor, delimiter_parent)?;

    let children_to_reconnect = selected_edges_from_set(
        &editor,
        segment_delimiter.child,
        &children_to_disconnect,
        EdgeSelection::Children,
    )?;
    let integration_commit_ids = integration_step_commit_ids(&integration.steps);
    let children_to_reconnect = children_to_reconnect
        .into_iter()
        .filter(|(selector, _)| match editor.lookup_pick(*selector) {
            Ok(commit_id) => !integration_commit_ids.contains(&commit_id),
            Err(_) => true,
        })
        .collect::<Vec<_>>();
    let parents_to_reconnect = selected_edges_from_set(
        &editor,
        segment_delimiter.parent,
        &parents_to_disconnect,
        EdgeSelection::Parents,
    )?
    .into_iter()
    .map(|(selector, order)| {
        if selector == delimiter_child {
            editor
                .select_commit(integration.merge_base)
                .map(|merge_base| (merge_base, order))
        } else {
            Ok((selector, order))
        }
    })
    .collect::<Result<Vec<_>>>()?;

    // Step 3: Disconnect the segment, isolating it so that we can freely manipulate it.
    editor.disconnect_segment_from(
        segment_delimiter,
        children_to_disconnect,
        parents_to_disconnect,
        true,
    )?;

    // Step 4: Based on the prepared steps, we rebuild the chain.
    let new_segment_delimiter =
        integration_steps_into_segment_nodes(&mut editor, ref_name, &prepared_steps)?;
    // Step 5: Once we have our new chain, we reconnect it to the original children and parents.
    connect_segment_to_edges(
        &mut editor,
        new_segment_delimiter,
        &children_to_reconnect,
        &parents_to_reconnect,
    )?;

    editor.rebase()
}

fn integration_step_commit_ids(steps: &[InteractiveIntegrationStep]) -> HashSet<gix::ObjectId> {
    let mut out = HashSet::new();
    for step in steps {
        match step {
            InteractiveIntegrationStep::Pick { commit_id }
            | InteractiveIntegrationStep::Merge { commit_id } => {
                out.insert(*commit_id);
            }
            InteractiveIntegrationStep::Squash { commits, .. } => {
                out.extend(commits.iter().copied());
            }
        }
    }
    out
}

/// Get the initial integration steps for a branch.
///
/// The returned steps are ordered for application from parent to child so they
/// can be passed directly to integration without reordering by the caller.
///
/// `ref_name` - The full reference name of the local branch to get the integration steps for.
///
/// `repo` - The repository handle.
///
/// `workspace` - The current workspace graph projection used to construct the editor.
///
/// `meta` - Reference metadata used while constructing the editor.
///
/// Returns the initial integration script and current divergence display state.
pub fn get_initial_integration_steps_for_branch<M: RefMetadata>(
    ref_name: &gix::refs::FullNameRef,
    strategy: BranchIntegrationStrategy,
    workspace: &mut but_graph::Workspace,
    meta: &mut M,
    repo: &gix::Repository,
) -> Result<InitialBranchIntegration> {
    // Step 1: We create the editor, which maps every segment in the graph -
    // including the remote branch to integrate and the project's target ref.
    let upstream_ref_name = resolve_tracking_branch_ref_name(ref_name, repo)?;
    let target_ref_name = workspace
        .target_ref
        .as_ref()
        .map(|target| target.ref_name.clone())
        .filter(|target_ref_name| target_ref_name.as_ref() != upstream_ref_name.as_ref());

    let editor = Editor::create(workspace, meta, repo)?;

    // Step 2: We traverse the editor graph and determine the divergence between the local and remote branch.
    let BranchMergeBaseCommits {
        local_commits: local_commit_selectors,
        upstream_commits: upstream_commit_selectors,
        merge_base: merge_base_selector,
    } = get_commits_until_merge_base(ref_name, upstream_ref_name.clone(), &editor)?;
    let local_commits = commit_ids_from_selectors(&editor, local_commit_selectors.iter().copied())?;
    let upstream_commits =
        commit_ids_from_selectors(&editor, upstream_commit_selectors.iter().copied())?;
    let merge_base = editor.lookup_pick(merge_base_selector)?;

    // Step 3: We determine the integration state of all the relevant commits, to know which are editable
    // and for display purposes.
    // All upstream commits are editable, regardless of integration.
    // Only the non-integrated local commits are editable.
    let candidate_selectors = local_commit_selectors
        .iter()
        .chain(upstream_commit_selectors.iter())
        .copied()
        .chain(std::iter::once(merge_base_selector))
        .collect::<Vec<_>>();
    let target_relations = target_ref_name
        .as_ref()
        .map(|target_ref_name| {
            let target_ref_selector = target_ref_name.as_ref().to_selector(&editor)?;
            classify_selectors_against_target_ref(
                &editor,
                target_ref_selector,
                &candidate_selectors,
            )
        })
        .transpose()?
        .unwrap_or_default();

    // Step 4: Build the initial set of integration steps.
    let change_ids = if matches!(strategy, BranchIntegrationStrategy::SmartSquash) {
        explicit_change_ids(
            repo,
            local_commits.iter().chain(upstream_commits.iter()).copied(),
        )?
    } else {
        HashMap::new()
    };
    let initial_steps = initial_integration_steps(
        strategy,
        &local_commits,
        &upstream_commits,
        &target_relations,
        &change_ids,
    );

    // Step 5: Build the return payload, including the integration steps recommended
    // and the divergence display information.

    // We turn the commits into divergence commits, which are just decorated with extra information,
    // for display purposes.
    let divergence_local_only = local_commits
        .iter()
        .copied()
        .map(|commit_id| {
            divergence_commit(repo, commit_id, relation_for(&target_relations, commit_id))
        })
        .collect::<Result<Vec<_>>>()?;
    let divergence_upstream_only = upstream_commits
        .iter()
        .copied()
        .map(|commit_id| {
            divergence_commit(repo, commit_id, relation_for(&target_relations, commit_id))
        })
        .collect::<Result<Vec<_>>>()?;

    let integration = InteractiveIntegration {
        steps: initial_steps,
        merge_base,
        first_local_not_integrated: editable_local_commits(&local_commits, &target_relations)
            .next(),
    };
    let mut divergence = IntegrationDivergenceDisplay {
        branch_ref_name: ref_name.to_owned(),
        upstream_ref_name: upstream_ref_name.into_owned(),
        local_only: divergence_local_only,
        upstream_only: divergence_upstream_only,
        merge_base: divergence_commit(
            repo,
            merge_base,
            relation_for(&target_relations, merge_base),
        )?,
    };
    let local_tip = divergence
        .local_only
        .first()
        .map(|commit| commit.id)
        .or(Some(merge_base));
    let upstream_tip = divergence
        .upstream_only
        .first()
        .map(|commit| commit.id)
        .or(Some(merge_base));
    add_ref_label(
        &mut divergence.local_only,
        &mut divergence.merge_base,
        local_tip,
        divergence.branch_ref_name.shorten().to_string(),
    );
    add_ref_label(
        &mut divergence.upstream_only,
        &mut divergence.merge_base,
        upstream_tip,
        divergence.upstream_ref_name.shorten().to_string(),
    );

    Ok(InitialBranchIntegration {
        integration,
        divergence,
    })
}

fn explicit_change_ids(
    repo: &gix::Repository,
    commit_ids: impl IntoIterator<Item = gix::ObjectId>,
) -> Result<HashMap<gix::ObjectId, but_core::ChangeId>> {
    commit_ids
        .into_iter()
        .map(|commit_id| {
            let raw_commit = repo.find_commit(commit_id)?;
            let commit = raw_commit.decode()?;
            let change_id = Headers::try_from_commit_headers(|| commit.extra_headers())
                .and_then(|headers| headers.change_id);
            Ok((commit_id, change_id))
        })
        .filter_map(|result| match result {
            Ok((commit_id, Some(change_id))) => Some(Ok((commit_id, change_id))),
            Ok((_, None)) => None,
            Err(err) => Some(Err(err)),
        })
        .collect()
}

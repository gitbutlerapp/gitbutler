use std::{
    collections::{HashMap, HashSet},
    fmt,
    str::FromStr,
};

use anyhow::{Context as _, Result, bail};
use bstr::{BStr, ByteSlice};
use but_core::{
    RefMetadata, RepositoryExt,
    commit::{add_conflict_markers, write_conflicted_tree},
};
use but_rebase::graph_rebase::{
    Editor, ExtraRef, GraphEditorOptions, LookupStep, Selector, Step, SuccessfulRebase, ToSelector,
    mutate::{SegmentDelimiter, SelectorSet},
};
use but_rebase::{commit::DateMode, graph_rebase::merge_commit_changes::MergeCommitChangesOutcome};
use gix::{prelude::ObjectIdExt as _, remote::Direction};

use crate::branch::segment_disconnect::determine_parent_selector;

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
        /// The SHA of the commit to squash.
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

/// The necessay information about the integration to be performed.
#[derive(Debug)]
pub struct InteractiveIntegration {
    /// The list of steps to follow in order to integrate the upstream changes into the local.
    pub steps: Vec<InteractiveIntegrationStep>,
    /// Merge base between the upstream and the local reference.
    pub merge_base: gix::ObjectId,
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

/// A single commit row in the divergence display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationDivergenceCommit {
    /// The commit shown in the graph row.
    pub id: gix::ObjectId,
    /// The first-line subject shown for the commit.
    pub subject: String,
    /// Human-facing ref labels rendered inline on the commit row.
    pub refs: Vec<String>,
}

/// Current branch/upstream divergence information for display purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationDivergenceDisplay {
    /// The local branch being integrated.
    pub branch_ref_name: gix::refs::FullName,
    /// The upstream branch this local branch integrates with.
    pub upstream_ref_name: gix::refs::FullName,
    /// Commits only reachable from the local branch tip down to the shared section.
    pub local_only: Vec<IntegrationDivergenceCommit>,
    /// Commits only reachable from the upstream branch tip down to the shared section.
    pub upstream_only: Vec<IntegrationDivergenceCommit>,
    /// Commits shared or matched between local and upstream above the merge-base.
    pub matched: Vec<IntegrationDivergenceCommit>,
    /// The merge-base row shown once at the bottom.
    pub merge_base: IntegrationDivergenceCommit,
}

impl fmt::Display for IntegrationDivergenceDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for commit in &self.local_only {
            writeln!(f, "{}", graph_commit_string("* ", commit))?;
        }
        for commit in &self.upstream_only {
            let prefix = if self.local_only.is_empty() {
                "* "
            } else {
                "| * "
            };
            writeln!(f, "{}", graph_commit_string(prefix, commit))?;
        }
        if !self.local_only.is_empty() && !self.upstream_only.is_empty() {
            writeln!(f, "|/")?;
        }
        for commit in &self.matched {
            writeln!(f, "{}", graph_commit_string("* ", commit))?;
        }
        write!(f, "{}", graph_commit_string("* ", &self.merge_base))
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

/// Commit ancestry information for a branch and its configured upstream.
#[derive(Debug)]
struct BranchMergeBaseCommits<'a> {
    /// Local branch first-parent commits from tip down to, but excluding, the merge base.
    local_commits: Vec<gix::ObjectId>,
    /// Upstream branch first-parent commits from tip down to, but excluding, the merge base.
    upstream_commits: Vec<gix::ObjectId>,
    /// Shared merge base between the local branch and its upstream.
    merge_base: gix::ObjectId,
    /// Tracking branch reference name associated with the local branch.
    upstream_ref_name: std::borrow::Cow<'a, gix::refs::FullNameRef>,
}

impl InteractiveIntegration {
    /// Parse a textual integration script into an [`InteractiveIntegration`].
    ///
    /// Blank lines and comment lines starting with `#` are ignored.
    pub fn parse(input: &str) -> Result<Self> {
        let mut merge_base = None;
        let mut steps = Vec::new();

        for (line_idx, raw_line) in input.lines().enumerate() {
            let line_no = line_idx + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if merge_base.is_none() {
                let Some(rest) = line.strip_prefix("merge-base ") else {
                    bail!(
                        "Line {line_no}: expected first non-comment line to be 'merge-base <sha>'"
                    );
                };
                merge_base = Some(parse_object_id(rest.trim(), line_no)?);
                continue;
            }

            steps.push(parse_integration_step(line, line_no)?);
        }

        let Some(merge_base) = merge_base else {
            bail!("Missing required 'merge-base <sha>' line");
        };
        if steps.is_empty() {
            bail!("Integration steps cannot be empty");
        }

        Ok(Self { merge_base, steps })
    }
}

impl FromStr for InteractiveIntegration {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s)
    }
}

fn parse_integration_step(line: &str, line_no: usize) -> Result<InteractiveIntegrationStep> {
    let mut parts = line.splitn(2, ' ');
    let command = parts
        .next()
        .expect("splitn always yields at least one part for non-empty input");
    let rest = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Line {line_no}: missing arguments for '{command}'"))?
        .trim();

    match command {
        "pick" => Ok(InteractiveIntegrationStep::Pick {
            commit_id: parse_object_id(rest, line_no)?,
        }),
        "merge" => Ok(InteractiveIntegrationStep::Merge {
            commit_id: parse_object_id(rest, line_no)?,
        }),
        "squash" => parse_squash_step(rest, line_no),
        _ => bail!("Line {line_no}: unsupported integration command '{command}'"),
    }
}

fn parse_squash_step(rest: &str, line_no: usize) -> Result<InteractiveIntegrationStep> {
    let (commit_part, message) = if let Some((commits, suffix)) = rest.split_once('|') {
        let suffix = suffix.trim();
        let Some(message) = suffix.strip_prefix("message=") else {
            bail!("Line {line_no}: expected squash metadata suffix 'message=\"...\"'");
        };
        let message = parse_quoted_string(message, line_no)?;
        (commits.trim(), Some(message))
    } else {
        (rest, None)
    };

    let commits = commit_part
        .split_whitespace()
        .map(|token| parse_object_id(token, line_no))
        .collect::<Result<Vec<_>>>()?;
    if commits.len() < 2 {
        bail!("Line {line_no}: squash step must list at least two commits");
    }

    Ok(InteractiveIntegrationStep::Squash { commits, message })
}

fn parse_object_id(input: &str, line_no: usize) -> Result<gix::ObjectId> {
    gix::ObjectId::from_hex(input.as_bytes())
        .with_context(|| format!("Line {line_no}: '{input}' is not a valid full object ID"))
}

fn parse_quoted_string(input: &str, line_no: usize) -> Result<String> {
    let Some(content) = input
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        bail!("Line {line_no}: invalid squash message string");
    };

    let mut out = String::new();
    let mut chars = content.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            bail!("Line {line_no}: invalid squash message string");
        };
        match escaped {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            _ => bail!("Line {line_no}: invalid squash message string"),
        }
    }
    Ok(out)
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
    let (_, upstream_ref_name, _) = get_branch_tips_and_upstream(ref_name, repo)?;

    let upstream_ref_name = upstream_ref_name.as_ref();
    let editor_options = GraphEditorOptions {
        extra_refs: vec![ExtraRef::immutable(upstream_ref_name)],
        ..GraphEditorOptions::default()
    };
    let mut editor = Editor::create_with_opts(workspace, meta, repo, &editor_options)?;
    let prepared_steps = prepare_integration_steps_for_editor(&editor, &integration.steps)?;
    let delimiter_child = editor.select_reference(ref_name)?;
    let delimiter_parent = editor.select_commit(integration.merge_base)?;
    let segment_delimiter = SegmentDelimiter {
        child: delimiter_child,
        parent: delimiter_parent,
    };
    let children_to_disconnect = SelectorSet::All;
    let parents_to_disconnect = determine_parent_selector(&editor, delimiter_parent)?;

    let children_to_reconnect = selected_edges_from_set(
        &editor,
        segment_delimiter.child,
        &children_to_disconnect,
        EdgeSelection::Children,
    )?;
    let parents_to_reconnect = selected_edges_from_set(
        &editor,
        segment_delimiter.parent,
        &parents_to_disconnect,
        EdgeSelection::Parents,
    )?;

    editor.disconnect_segment_from(
        segment_delimiter,
        children_to_disconnect,
        parents_to_disconnect,
        true,
    )?;

    let new_segment_delimiter = integration_steps_into_segment_nodes(
        &mut editor,
        ref_name,
        integration.merge_base,
        &prepared_steps,
    )?;

    connect_segment_to_edges(
        &mut editor,
        new_segment_delimiter,
        &children_to_reconnect,
        &parents_to_reconnect,
    )?;

    editor.rebase()
}

#[derive(Debug, Clone)]
enum PreparedIntegrationStep {
    Pick { commit_id: gix::ObjectId },
    Merge { commit_id: gix::ObjectId },
}

fn prepare_integration_steps_for_editor<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    steps: &[InteractiveIntegrationStep],
) -> Result<Vec<PreparedIntegrationStep>> {
    steps
        .iter()
        .map(|step| match step {
            InteractiveIntegrationStep::Pick { commit_id } => Ok(PreparedIntegrationStep::Pick {
                commit_id: *commit_id,
            }),
            InteractiveIntegrationStep::Merge { commit_id } => Ok(PreparedIntegrationStep::Merge {
                commit_id: *commit_id,
            }),
            InteractiveIntegrationStep::Squash { commits, message } => {
                Ok(PreparedIntegrationStep::Pick {
                    commit_id: prepare_squash_step_for_editor(editor, commits, message.as_deref())?,
                })
            }
        })
        .collect()
}

/// Builds and inserts the integrated commit chain under `ref_name` down to `merge_base`.
///
/// Returns the delimiter spanning from the reference node to the deepest inserted parent.
fn integration_steps_into_segment_nodes<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    ref_name: &gix::refs::FullNameRef,
    merge_base: gix::ObjectId,
    steps: &[PreparedIntegrationStep],
) -> Result<SegmentDelimiter<Selector, Selector>> {
    // Step 1: We interpret the integration steps and transform them into graph steps disconnected from their parents.
    // We disconnect them in order to be able to allow for reordering.
    let segment_steps = integration_steps_to_segment_steps_for_editor(editor, ref_name, steps)?;

    // Step 2. We build the new local branch out of the steps.
    // We start by disconnecting all the parents of the local branch reference step, as we will connect it to the new
    // set of commits.
    let child_most = editor.select_reference(ref_name)?;
    disconnect_selector_from_all_parents(editor, child_most)?;
    let mut parent_most = child_most;

    for step in segment_steps.into_iter().skip(1) {
        if let Some(existing_parent) =
            already_connected_parent_for_step(editor, parent_most, &step)?
        {
            parent_most = existing_parent;
            continue;
        }

        parent_most = connect_parent_step(editor, parent_most, step)?;
    }

    // Step 3: Append the merge base at the bottom
    let merge_base_selector = editor.select_commit(merge_base)?;
    let merge_base_step = editor.lookup_step(merge_base_selector)?;
    parent_most = if let Some(existing_parent) =
        already_connected_parent_for_step(editor, parent_most, &merge_base_step)?
    {
        existing_parent
    } else {
        connect_parent_step(editor, parent_most, merge_base_step)?
    };

    Ok(SegmentDelimiter {
        child: child_most,
        parent: parent_most,
    })
}

/// Returns an already-connected parent selector for `child` when `step` points to an
/// existing pick node in the graph.
fn already_connected_parent_for_step<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    child: Selector,
    step: &Step,
) -> Result<Option<Selector>> {
    let Step::Pick(pick) = step else {
        return Ok(None);
    };

    let Some(existing_pick) = editor.try_select_commit(pick.id) else {
        return Ok(None);
    };

    let direct_parents = editor.direct_parents(child)?;
    Ok(direct_parents
        .into_iter()
        .find_map(|(parent, _)| (parent == existing_pick).then_some(parent)))
}

/// Connects `child` to `parent_step`, choosing the smallest available edge order.
///
/// Prefers order `0` when free; otherwise picks the next smallest unused order.
fn connect_parent_step<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    child: Selector,
    parent_step: Step,
) -> Result<Selector> {
    let parent = match parent_step {
        Step::Pick(pick) => {
            if let Some(existing_pick) = editor.try_select_commit(pick.id) {
                existing_pick
            } else {
                editor.add_step(Step::Pick(pick))?
            }
        }
        Step::Reference { refname } => editor.select_reference(refname.as_ref())?,
        Step::None => bail!("BUG: trying to connect to none"),
    };

    let used_orders = editor
        .direct_parents(child)?
        .into_iter()
        .map(|(_, order)| order)
        .collect::<HashSet<_>>();
    let mut order = 0;
    while used_orders.contains(&order) {
        order += 1;
    }

    editor.add_edge(child, parent, order)?;
    Ok(parent)
}

/// Converts user-provided integration steps into graph `Step`s in insertion order.
///
/// While translating, it applies graph detachments and prepares picks for
/// insertion under the target reference.
fn integration_steps_to_segment_steps_for_editor<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    ref_name: &gix::refs::FullNameRef,
    steps: &[PreparedIntegrationStep],
) -> Result<Vec<Step>> {
    let mut out = vec![Step::Reference {
        refname: ref_name.to_owned(),
    }];

    // Interactive steps are parent->child for execution. For graph connectivity
    // from reference(child-most) toward parents, we append in reverse.
    for step in steps.iter().rev() {
        match step {
            PreparedIntegrationStep::Pick { commit_id, .. } => {
                out.push(existing_or_new_pick_step(editor, *commit_id)?);
            }
            PreparedIntegrationStep::Merge { commit_id } => {
                let mut merge_commit = editor.empty_commit()?;
                merge_commit.message = format!("Merge {commit_id} into previous commit").into();
                let merge_commit =
                    editor.new_commit_untracked(merge_commit, DateMode::CommitterKeepAuthorKeep)?;
                let preserved_parents = editor
                    .find_commit(*commit_id)?
                    .inner
                    .parents
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                let mut commit_to_merge = Step::new_untracked_pick(*commit_id);
                let Step::Pick(pick) = &mut commit_to_merge else {
                    bail!("BUG: expected merge side parent to be a pick step");
                };
                pick.preserved_parents = Some(preserved_parents);
                let commit_to_merge = editor.add_step(commit_to_merge)?;
                let merge_commit = editor.add_step(Step::new_untracked_pick(merge_commit))?;
                editor.add_edge(merge_commit, commit_to_merge, 1)?;
                out.push(editor.lookup_step(merge_commit)?);
            }
        }
    }

    Ok(out)
}

/// Precompute the squash payload from the current editor/repository state,
/// before later integration graph mutations can rewire step-graph ancestry.
fn prepare_squash_step_for_editor<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    commit_ids: &[gix::ObjectId],
    message: Option<&str>,
) -> Result<gix::ObjectId> {
    if commit_ids.len() < 2 {
        bail!("Squash step must have at least two commits");
    }

    let maybe_selectors = commit_ids
        .iter()
        .map(|commit_id| editor.try_select_commit(*commit_id))
        .collect::<Vec<_>>();
    let ordered_commit_ids = if maybe_selectors.iter().all(Option::is_some) {
        let ordered_selectors = editor.order_commit_selectors_by_parentage(
            maybe_selectors
                .into_iter()
                .map(|selector| selector.expect("checked all selectors are present"))
                .collect::<Vec<_>>(),
        )?;
        ordered_selectors
            .iter()
            .map(|selector| {
                editor
                    .find_selectable_commit(*selector)
                    .map(|(_, commit)| commit.id)
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        commit_ids.to_vec()
    };

    let target_commit_id = *ordered_commit_ids
        .first()
        .expect("validated non-empty squash commit list");
    let merge_subject_ids = commit_ids
        .iter()
        .copied()
        .filter(|commit_id| *commit_id != target_commit_id)
        .collect::<Vec<_>>();
    let merge_outcome = editor.merge_commit_changes_to_tree(
        target_commit_id,
        merge_subject_ids,
        editor.repo().merge_options_force_ours()?,
    )?;
    let squashed_parent = editor
        .repo()
        .merge_base_octopus(ordered_commit_ids.iter().copied())
        .context("failed to compute squash merge-base")?
        .detach();

    let tip_commit_id = *ordered_commit_ids
        .last()
        .expect("validated non-empty squash commit list");
    let mut squashed_commit = editor.find_commit(tip_commit_id)?;
    squashed_commit.inner.parents = vec![squashed_parent].into();
    let commit_message = message
        .map(|message| message.as_bytes().to_vec())
        .unwrap_or_else(|| Vec::from(squashed_commit.message.clone()));
    apply_merge_commit_changes_outcome(
        editor.repo(),
        &mut squashed_commit,
        merge_outcome,
        commit_message,
    )?;

    editor.new_commit_untracked(squashed_commit, DateMode::CommitterUpdateAuthorKeep)
}

fn apply_merge_commit_changes_outcome(
    repo: &gix::Repository,
    commit: &mut but_core::CommitOwned,
    outcome: MergeCommitChangesOutcome,
    message: Vec<u8>,
) -> Result<()> {
    if let Some(conflict) = outcome.conflict {
        commit.tree = write_conflicted_tree(
            repo,
            outcome.tree_id,
            conflict.base_tree_id,
            conflict.ours_tree_id,
            conflict.theirs_tree_id,
            &conflict.conflict_entries,
        )?;
        commit.message = add_conflict_markers(BStr::new(&message));
    } else {
        commit.tree = outcome.tree_id;
        commit.message = message.into();
    }

    Ok(())
}

/// Produces a pick step for `commit_id`, reusing an existing selectable commit when present.
///
/// Existing commits are detached from selected parent edges first so they can be safely
/// reconnected into the new integration chain.
fn existing_or_new_pick_step<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    commit_id: gix::ObjectId,
) -> Result<Step> {
    if let Some(existing) = editor.try_select_commit(commit_id) {
        let parents_to_disconnect = determine_parent_selector(editor, existing)?;
        editor.disconnect_segment_from(
            SegmentDelimiter {
                child: existing,
                parent: existing,
            },
            SelectorSet::None,
            parents_to_disconnect,
            true,
        )?;

        return editor.lookup_step(existing);
    }

    Ok(Step::new_pick(commit_id))
}

/// Disconnects all parent edges from a single selector without reconnecting them.
///
/// This is used to isolate reference nodes before rebuilding integration connectivity.
fn disconnect_selector_from_all_parents<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    selector: Selector,
) -> Result<()> {
    editor.disconnect_segment_from(
        SegmentDelimiter {
            child: selector,
            parent: selector,
        },
        SelectorSet::None,
        SelectorSet::All,
        true,
    )?;

    Ok(())
}

#[derive(Clone, Copy)]
enum EdgeSelection {
    Children,
    Parents,
}

/// Resolves concrete direct edges selected by a `SelectorSet` for either children or
/// parents of `target`, preserving edge order metadata.
fn selected_edges_from_set<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    target: Selector,
    selectors: &SelectorSet,
    edge_selection: EdgeSelection,
) -> Result<Vec<(Selector, usize)>> {
    let available = match edge_selection {
        EdgeSelection::Children => editor.direct_children(target)?,
        EdgeSelection::Parents => editor.direct_parents(target)?,
    };

    match selectors {
        SelectorSet::All => Ok(available),
        SelectorSet::None => Ok(Vec::new()),
        SelectorSet::Some(some_selectors) => {
            let mut selected = Vec::new();
            for selector in some_selectors.as_slice() {
                let selector = selector.to_selector(editor)?;
                let Some((_, order)) = available
                    .iter()
                    .find(|(candidate, _)| *candidate == selector)
                else {
                    bail!("Selected edge endpoint wasn't found among direct neighbors")
                };
                selected.push((selector, *order));
            }
            Ok(selected)
        }
    }
}

/// Reconnects a newly built segment delimiter to previously selected child and parent
/// edge endpoints, assigning fresh edge orders after current maxima.
fn connect_segment_to_edges<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    delimiter: SegmentDelimiter<Selector, Selector>,
    children: &[(Selector, usize)],
    parents: &[(Selector, usize)],
) -> Result<()> {
    let max_child_weight = editor
        .direct_children(delimiter.child)?
        .into_iter()
        .map(|(_, order)| order)
        .max()
        .unwrap_or(0);

    for (child, order) in children {
        let next_order = max_child_weight + *order + 1;
        editor.add_edge(*child, delimiter.child, next_order)?;
    }

    let max_parent_weight = editor
        .direct_parents(delimiter.parent)?
        .into_iter()
        .map(|(_, order)| order)
        .max()
        .unwrap_or(0);

    for (parent, order) in parents {
        let next_order = max_parent_weight + *order + 1;
        editor.add_edge(delimiter.parent, *parent, next_order)?;
    }

    Ok(())
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
/// Returns the initial integration script and current divergence display state.
pub fn get_initial_integration_steps_for_branch(
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
) -> Result<InitialBranchIntegration> {
    let BranchMergeBaseCommits {
        local_commits,
        upstream_commits,
        merge_base,
        upstream_ref_name,
    } = get_commits_until_merge_base(ref_name, repo)?;

    let upstream_by_id = upstream_commits.iter().copied().collect::<HashSet<_>>();
    let mut upstream_by_change_id = HashMap::<String, gix::ObjectId>::new();
    for commit_id in &upstream_commits {
        let change_id = effective_change_id(repo, *commit_id)?;
        // Keep the first seen (closest to tip) upstream commit for stable matching.
        upstream_by_change_id.entry(change_id).or_insert(*commit_id);
    }

    let mut matched_upstream = HashSet::new();
    let mut local_result_order_commits = Vec::new();
    let mut divergence_local_only = Vec::new();
    let mut divergence_matched = Vec::new();
    for commit_id in local_commits {
        if upstream_by_id.contains(&commit_id) {
            matched_upstream.insert(commit_id);
            local_result_order_commits.push(commit_id);
            divergence_matched.push(divergence_commit(repo, commit_id)?);
            continue;
        }

        let change_id = effective_change_id(repo, commit_id)?;
        if let Some(upstream_commit_id) = upstream_by_change_id.get(&change_id) {
            matched_upstream.insert(*upstream_commit_id);
            local_result_order_commits.push(commit_id);
            divergence_matched.push(divergence_commit(repo, commit_id)?);
        } else {
            local_result_order_commits.push(commit_id);
            divergence_local_only.push(divergence_commit(repo, commit_id)?);
        }
    }

    let remote_only_commits = upstream_commits
        .into_iter()
        .filter(|id| !matched_upstream.contains(id));
    let mut divergence_upstream_only = Vec::new();

    let mut initial_steps = Vec::new();

    // Build the branch in natural tip-to-base result order first, then reverse
    // the whole sequence so the returned steps are ready to apply from the
    // merge-base upward.
    for commit in local_result_order_commits {
        initial_steps.push(InteractiveIntegrationStep::Pick { commit_id: commit });
    }

    for upstream_commit in remote_only_commits {
        divergence_upstream_only.push(divergence_commit(repo, upstream_commit)?);
        initial_steps.push(InteractiveIntegrationStep::Pick {
            commit_id: upstream_commit,
        });
    }

    initial_steps.reverse();

    let integration = InteractiveIntegration {
        steps: initial_steps,
        merge_base,
    };
    let mut divergence = IntegrationDivergenceDisplay {
        branch_ref_name: ref_name.to_owned(),
        upstream_ref_name: upstream_ref_name.into_owned(),
        local_only: divergence_local_only,
        upstream_only: divergence_upstream_only,
        matched: divergence_matched,
        merge_base: divergence_commit(repo, merge_base)?,
    };
    let local_tip = divergence
        .local_only
        .first()
        .map(|commit| commit.id)
        .or_else(|| divergence.matched.first().map(|commit| commit.id));
    let upstream_tip = divergence
        .upstream_only
        .first()
        .map(|commit| commit.id)
        .or_else(|| divergence.matched.first().map(|commit| commit.id));
    add_ref_label(
        &mut divergence.local_only,
        &mut divergence.matched,
        local_tip,
        divergence.branch_ref_name.shorten().to_string(),
    );
    add_ref_label(
        &mut divergence.upstream_only,
        &mut divergence.matched,
        upstream_tip,
        divergence.upstream_ref_name.shorten().to_string(),
    );

    Ok(InitialBranchIntegration {
        integration,
        divergence,
    })
}

/// Computes local and upstream commit lists (tip to merge-base, first-parent) together
/// with their merge base for a branch and its tracking branch.
fn get_commits_until_merge_base<'a>(
    ref_name: &'a gix::refs::FullNameRef,
    repo: &'a gix::Repository,
) -> Result<BranchMergeBaseCommits<'a>, anyhow::Error> {
    let (local_tip, upstream_ref_name, upstream_tip) =
        get_branch_tips_and_upstream(ref_name, repo)?;
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let merge_base = repo
        .merge_base_with_graph(local_tip.attach(repo), upstream_tip.attach(repo), &mut graph)
        .map(|id| id.detach())
        .map_err(|_| {
            anyhow::anyhow!(
                "No merge-base found between '{ref_name}' and its tracking branch '{upstream_ref_name}'"
            )
        })?;
    let local_commits = branch_commits_until(repo, local_tip, merge_base)?;
    let upstream_commits = branch_commits_until(repo, upstream_tip, merge_base)?;
    Ok(BranchMergeBaseCommits {
        local_commits,
        upstream_commits,
        merge_base,
        upstream_ref_name,
    })
}

/// Resolves local/upstream branch tips and tracking reference name for `ref_name`.
fn get_branch_tips_and_upstream<'a>(
    ref_name: &'a gix::refs::FullNameRef,
    repo: &'a gix::Repository,
) -> Result<
    (
        gix::ObjectId,
        std::borrow::Cow<'a, gix::refs::FullNameRef>,
        gix::ObjectId,
    ),
    anyhow::Error,
> {
    let mut local_branch = repo
        .find_reference(ref_name)
        .with_context(|| format!("Couldn't find local branch '{ref_name}'"))?;
    let local_tip = local_branch.peel_to_id()?.detach();
    let upstream_ref_name = resolve_tracking_branch_ref_name(ref_name, repo)?;
    let mut upstream_branch = repo
        .find_reference(upstream_ref_name.as_ref())
        .with_context(|| {
            format!(
                "Couldn't find tracking branch '{upstream_ref_name}' for local branch '{ref_name}'"
            )
        })?;
    let upstream_tip = upstream_branch.peel_to_id()?.detach();
    Ok((local_tip, upstream_ref_name, upstream_tip))
}

/// Resolve the remote-tracking ref that corresponds to `ref_name`.
///
/// This first honors the configured tracking branch. If there is no tracking
/// configuration, or it points at a missing ref, we fall back to a unique
/// `refs/remotes/*/<branch>` match, mirroring legacy `but` CLI behavior.
pub fn resolve_tracking_branch_ref_name<'a>(
    ref_name: &'a gix::refs::FullNameRef,
    repo: &'a gix::Repository,
) -> Result<std::borrow::Cow<'a, gix::refs::FullNameRef>> {
    if let Some(upstream_ref_name) = repo
        .branch_remote_tracking_ref_name(ref_name, Direction::Fetch)
        .transpose()?
        && repo
            .try_find_reference(upstream_ref_name.as_ref())?
            .is_some()
    {
        return Ok(upstream_ref_name);
    }

    let branch_name = ref_name.shorten();
    let mut remote_matches = repo
        .remote_names()
        .iter()
        .filter_map(|remote_name| {
            let full_name = format!("refs/remotes/{remote_name}/{branch_name}");
            repo.try_find_reference(&full_name)
                .transpose()
                .map(|reference| {
                    reference.map(|_| {
                        full_name
                            .try_into()
                            .expect("constructed remote-tracking refname must be valid")
                    })
                })
        })
        .collect::<Result<Vec<gix::refs::FullName>, _>>()?;

    if remote_matches.len() == 1 {
        return Ok(std::borrow::Cow::Owned(
            remote_matches
                .pop()
                .expect("exactly one remote match exists"),
        ));
    }

    bail!("Branch '{ref_name}' has no tracking branch")
}

/// Returns first-parent commits reachable from `tip` until (excluding) `merge_base`.
fn branch_commits_until(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    merge_base: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    let traversal = tip
        .attach(repo)
        .ancestors()
        .with_hidden(Some(merge_base))
        .first_parent_only()
        .all()?;

    let mut out = Vec::new();
    for info in traversal {
        out.push(info?.id);
    }
    Ok(out)
}

/// Returns the effective change-id string for a commit, used for rewritten-commit matching.
fn effective_change_id(repo: &gix::Repository, commit_id: gix::ObjectId) -> Result<String> {
    Ok(but_core::Commit::from_id(commit_id.attach(repo))?
        .change_id()
        .to_string())
}

fn divergence_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> Result<IntegrationDivergenceCommit> {
    Ok(IntegrationDivergenceCommit {
        id: commit_id,
        subject: but_core::Commit::from_id(commit_id.attach(repo))?
            .message
            .lines()
            .next()
            .unwrap_or_default()
            .to_str_lossy()
            .into_owned(),
        refs: Vec::new(),
    })
}

fn add_ref_label(
    primary: &mut [IntegrationDivergenceCommit],
    secondary: &mut [IntegrationDivergenceCommit],
    id: Option<gix::ObjectId>,
    label: String,
) {
    let Some(id) = id else {
        return;
    };
    if let Some(commit) = primary.iter_mut().find(|commit| commit.id == id) {
        if !commit.refs.contains(&label) {
            commit.refs.push(label);
        }
        return;
    }
    if let Some(commit) = secondary.iter_mut().find(|commit| commit.id == id)
        && !commit.refs.contains(&label)
    {
        commit.refs.push(label);
    }
}

fn graph_commit_string(prefix: &str, commit: &IntegrationDivergenceCommit) -> String {
    let refs = if commit.refs.is_empty() {
        String::new()
    } else {
        format!(" ({})", commit.refs.join(", "))
    };
    format!(
        "{prefix}{}{} {}",
        commit.id.to_hex_with_len(7),
        refs,
        commit.subject
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).expect("valid object id")
    }

    #[test]
    fn interactive_integration_step_display_is_stable() {
        let parent = oid("1111111111111111111111111111111111111111");
        let squash_parent = oid("2222222222222222222222222222222222222222");
        let squash_child = oid("3333333333333333333333333333333333333333");
        let pick = InteractiveIntegrationStep::Pick { commit_id: parent };
        assert_eq!(pick.to_string(), format!("pick {parent}"));

        let squash_without_message = InteractiveIntegrationStep::Squash {
            commits: vec![squash_parent, squash_child],
            message: None,
        };
        assert_eq!(
            squash_without_message.to_string(),
            format!("squash {squash_parent} {squash_child}")
        );

        let squash_with_message = InteractiveIntegrationStep::Squash {
            commits: vec![squash_parent, squash_child],
            message: Some("hello \"world\"".to_string()),
        };
        assert_eq!(
            squash_with_message.to_string(),
            format!("squash {squash_parent} {squash_child} | message=\"hello \\\"world\\\"\"")
        );
    }

    #[test]
    fn interactive_integration_parse_round_trips_display_format() {
        let merge_base = oid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let parent = oid("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let upstream = oid("cccccccccccccccccccccccccccccccccccccccc");
        let squash_child = oid("dddddddddddddddddddddddddddddddddddddddd");

        let integration = InteractiveIntegration {
            merge_base,
            steps: vec![
                InteractiveIntegrationStep::Pick { commit_id: parent },
                InteractiveIntegrationStep::Pick {
                    commit_id: upstream,
                },
                InteractiveIntegrationStep::Squash {
                    commits: vec![parent, squash_child],
                    message: Some("hello".into()),
                },
            ],
        };

        let parsed = InteractiveIntegration::parse(&integration.to_string()).expect(
            "display format should remain parseable so the TUI can round-trip edited scripts",
        );
        assert_eq!(parsed.merge_base, integration.merge_base);
        assert_eq!(parsed.steps.len(), integration.steps.len());
    }

    #[test]
    fn interactive_integration_parse_rejects_skip_command() {
        let merge_base = oid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let parent = oid("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        let err =
            InteractiveIntegration::parse(&format!("merge-base {merge_base}\nskip {parent}\n"))
                .expect_err("skip commands should be rejected in integration scripts");

        assert!(
            err.to_string()
                .contains("unsupported integration command 'skip'"),
            "skip command should be rejected explicitly: {err:#}"
        );
    }

    #[test]
    fn interactive_integration_parse_requires_merge_base() {
        let err = InteractiveIntegration::parse("pick 1111111111111111111111111111111111111111\n")
            .expect_err("script without merge-base must fail");

        assert!(
            err.to_string().contains("merge-base"),
            "missing merge-base should be called out clearly"
        );
    }

    #[test]
    fn interactive_integration_parse_rejects_invalid_squash_message() {
        let err = InteractiveIntegration::parse(
            "merge-base 1111111111111111111111111111111111111111\nsquash 2222222222222222222222222222222222222222 3333333333333333333333333333333333333333 | message=hello\n",
        )
        .expect_err("unquoted squash message must fail");

        assert!(
            err.to_string().contains("invalid squash message"),
            "invalid squash message should produce a targeted error"
        );
    }

    #[test]
    fn divergence_display_renders_git_style_graph() {
        let display = IntegrationDivergenceDisplay {
            branch_ref_name: gix::refs::Category::LocalBranch
                .to_full_name("feature")
                .expect("valid local branch"),
            upstream_ref_name: gix::refs::Category::RemoteBranch
                .to_full_name("origin/feature")
                .expect("valid remote branch"),
            local_only: vec![IntegrationDivergenceCommit {
                id: oid("1111111111111111111111111111111111111111"),
                subject: "local tip".into(),
                refs: vec!["feature".into()],
            }],
            upstream_only: vec![IntegrationDivergenceCommit {
                id: oid("2222222222222222222222222222222222222222"),
                subject: "remote tip".into(),
                refs: vec!["origin/feature".into()],
            }],
            matched: vec![IntegrationDivergenceCommit {
                id: oid("3333333333333333333333333333333333333333"),
                subject: "shared".into(),
                refs: Vec::new(),
            }],
            merge_base: IntegrationDivergenceCommit {
                id: oid("4444444444444444444444444444444444444444"),
                subject: "base".into(),
                refs: Vec::new(),
            },
        };

        insta::assert_snapshot!(
            display.to_string(),
            "graph output should stay stable because the CLI and frontend consume it directly",
            @r"
        * 1111111 (feature) local tip
        | * 2222222 (origin/feature) remote tip
        |/
        * 3333333 shared
        * 4444444 base
        "
        );
    }
}

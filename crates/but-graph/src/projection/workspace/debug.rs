//! On-demand DOT/SVG visualization of a [`Workspace`]'s commit graph, for debugging only
//! (support bundles, `but-debug graph`, the desktop "show graph" action). Renders the canonical
//! [`CommitGraph`](crate::CommitGraph) directly — there is no record graph to draw.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;

use anyhow::{Context as _, bail};
use bstr::{BString, ByteSlice, ByteVec};
use gix::reference::Category;
use petgraph::stable_graph::EdgeReference;

use super::WorkspaceKind;
use crate::Workspace;
use crate::commit_graph::{CommitGraph, ParentEdge};

impl Workspace {
    /// Render the commit graph as a Graphviz DOT string, pruning history strictly below
    /// `lower_bound` (the workspace base, when known) and capping the node count so `dot` stays
    /// responsive. Diagnostic-only: returns an empty graph rather than failing when no commit graph
    /// was built (e.g. an unborn or default workspace).
    pub fn dot_graph_pruned(&self, lower_bound: Option<gix::ObjectId>) -> String {
        let Some(cg) = self.commit_graph.as_ref() else {
            return "digraph {\n}\n".to_string();
        };
        // Compute stop markers on the *full* graph so a pruned-away parent doesn't masquerade as a
        // traversal limit.
        let stops = leaf_stops(cg, self.hard_limit_hit);
        let mut cg = cg.clone();
        prune_below(&mut cg, lower_bound);
        render_dot(&cg, &stops)
    }

    /// Print the DOT graph to stderr.
    pub fn eprint_dot_graph(&self) {
        eprintln!("{}", self.dot_graph_pruned(self.lower_bound));
    }

    /// Render the DOT graph to a temporary SVG via `dot` and open it. Best-effort: logs and returns
    /// on any failure, since this is a debugging convenience.
    pub fn open_as_svg(&self) {
        let dot = self.dot_graph_pruned(self.lower_bound);
        if let Err(err) = render_and_open_svg(&dot) {
            tracing::warn!("Failed to open graph as SVG: {err:#}");
        }
    }

    /// Deterministically anonymize every branch/ref name this workspace carries, for shared
    /// diagnostics like support bundles. Local branches/tags become `A`, `B`, … and remotes become
    /// `remote-N/X`, consistently across every carried view — the commit graph (what the DOT
    /// renders), the branch records, the display stacks, the targets, and the metadata — so a
    /// rendered graph leaks no real names. Commit hashes and flags are kept.
    ///
    /// The walk must touch *every* ref-bearing field: a missed one leaks a real name into a bundle.
    pub fn anonymize(&mut self, remotes: &gix::remote::Names) -> anyhow::Result<&mut Self> {
        let mut a = Anonymizer::default();

        // Commit graph — the security-critical surface, since the DOT renders it.
        if let Some(cg) = self.commit_graph.as_mut() {
            for commit in cg.inner.node_weights_mut() {
                anon_refs(&mut a, &mut commit.refs, remotes)?;
            }
        }
        // Branch records (rendered by branch_tree).
        for branch in self.branches.iter_mut().flatten() {
            if let Some(rn) = branch.ref_name.as_mut() {
                a.anon(rn, remotes)?;
            }
            for commit in &mut branch.commits {
                anon_refs(&mut a, &mut commit.refs, remotes)?;
            }
        }
        // Display stacks (rendered by graph_workspace).
        for segment in self.stacks.iter_mut().flat_map(|s| s.segments.iter_mut()) {
            if let Some(ri) = segment.ref_info.as_mut() {
                a.anon(&mut ri.ref_name, remotes)?;
            }
            if let Some(rn) = segment.remote_tracking_ref_name.as_mut() {
                a.anon(rn, remotes)?;
            }
            let commits = segment
                .commits
                .iter_mut()
                .chain(segment.commits_outside.iter_mut().flatten())
                .chain(segment.commits_on_remote.iter_mut());
            for commit in commits {
                anon_refs(&mut a, &mut commit.refs, remotes)?;
            }
        }
        // Workspace-level refs.
        if let Some(ri) = self.ref_info.as_mut() {
            a.anon(&mut ri.ref_name, remotes)?;
        }
        match &mut self.kind {
            WorkspaceKind::Managed { ref_info }
            | WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => {
                a.anon(&mut ref_info.ref_name, remotes)?;
            }
            WorkspaceKind::AdHoc => {}
        }
        if let Some(rn) = self.entrypoint_ref.as_mut() {
            a.anon(rn, remotes)?;
        }
        if let Some(rn) = self.lower_bound_ref_name.as_mut() {
            a.anon(rn, remotes)?;
        }
        if let Some(target) = self.target_ref.as_mut() {
            a.anon(&mut target.ref_name, remotes)?;
            if let Some(ri) = target.local_tracking.as_mut() {
                a.anon(&mut ri.ref_name, remotes)?;
            }
        }
        if let Some(ancestor) = self.ancestor_workspace_commit.as_mut() {
            for commit in &mut ancestor.commits_outside {
                anon_refs(&mut a, &mut commit.refs, remotes)?;
            }
        }
        for (rn, _) in self
            .named_segments
            .iter_mut()
            .chain(self.ref_tips.iter_mut())
        {
            a.anon(rn, remotes)?;
        }
        // Unreconciled metadata (matches the former segment-graph anonymizer).
        if let Some(md) = self.metadata.as_mut() {
            for branch in md.stacks.iter_mut().flat_map(|s| s.branches.iter_mut()) {
                a.anon(&mut branch.ref_name, remotes)?;
            }
        }
        Ok(self)
    }
}

/// Map each commit with no in-graph parents to a stop marker: 🏁 (a true root, no parents at all)
/// or ✂ / ❌ (parents exist but were not traversed — a soft or hard limit).
fn leaf_stops(cg: &CommitGraph, hard_limit_hit: bool) -> HashMap<gix::ObjectId, &'static str> {
    cg.inner
        .node_weights()
        .filter_map(|c| {
            let nx = cg.node(c.id)?;
            if cg.parents(nx).next().is_some() {
                return None; // has in-graph parents — not a stop
            }
            let marker = if c.parent_ids.is_empty() {
                "🏁"
            } else if hard_limit_hit {
                "❌"
            } else {
                "✂"
            };
            Some((c.id, marker))
        })
        .collect()
}

/// Remove commits strictly below `lower_bound` (its ancestors, excluding itself) so the diagram
/// stays focused on the workspace, and cap the total node count so `dot` won't hang.
fn prune_below(cg: &mut CommitGraph, lower_bound: Option<gix::ObjectId>) {
    if let Some(lb) = lower_bound {
        let strict_ancestors: Vec<_> = cg
            .ancestor_ids(lb)
            .into_iter()
            .filter(|id| *id != lb)
            .collect();
        for id in strict_ancestors {
            if let Some(nx) = cg.node(id) {
                cg.inner.remove_node(nx);
            }
        }
    }
    const LIMIT: usize = 5000;
    if cg.inner.node_count() > LIMIT {
        // Keep the newest commits (lowest generation) so the workspace stays visible.
        let generations = cg.generations();
        let mut by_generation: Vec<_> = cg.inner.node_indices().collect();
        by_generation
            .sort_by_key(|nx| std::cmp::Reverse(generations.get(nx).copied().unwrap_or(0)));
        for nx in by_generation
            .into_iter()
            .take(cg.inner.node_count() - LIMIT)
        {
            cg.inner.remove_node(nx);
        }
        tracing::warn!("Pruned the oldest commits beyond {LIMIT} so 'dot' won't hang");
    }
}

fn render_dot(cg: &CommitGraph, stops: &HashMap<gix::ObjectId, &'static str>) -> String {
    let node_attrs = |_: &_, (_, c): (_, &crate::Commit)| {
        let refs = c
            .refs
            .iter()
            .map(|ri| ri.debug_string())
            .collect::<Vec<_>>()
            .join(" ");
        let refs = if refs.is_empty() {
            String::new()
        } else {
            format!(" {refs}")
        };
        let stop = stops.get(&c.id).copied().unwrap_or("");
        format!(
            ", shape = box, label = \"{stop}{id} ({flags}){refs}\", fontname = Courier",
            id = c.id.to_hex_with_len(7),
            flags = c.flags.debug_string(None),
        )
    };
    let edge_attrs = |_: &_, e: EdgeReference<'_, ParentEdge>| match e.weight().parent_order {
        0 => ", label = \"\"".to_string(),
        order => format!(", label = \"{order}\", fontname = Courier"),
    };
    let dot = petgraph::dot::Dot::with_attr_getters(&cg.inner, &[], &edge_attrs, &node_attrs);
    format!("{dot:?}")
}

/// Run `dot -Tsvg` over `dot_contents` into a temp file and open it with the platform opener.
fn render_and_open_svg(dot_contents: &str) -> anyhow::Result<()> {
    let dir = std::env::temp_dir();
    let svg_path = dir.join("but-graph.svg");
    let mut child = std::process::Command::new("dot")
        .args(["-Tsvg", "-o"])
        .arg(&svg_path)
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(dot_contents.as_bytes())?;
    let status = child.wait()?;
    anyhow::ensure!(status.success(), "`dot` exited with {status}");
    let opened = std::process::Command::new("open").arg(&svg_path).status()?;
    anyhow::ensure!(opened.success(), "failed to open {}", svg_path.display());
    Ok(())
}

/// Anonymize every ref carried by a list of commits.
fn anon_refs(
    a: &mut Anonymizer,
    refs: &mut [crate::RefInfo],
    remotes: &gix::remote::Names,
) -> anyhow::Result<()> {
    for ri in refs {
        a.anon(&mut ri.ref_name, remotes)?;
    }
    Ok(())
}

/// Deterministic branch-name anonymization state, shared across all of a workspace's carried views.
#[derive(Default)]
struct Anonymizer {
    remote_mapping: BTreeMap<BString, BString>,
    name_mapping: BTreeMap<BString, BString>,
}

impl Anonymizer {
    fn anon(
        &mut self,
        rn: &mut gix::refs::FullName,
        remotes: &gix::remote::Names,
    ) -> anyhow::Result<()> {
        // 0-based bijective base-26: 0→A, 1→B, …, 25→Z, 26→AA. (A previous form mapped both 0 and
        // 1 to "A", colliding the first two distinct names — invisible only when one happened to be
        // a dropped ref.)
        fn int_to_alpha(mut n: usize) -> String {
            let mut result = String::new();
            loop {
                result.insert(0, (b'A' + (n % 26) as u8) as char);
                if n < 26 {
                    break;
                }
                n = n / 26 - 1;
            }
            result
        }

        let (category, short_name) = rn
            .category_and_short_name()
            .with_context(|| format!("Couldn't classify reference '{rn}'"))?;
        match category {
            Category::Tag | Category::LocalBranch => {
                let num_names = self.name_mapping.len();
                let new_name = self
                    .name_mapping
                    .entry(short_name.to_owned())
                    .or_insert_with(|| int_to_alpha(num_names).into());
                *rn = category.to_full_name(new_name.as_bstr())?;
            }
            Category::RemoteBranch => {
                let (remote_name, short_name) = remotes
                    .iter()
                    .rev()
                    .find_map(|remote| {
                        rn.as_bstr()[Category::RemoteBranch.prefix().len()..]
                            .as_bstr()
                            .strip_prefix(remote.as_bytes())
                            .map(|short_name| (remote, short_name.as_bstr()))
                    })
                    .with_context(|| format!("Couldn't determine remote name in {rn}"))?;

                let short_name = short_name
                    .strip_prefix(b"/")
                    .with_context(|| {
                        format!("Couldn't *unambiguously* determine remote name in {rn}")
                    })?
                    .as_bstr();

                let mut new_name: BString = "refs/remotes/".into();

                let num_remotes = self.remote_mapping.len();
                let new_remote_name = self
                    .remote_mapping
                    .entry(remote_name.as_bstr().to_owned())
                    .or_insert_with(|| format!("remote-{num_remotes}").into());
                new_name.push_str(new_remote_name);

                let num_names = self.name_mapping.len();
                let new_short_name = self
                    .name_mapping
                    .entry(short_name.to_owned())
                    .or_insert_with(|| int_to_alpha(num_names).into());
                new_name.push_byte(b'/');
                new_name.push_str(new_short_name);
                *rn = gix::refs::FullName::try_from(new_name.as_bstr())
                    .expect("Our replacement names are always valid");
            }

            Category::Note
            | Category::PseudoRef
            | Category::MainPseudoRef
            | Category::MainRef
            | Category::LinkedPseudoRef { .. }
            | Category::LinkedRef { .. }
            | Category::Bisect
            | Category::Rewritten
            | Category::WorktreePrivate => {
                bail!("Can't handle reference '{rn}' of category '{category:?}'");
            }
        }
        Ok(())
    }
}

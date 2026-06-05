use std::collections::{BTreeMap, VecDeque};

use anyhow::{Context as _, bail};
use bstr::{BString, ByteSlice, ByteVec};
use gix::reference::Category;
use petgraph::{prelude::EdgeRef, stable_graph::EdgeReference};

use crate::{
    CommitFlags, Edge, Graph, Segment, SegmentIndex, SegmentMetadata, StopCondition, init::PetGraph,
};

/// Debugging
impl Graph {
    /// Assure that no PII is left in the graph, by deterministically anonymizing branch names.
    ///
    /// What it will keep is `gitbutler/` references, and all commit information (flags, hashes).
    ///
    /// Use `remotes` to know how to separate the remote name from the branch name of a short name.
    pub fn anonymize(&mut self, remotes: &gix::remote::Names) -> anyhow::Result<&mut Self> {
        fn int_to_alpha(mut n: usize) -> String {
            let mut result = String::new();
            while n > 0 {
                n -= 1; // Adjust for 0-based indexing in base-26
                let remainder = n % 26;
                let c = (b'A' + remainder as u8) as char;
                result.insert(0, c);
                n /= 26;
            }
            if result.is_empty() {
                result.push('A');
            }
            result
        }

        let mut remote_mapping = BTreeMap::<BString, BString>::new();
        let mut name_mapping = BTreeMap::<BString, BString>::new();
        let mut anon = |rn: &mut gix::refs::FullName| -> anyhow::Result<()> {
            let (category, short_name) = rn
                .category_and_short_name()
                .with_context(|| format!("Couldn't classify reference '{rn}'"))?;
            match category {
                Category::Tag | Category::LocalBranch => {
                    let num_names = name_mapping.len();
                    let new_name = name_mapping
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

                    let num_remotes = remote_mapping.len();
                    let new_remote_name = remote_mapping
                        .entry(remote_name.as_bstr().to_owned())
                        .or_insert_with(|| format!("remote-{num_remotes}").into());
                    new_name.push_str(new_remote_name);

                    let num_names = name_mapping.len();
                    let new_short_name = name_mapping
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
        };
        for node in self.inner.node_weights_mut() {
            if let Some(ri) = node.ref_info.as_mut() {
                anon(&mut ri.ref_name)?;
            }
            if let Some(rn) = node.remote_tracking_ref_name.as_mut() {
                anon(rn)?;
            }
            for ri in node.commits.iter_mut().flat_map(|c| c.refs.iter_mut()) {
                anon(&mut ri.ref_name)?;
            }
            if let Some(SegmentMetadata::Workspace(md)) = node.metadata.as_mut() {
                for rn in md
                    .stacks
                    .iter_mut()
                    .flat_map(|s| s.branches.iter_mut().map(|b| &mut b.ref_name))
                {
                    anon(rn)?;
                }
            }
        }
        Ok(self)
    }
    /// Produce a string that concisely represents `commit`, adding `extra` information as needed.
    pub fn commit_debug_string(
        commit: &crate::Commit,
        is_entrypoint: bool,
        stop_condition: Option<StopCondition>,
        hard_limit: bool,
        max_goals: Option<usize>,
    ) -> String {
        Self::commit_debug_string_inner(
            commit,
            is_entrypoint,
            stop_condition,
            hard_limit,
            max_goals,
            false,
        )
    }

    /// Like [`Self::commit_debug_string()`], but includes graph-contextual worktree ownership markers.
    pub fn commit_debug_string_with_graph_context(
        &self,
        commit: &crate::Commit,
        is_entrypoint: bool,
        stop_condition: Option<StopCondition>,
        hard_limit: bool,
        max_goals: Option<usize>,
    ) -> String {
        Self::commit_debug_string_inner(
            commit,
            is_entrypoint,
            stop_condition,
            hard_limit,
            max_goals,
            self.has_multiple_worktrees(),
        )
    }

    fn commit_debug_string_inner(
        commit: &crate::Commit,
        is_entrypoint: bool,
        stop_condition: Option<StopCondition>,
        hard_limit: bool,
        max_goals: Option<usize>,
        show_owned_by_repo: bool,
    ) -> String {
        format!(
            "{ep}{end}{kind}{hex}{flags}{refs}",
            ep = if is_entrypoint { "👉" } else { "" },
            end = stop_condition
                .map(|condition| condition.debug_string(hard_limit))
                .unwrap_or_default(),
            kind = if commit.flags.is_remote() {
                "🟣"
            } else {
                "·"
            },
            flags = if !commit.flags.is_empty() {
                format!(" ({})", commit.flags.debug_string(max_goals))
            } else {
                "".to_string()
            },
            hex = commit.id.to_hex_with_len(7),
            refs = if commit.refs.is_empty() {
                "".to_string()
            } else {
                format!(
                    " {}",
                    commit
                        .refs
                        .iter()
                        .map(|ri| format!("►{}", {
                            Self::ref_debug_string_inner(
                                ri.ref_name.as_ref(),
                                ri.worktree.as_ref(),
                                show_owned_by_repo,
                            )
                        }))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )
    }

    /// Shorten the given `name` so it's still clear if it is a special ref (like tag) or not.
    pub fn ref_debug_string(
        ref_name: &gix::refs::FullNameRef,
        worktree: Option<&crate::Worktree>,
    ) -> String {
        Self::ref_debug_string_inner(ref_name, worktree, false)
    }

    /// Like [`Self::ref_debug_string()`], but includes graph-contextual worktree ownership markers.
    pub fn ref_debug_string_with_graph_context(
        &self,
        ref_name: &gix::refs::FullNameRef,
        worktree: Option<&crate::Worktree>,
    ) -> String {
        Self::ref_debug_string_inner(ref_name, worktree, self.has_multiple_worktrees())
    }

    fn ref_debug_string_inner(
        ref_name: &gix::refs::FullNameRef,
        worktree: Option<&crate::Worktree>,
        show_owned_by_repo: bool,
    ) -> String {
        let (cat, sn) = ref_name.category_and_short_name().expect("valid refs");
        // Only shorten those that look good and are unambiguous enough.
        format!(
            "{}{ws}",
            if matches!(cat, Category::LocalBranch | Category::RemoteBranch) {
                sn
            } else {
                ref_name
                    .as_bstr()
                    .strip_prefix(b"refs/")
                    .map(|n| n.as_bstr())
                    .unwrap_or(ref_name.as_bstr())
            },
            ws = worktree
                .map(|wt| wt.debug_string_with_graph_context(ref_name, show_owned_by_repo))
                .unwrap_or_default()
        )
    }

    /// Return a useful one-line string showing the relationship between `ref_name`, `remote_ref_name` and how
    /// they are linked with `sibling_id` and `remote_tracking_branch_id`.
    pub fn ref_and_remote_debug_string(
        ref_info: Option<&crate::RefInfo>,
        remote_ref_name: Option<&gix::refs::FullName>,
        sibling_id: Option<SegmentIndex>,
        remote_tracking_branch_id: Option<SegmentIndex>,
    ) -> String {
        Self::ref_and_remote_debug_string_inner(
            ref_info,
            remote_ref_name,
            sibling_id,
            remote_tracking_branch_id,
            false,
        )
    }

    /// Like [`Self::ref_and_remote_debug_string()`], but includes graph-contextual worktree ownership markers.
    pub fn ref_and_remote_debug_string_with_graph_context(
        &self,
        ref_info: Option<&crate::RefInfo>,
        remote_ref_name: Option<&gix::refs::FullName>,
        sibling_id: Option<SegmentIndex>,
        remote_tracking_branch_id: Option<SegmentIndex>,
    ) -> String {
        Self::ref_and_remote_debug_string_inner(
            ref_info,
            remote_ref_name,
            sibling_id,
            remote_tracking_branch_id,
            self.has_multiple_worktrees(),
        )
    }

    fn ref_and_remote_debug_string_inner(
        ref_info: Option<&crate::RefInfo>,
        remote_ref_name: Option<&gix::refs::FullName>,
        sibling_id: Option<SegmentIndex>,
        remote_tracking_branch_id: Option<SegmentIndex>,
        show_owned_by_repo: bool,
    ) -> String {
        format!(
            "{ref_name}{remote}",
            ref_name = ref_info
                .as_ref()
                .map(|ri| format!(
                    "{}{maybe_id}",
                    Graph::ref_debug_string_inner(
                        ri.ref_name.as_ref(),
                        ri.worktree.as_ref(),
                        show_owned_by_repo,
                    ),
                    maybe_id = sibling_id
                        .filter(|_| remote_ref_name.is_none())
                        .map(|id| format!(" →:{}:", id.index()))
                        .unwrap_or_default()
                ))
                .unwrap_or_else(|| format!(
                    "anon:{maybe_id}",
                    maybe_id = sibling_id
                        .map(|id| format!(" →:{}:", id.index()))
                        .unwrap_or_default()
                )),
            remote = remote_ref_name
                .as_ref()
                .map(|remote_ref_name| format!(
                    " <> {remote_name}{maybe_id}",
                    remote_name = Graph::ref_debug_string(remote_ref_name.as_ref(), None),
                    maybe_id = remote_tracking_branch_id
                        .or(sibling_id)
                        .map(|id| format!(" →:{}:", id.index()))
                        .unwrap_or_default()
                ))
                .unwrap_or_default()
        )
    }

    /// Validate the graph for consistency and fail loudly when an issue was found, after printing the dot graph.
    /// Mostly useful for debugging to stop early when a connection wasn't created correctly.
    #[cfg(unix)]
    pub fn validated_or_open_as_svg(self) -> anyhow::Result<Self> {
        use petgraph::visit::IntoEdgeReferences;
        for edge in self.inner.edge_references() {
            let res = Self::check_edge(&self.inner, edge, false);
            if res.is_err() {
                self.open_as_svg();
            }
            res?;
        }
        Ok(self)
    }

    /// Output this graph in dot-format to stderr to allow copying it, and using like this for visualization:
    ///
    /// ```shell
    /// pbpaste | dot -Tsvg >graph.svg && open graph.svg
    /// ```
    ///
    /// Note that this may reveal additional debug information when invariants of the graph are violated.
    /// This often is more useful than seeing a hard error, which can be achieved with `Self::validated()`
    pub fn eprint_dot_graph(&self) {
        let dot = self.dot_graph_pruned();
        eprintln!("{dot}");
    }

    /// Open an SVG dot visualization in the browser or panic if the `dot` or `open` tool can't be found.
    #[cfg(unix)]
    #[tracing::instrument(skip(self))]
    pub fn open_as_svg(&self) {
        use std::{io::Write, process::Stdio, sync::atomic::AtomicUsize};

        static SUFFIX: AtomicUsize = AtomicUsize::new(0);
        let suffix = SUFFIX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let svg_name = format!("debug-graph-{suffix:02}.svg");
        let svg_path = std::env::var_os("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_default()
            .join(svg_name);
        let mut dot = std::process::Command::new("dot")
            .args(["-Tsvg", "-o"])
            .arg(&svg_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("'dot' (graphviz) must be installed on the system");
        dot.stdin
            .as_mut()
            .unwrap()
            .write_all(self.dot_graph_pruned().as_bytes())
            .ok();
        let mut out = dot.wait_with_output().unwrap();
        out.stdout.extend(out.stderr);
        assert!(
            out.status.success(),
            "dot failed: {out}",
            out = out.stdout.as_bstr()
        );

        assert!(
            std::process::Command::new("open")
                .arg(&svg_path)
                .status()
                .unwrap()
                .success(),
            "Opening of {svg_path} failed",
            svg_path = svg_path.display()
        );
    }

    /// Return the highest amount of goals that a commit has stored in its flags.
    ///
    /// This relates to the amount of commits who were tracked for reachability, i.e. allowing an ancestor to see
    /// if a particular commit is in its future.
    pub fn max_goals(&self) -> Option<usize> {
        self.node_weights()
            .filter_map(|s| s.commits.iter().map(|c| c.flags.num_goals()).max())
            .max()
    }

    /// Return `true` if more than one unique worktree is referenced by the graph.
    pub(crate) fn has_multiple_worktrees(&self) -> bool {
        let mut first: Option<&crate::WorktreeKind> = None;
        self.node_weights()
            .flat_map(|segment| {
                segment
                    .ref_info
                    .iter()
                    .chain(segment.commits.iter().flat_map(|commit| commit.refs.iter()))
            })
            .filter_map(|ref_info| ref_info.worktree.as_ref())
            .any(|worktree| {
                if let Some(first) = first {
                    first != &worktree.kind
                } else {
                    first = Some(&worktree.kind);
                    false
                }
            })
    }

    /// Produces a pruned dot-version of the graph.
    ///
    /// This best-effort rendering path prunes very large graphs from the bottom
    /// before handing them to Graphviz, because complete DOT input can make
    /// `dot` hang on huge histories. It tries to compute the workspace lower
    /// bound to preserve workspace-relevant segments and clear stale
    /// `InWorkspace` flags below that bound, but projection is allowed to fail:
    /// DOT output is diagnostic-only, so a workspace projection error must not
    /// prevent rendering the graph.
    pub fn dot_graph_pruned(&self) -> String {
        let mut display_graph = self.clone();
        display_graph.prune_for_dot_graph();
        display_graph.dot_graph_unpruned()
    }

    /// Produces a dot-version of the graph without pruning.
    fn dot_graph_unpruned(&self) -> String {
        const HEX: usize = 7;
        let entrypoint = self.entrypoint_location();
        let max_goals = self.max_goals();
        let show_owned_by_repo = self.has_multiple_worktrees();
        let node_attrs = |_: &PetGraph, (sidx, s): (SegmentIndex, &Segment)| {
            let name = format!(
                "{ref_name_and_remote}{maybe_centering_newline}",
                ref_name_and_remote = Self::ref_and_remote_debug_string_inner(
                    s.ref_info.as_ref(),
                    s.remote_tracking_ref_name.as_ref(),
                    s.sibling_segment_id,
                    s.remote_tracking_branch_segment_id,
                    show_owned_by_repo,
                ),
                maybe_centering_newline = if s.commits.is_empty() { "" } else { "\n" },
            );
            // Reduce noise by preferring ref-based entry-points.
            let show_segment_entrypoint = s.ref_info.is_some()
                && entrypoint.is_some_and(|(s, cidx)| s == sidx && matches!(cidx, None | Some(0)));
            let mut commits = s
                .commits
                .iter()
                .enumerate()
                .map(|(cidx, c)| {
                    Self::commit_debug_string_inner(
                        c,
                        !show_segment_entrypoint && Some((sidx, Some(cidx))) == entrypoint,
                        if cidx + 1 != s.commits.len() {
                            None
                        } else {
                            self.stop_condition(sidx)
                        },
                        self.hard_limit_hit,
                        max_goals,
                        show_owned_by_repo,
                    )
                })
                .collect::<Vec<_>>()
                .join("\\l");
            let max_dot_label_size = 16384 - 384 /* safety buffer for everything else in the label */;
            if commits.len() > max_dot_label_size {
                let new_len = commits
                    .char_indices()
                    .rev()
                    .find(|(idx, _)| *idx < max_dot_label_size)
                    .expect("there must be one")
                    .0;
                let cut = commits.len() - new_len;
                commits.truncate(new_len);
                commits.push_str(&format!("[{cut} bytes cut]…\\l"));
            }
            format!(
                ", shape = box, label = \"{entrypoint}{meta}:{id}[{generation}]:{name}{commits}\\l\", fontname = Courier, margin = 0.2",
                meta = match s.metadata {
                    None => {
                        ""
                    }
                    Some(SegmentMetadata::Workspace(_)) => {
                        "📕"
                    }
                    Some(SegmentMetadata::Branch(_)) => {
                        "📙"
                    }
                },
                entrypoint = if show_segment_entrypoint { "👉" } else { "" },
                id = sidx.index(),
                generation = s.generation,
            )
        };

        let edge_attrs = &|g: &PetGraph, e: EdgeReference<'_, Edge>| {
            let src = &g[e.source()];
            let dst = &g[e.target()];
            // Graphs may be half-baked, let's not worry about it then.
            if self.hard_limit_hit {
                return ", label = \"\"".into();
            }
            // Don't mark connections from the last commit to the first one,
            // but those that are 'splitting' a segment. These shouldn't exist.
            let Err(err) = Self::check_edge(g, e, true) else {
                return ", label = \"\"".into();
            };
            let e = e.weight();
            let src = src
                .commit_id_by_index(e.src)
                .map(|c| c.to_hex_with_len(HEX).to_string())
                .unwrap_or_else(|| "src".into());
            let dst = dst
                .commit_id_by_index(e.dst)
                .map(|c| c.to_hex_with_len(HEX).to_string())
                .unwrap_or_else(|| "dst".into());
            format!(", label = \"⚠{src} → {dst} ({err})\", fontname = Courier")
        };
        let dot = petgraph::dot::Dot::with_attr_getters(&self.inner, &[], &edge_attrs, &node_attrs);
        format!("{dot:?}")
    }

    // WARNING: should only be run on a fresh clone as it probably leaves the graph unusable.
    fn prune_for_dot_graph(&mut self) {
        let lower_bound_segment_id = self
            .to_workspace_state(crate::workspace::workspace::Downgrade::Allow)
            .ok()
            .and_then(|state| state.lower_bound_segment_id);
        if let Some(lower_bound_segment_id) = lower_bound_segment_id {
            self.remove_in_workspace_flag_below_lower_bound(lower_bound_segment_id);
        }

        // It's OK if it takes a while, prefer complete graphs.
        const LIMIT: usize = 5000;
        let mut to_remove = self.num_segments().saturating_sub(LIMIT);
        if to_remove > 0 {
            tracing::warn!(
                "Pruning at most {to_remove} nodes from the bottom to assure 'dot' won't hang",
            );
            let mut next = VecDeque::new();
            next.extend(self.base_segments());
            let mut seen = self.seen_table();
            while let Some(sidx) = next.pop_front() {
                if to_remove == 0 {
                    break;
                }
                if let Some(s) = self.node_weight(sidx) {
                    if lower_bound_segment_id.is_some()
                        && s.non_empty_flags_of_first_commit()
                            .is_some_and(|flags| flags.contains(CommitFlags::InWorkspace))
                    {
                        continue;
                    }
                    if s.metadata.is_some()
                        || s.sibling_segment_id.is_some()
                        || s.remote_tracking_branch_segment_id.is_some()
                    {
                        continue;
                    }
                }
                next.extend(
                    self.neighbors_directed(sidx, petgraph::Direction::Incoming)
                        .filter(|n| seen.insert_unseen(*n)),
                );
                self.remove_node(sidx);
                to_remove -= 1;
            }
            if to_remove != 0 {
                tracing::warn!(
                    "{to_remove} extra nodes were kept to keep vital portions of the graph"
                );
            }
            self.set_hard_limit_hit();
        }
    }

    fn remove_in_workspace_flag_below_lower_bound(&mut self, lower_bound_segment_id: SegmentIndex) {
        let mut seen = self.seen_table();
        seen.insert_unseen(lower_bound_segment_id);
        let mut queue = VecDeque::from([lower_bound_segment_id]);
        while let Some(sidx) = queue.pop_front() {
            let below_segments: Vec<_> = self
                .neighbors_directed(sidx, petgraph::Direction::Outgoing)
                .filter(|n| seen.insert_unseen(*n))
                .collect();
            for below_sidx in below_segments {
                if let Some(segment) = self.node_weight_mut(below_sidx)
                    && let Some(first_commit) = segment.commits.first_mut()
                {
                    first_commit.flags.remove(CommitFlags::InWorkspace);
                }
                queue.push_back(below_sidx);
            }
        }
    }
}

use crate::init::PetGraph;
use crate::{Edge, Graph, Segment, SegmentIndex, SegmentMetadata};
use anyhow::{Context, bail};
use bstr::{BString, ByteSlice, ByteVec};
use gix::reference::Category;
use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::EdgeReference;
use petgraph::visit::IntoEdgeReferences;
use std::collections::BTreeMap;

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
                        .or_insert_with(|| format!("remote-{}", num_remotes).into());
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
            if let Some(rn) = node.ref_name.as_mut() {
                anon(rn)?;
            }
            if let Some(rn) = node.remote_tracking_ref_name.as_mut() {
                anon(rn)?;
            }
            for rn in node.commits.iter_mut().flat_map(|c| c.refs.iter_mut()) {
                anon(rn)?;
            }
            if let Some(SegmentMetadata::Workspace(md)) = node.metadata.as_mut() {
                if let Some(rn) = md.target_ref.as_mut() {
                    anon(rn)?;
                }
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
        is_early_end: bool,
        hard_limit: bool,
    ) -> String {
        format!(
            "{ep}{end}{kind}{hex}{flags}{refs}",
            ep = if is_entrypoint { "👉" } else { "" },
            end = if is_early_end {
                if hard_limit { "❌" } else { "✂️" }
            } else {
                ""
            },
            kind = if commit.flags.is_remote() {
                "🟣"
            } else {
                "·"
            },
            flags = if !commit.flags.is_empty() {
                format!(" ({})", commit.flags.debug_string())
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
                        .map(|rn| format!("►{}", { Self::ref_debug_string(rn) }))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )
    }

    /// Shorten the given `name` so it's still clear if it is a special ref (like tag) or not.
    pub fn ref_debug_string(name: &gix::refs::FullName) -> String {
        let (cat, sn) = name.category_and_short_name().expect("valid refs");
        // Only shorten those that look good and are unambiguous enough.
        if matches!(cat, Category::LocalBranch | Category::RemoteBranch) {
            sn
        } else {
            name.as_bstr()
                .strip_prefix(b"refs/")
                .map(|n| n.as_bstr())
                .unwrap_or(name.as_bstr())
        }
        .to_string()
    }

    /// Return a useful one-line string showing the relationship between `ref_name`, `remote_ref_name` and how
    /// they are linked with `sibling_id`.
    pub fn ref_and_remote_debug_string(
        ref_name: Option<&gix::refs::FullName>,
        remote_ref_name: Option<&gix::refs::FullName>,
        sibling_id: Option<SegmentIndex>,
    ) -> String {
        format!(
            "{ref_name}{remote}",
            ref_name = ref_name
                .as_ref()
                .map(|rn| format!(
                    "{}{maybe_id}",
                    Graph::ref_debug_string(rn),
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
                    remote_name = Graph::ref_debug_string(remote_ref_name),
                    maybe_id = sibling_id
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
        let dot = self.dot_graph();
        eprintln!("{dot}");
    }

    /// Open an SVG dot visualization in the browser or panic if the `dot` or `open` tool can't be found.
    #[cfg(unix)]
    #[tracing::instrument(skip(self))]
    pub fn open_as_svg(&self) {
        use std::io::Write;
        use std::process::Stdio;
        use std::sync::atomic::AtomicUsize;

        static SUFFIX: AtomicUsize = AtomicUsize::new(0);
        let suffix = SUFFIX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let svg_name = format!("debug-graph-{suffix:02}.svg");
        let mut dot = std::process::Command::new("dot")
            .args(["-Tsvg", "-o", &svg_name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("'dot' (graphviz) must be installed on the system");
        dot.stdin
            .as_mut()
            .unwrap()
            .write_all(self.dot_graph().as_bytes())
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
                .arg(&svg_name)
                .status()
                .unwrap()
                .success(),
            "Opening of {svg_name} failed"
        );
    }

    /// Produces a dot-version of the graph.
    pub fn dot_graph(&self) -> String {
        const HEX: usize = 7;
        let entrypoint = self.entrypoint;
        let node_attrs = |_: &PetGraph, (sidx, s): (SegmentIndex, &Segment)| {
            let name = format!(
                "{ref_name_and_remote}{maybe_centering_newline}",
                ref_name_and_remote = Self::ref_and_remote_debug_string(
                    s.ref_name.as_ref(),
                    s.remote_tracking_ref_name.as_ref(),
                    s.sibling_segment_id
                ),
                maybe_centering_newline = if s.commits.is_empty() { "" } else { "\n" },
            );
            // Reduce noise by preferring ref-based entry-points.
            let show_segment_entrypoint = s.ref_name.is_some()
                && entrypoint.is_some_and(|(s, cidx)| s == sidx && matches!(cidx, None | Some(0)));
            let mut commits = s
                .commits
                .iter()
                .enumerate()
                .map(|(cidx, c)| {
                    Self::commit_debug_string(
                        c,
                        !show_segment_entrypoint && Some((sidx, Some(cidx))) == entrypoint,
                        if cidx + 1 != s.commits.len() {
                            false
                        } else {
                            self.is_early_end_of_traversal(sidx)
                        },
                        self.hard_limit_hit,
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
            format!(", label = \"⚠️{src} → {dst} ({err})\", fontname = Courier")
        };
        let dot = petgraph::dot::Dot::with_attr_getters(&self.inner, &[], &edge_attrs, &node_attrs);
        format!("{dot:?}")
    }
}

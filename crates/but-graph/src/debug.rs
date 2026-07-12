use std::collections::BTreeMap;

use anyhow::{Context as _, bail};
use bstr::{BString, ByteSlice, ByteVec};
use gix::reference::Category;

use crate::StopCondition;

/// Debugging
impl crate::Workspace {
    /// Assure that no PII is left, by deterministically anonymizing branch names everywhere
    /// the workspace surfaces them: the projected stacks, the carried commit graph's refs,
    /// the target, the captured tip/entrypoint facts, and workspace metadata. Commit
    /// information (flags, hashes) is kept. Consuming on purpose: the anonymized workspace
    /// is a different artifact for display, and the falsified original must not linger.
    ///
    /// Use `remotes` to know how to separate the remote name from the branch name of a
    /// short name.
    pub fn anonymized(mut self, remotes: &gix::remote::Names) -> anyhow::Result<Self> {
        let mut anon = RefAnonymizer::new(remotes);
        // Rewrite the SOURCES — public fact fields, context, frame, and the graph itself.
        // Every derivation (the segment graph below, the display per call) re-derives from
        // them, so anonymized names flow to renderers. Note that `ctx.project_meta` and
        // `ctx.symbolic_remote_names` keep their original names.
        if let Some(t) = self.target_ref.as_mut() {
            anon.apply(&mut t.ref_name)?;
        }
        if let Some(rn) = self.frame.entry_ref.as_mut() {
            anon.apply(rn)?;
        }
        if let Some(md) = self.metadata.as_mut() {
            for rn in md
                .stacks
                .iter_mut()
                .flat_map(|s| s.branches.iter_mut().map(|b| &mut b.ref_name))
            {
                anon.apply(rn)?;
            }
        }
        // Alias in sorted-key order: hash-map iteration would hand out aliases in a
        // different order every process.
        let mut remote_tracking: Vec<_> = std::mem::take(&mut self.ctx.remote_tracking)
            .into_iter()
            .collect();
        remote_tracking.sort();
        self.ctx.remote_tracking = remote_tracking
            .into_iter()
            .map(|(mut local, mut remote)| {
                anon.apply(&mut local)?;
                anon.apply(&mut remote)?;
                Ok((local, remote))
            })
            .collect::<anyhow::Result<_>>()?;
        let mut branch_details: Vec<_> = std::mem::take(&mut self.ctx.branch_details)
            .into_iter()
            .collect();
        branch_details.sort_by(|(a, _), (b, _)| a.cmp(b));
        self.ctx.branch_details = branch_details
            .into_iter()
            .map(|(mut name, details)| {
                anon.apply(&mut name)?;
                Ok((name, details))
            })
            .collect::<anyhow::Result<_>>()?;
        for rn in self.ctx.ad_hoc_branch_stack_orders.iter_mut().flatten() {
            anon.apply(rn)?;
        }
        if let Some(wm) = self.ctx.workspace_meta.as_mut() {
            anon.apply(&mut wm.ref_name)?;
            for rn in wm
                .metadata
                .stacks
                .iter_mut()
                .flat_map(|s| s.branches.iter_mut().map(|b| &mut b.ref_name))
            {
                anon.apply(rn)?;
            }
        }
        // The FRAME mirrors the public fields and feeds the derivations verbatim.
        match &mut self.frame.kind {
            crate::workspace::WorkspaceKind::Managed { ref_info }
            | crate::workspace::WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info, .. } => {
                anon.apply(&mut ref_info.ref_name)?;
            }
            crate::workspace::WorkspaceKind::AdHoc => {}
        }
        if let Some(md) = self.frame.metadata.as_mut() {
            for rn in md
                .stacks
                .iter_mut()
                .flat_map(|s| s.branches.iter_mut().map(|b| &mut b.ref_name))
            {
                anon.apply(rn)?;
            }
        }
        if let Some(ri) = self.frame.tip_ref_info.as_mut() {
            anon.apply(&mut ri.ref_name)?;
        }
        if let Some(rn) = self.frame.entry_ref.as_mut() {
            anon.apply(rn)?;
        }
        if let Some(rn) = self.frame.lower_bound_ref_name.as_mut() {
            anon.apply(rn)?;
        }
        if let Some(t) = self.frame.target_ref.as_mut() {
            anon.apply(&mut t.name)?;
        }
        self.commit_graph.anonymize_refs(&mut anon)?;
        // Re-derive the segment graph from the anonymized sources — coherent by construction
        // with what the display will re-derive.
        self.stacks = crate::workspace::reduce_to_segment_stacks(&self.derive_stacks());
        Ok(self)
    }
}

/// Deterministic branch-name anonymization: local branches and tags become alphabetic
/// aliases, remote-tracking branches keep their remote/name split with both parts
/// aliased. `gitbutler/` references are kept. The same input always maps to the same
/// alias within one anonymizer.
pub(crate) struct RefAnonymizer {
    remotes: Vec<BString>,
    remote_mapping: BTreeMap<BString, BString>,
    name_mapping: BTreeMap<BString, BString>,
}

impl RefAnonymizer {
    pub(crate) fn new(remotes: &gix::remote::Names) -> Self {
        RefAnonymizer {
            remotes: remotes.iter().cloned().collect(),
            remote_mapping: BTreeMap::new(),
            name_mapping: BTreeMap::new(),
        }
    }

    pub(crate) fn apply(&mut self, rn: &mut gix::refs::FullName) -> anyhow::Result<()> {
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

        let (category, short_name) = rn
            .category_and_short_name()
            .with_context(|| format!("Couldn't classify reference '{rn}'"))?;
        // `gitbutler/*` refs are KEPT, as documented: they are implementation refs, not user
        // branches, and the projection reads their namespace to tell a workspace view from a
        // plain checkout. Renaming them made an anonymized graph project differently from the
        // graph it was anonymized from.
        if matches!(category, Category::LocalBranch)
            && crate::ref_layout::in_gitbutler_namespace(rn.as_ref())
        {
            return Ok(());
        }
        match category {
            Category::Tag | Category::LocalBranch => {
                // 1-indexed: int_to_alpha(0) and (1) both yield "A".
                let num_names = self.name_mapping.len() + 1;
                let new_name = self
                    .name_mapping
                    .entry(short_name.to_owned())
                    .or_insert_with(|| int_to_alpha(num_names).into());
                *rn = category.to_full_name(new_name.as_bstr())?;
            }
            Category::RemoteBranch => {
                let (remote_name, short_name) = self
                    .remotes
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

                let num_names = self.name_mapping.len() + 1;
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

impl crate::CommitGraph {
    /// Apply `anon` to every reference attached to commits, plus the entrypoint ref, the
    /// seeds, and the stored layout, rebuilding the ref lookup afterwards.
    pub(crate) fn anonymize_refs(&mut self, anon: &mut RefAnonymizer) -> anyhow::Result<()> {
        let ids: Vec<_> = self.commit_ids().collect();
        for id in ids {
            let Some(refs) = self.commit_refs_mut(id) else {
                continue;
            };
            for ri in refs {
                anon.apply(&mut ri.ref_name)?;
            }
        }
        if let Some(rn) = self.entrypoint_ref_mut() {
            anon.apply(rn)?;
        }
        for seed in &mut self.seeds {
            if let Some(rn) = seed.ref_name.as_mut() {
                anon.apply(rn)?;
            }
            if let Some(crate::SegmentMetadata::Workspace(ws)) = seed.metadata.as_mut() {
                for rn in ws
                    .stacks
                    .iter_mut()
                    .flat_map(|s| s.branches.iter_mut().map(|b| &mut b.ref_name))
                {
                    anon.apply(rn)?;
                }
            }
        }
        // The stored layout speaks names throughout — groups, facts, head refs, and the
        // declared stack partition all feed re-derivations and must alias coherently.
        if let Some(layout) = self.layout.as_mut() {
            for group in layout.groups.groups_mut() {
                for rn in &mut group.members {
                    anon.apply(rn)?;
                }
                if let Some(rn) = group.attach.as_mut() {
                    anon.apply(rn)?;
                }
            }
            for (rn, _) in &mut layout.facts {
                anon.apply(rn)?;
            }
            for rn in &mut layout.head_refs {
                anon.apply(rn)?;
            }
            for rn in layout.stacks.iter_mut().flat_map(|s| &mut s.branches) {
                anon.apply(rn)?;
            }
        }
        self.rebuild_by_ref();
        Ok(())
    }
}

/// Debugging
pub(crate) fn commit_debug_string_inner(
    commit: &crate::Commit,
    is_entrypoint: bool,
    stop_condition: Option<StopCondition>,
    hard_limit: bool,
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
                    .map(|ri| format!("►{}", {
                        ref_debug_string_inner(
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
pub(crate) fn ref_debug_string(
    ref_name: &gix::refs::FullNameRef,
    worktree: Option<&crate::Worktree>,
) -> String {
    ref_debug_string_inner(ref_name, worktree, false)
}

pub(crate) fn ref_debug_string_inner(
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
pub(crate) fn ref_and_remote_debug_string(
    ref_info: Option<&crate::RefInfo>,
    remote_ref_name: Option<&gix::refs::FullName>,
    sibling_id: Option<usize>,
    remote_tracking_branch_id: Option<usize>,
) -> String {
    ref_and_remote_debug_string_inner(
        ref_info,
        remote_ref_name,
        sibling_id,
        remote_tracking_branch_id,
        false,
    )
}

pub(crate) fn ref_and_remote_debug_string_inner(
    ref_info: Option<&crate::RefInfo>,
    remote_ref_name: Option<&gix::refs::FullName>,
    sibling_id: Option<usize>,
    remote_tracking_branch_id: Option<usize>,
    show_owned_by_repo: bool,
) -> String {
    format!(
        "{ref_name}{remote}",
        ref_name = ref_info
            .as_ref()
            .map(|ri| format!(
                "{}{maybe_id}",
                ref_debug_string_inner(
                    ri.ref_name.as_ref(),
                    ri.worktree.as_ref(),
                    show_owned_by_repo,
                ),
                maybe_id = sibling_id
                    .filter(|_| remote_ref_name.is_none())
                    .map(|id| format!(" →:{id}:"))
                    .unwrap_or_default()
            ))
            .unwrap_or_else(|| format!(
                "anon:{maybe_id}",
                maybe_id = sibling_id.map(|id| format!(" →:{id}:")).unwrap_or_default()
            )),
        remote = remote_ref_name
            .as_ref()
            .map(|remote_ref_name| format!(
                " <> {remote_name}{maybe_id}",
                remote_name = ref_debug_string(remote_ref_name.as_ref(), None),
                maybe_id = remote_tracking_branch_id
                    .or(sibling_id)
                    .map(|id| format!(" →:{id}:"))
                    .unwrap_or_default()
            ))
            .unwrap_or_default()
    )
}

/// Render `dot` with Graphviz into an SVG next to the manifest and open it, or panic if
/// the `dot` or `open` tool can't be found.
#[cfg(unix)]
pub(crate) fn open_dot_as_svg(dot_document: &str) {
    {
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
            .write_all(dot_document.as_bytes())
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
}

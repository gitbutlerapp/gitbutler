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
        for stack in &mut self.stacks {
            for segment in &mut stack.segments {
                if let Some(ri) = segment.ref_info.as_mut() {
                    anon.apply(&mut ri.ref_name)?;
                }
                if let Some(rn) = segment.remote_tracking_ref_name.as_mut() {
                    anon.apply(rn)?;
                }
                for ri in segment
                    .commits
                    .iter_mut()
                    .flat_map(|c| c.refs.iter_mut())
                    .chain(
                        segment
                            .commits_on_remote
                            .iter_mut()
                            .flat_map(|c| c.refs.iter_mut()),
                    )
                {
                    anon.apply(&mut ri.ref_name)?;
                }
            }
        }
        match &mut self.kind {
            crate::workspace::WorkspaceKind::Managed { ref_info }
            | crate::workspace::WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => {
                anon.apply(&mut ref_info.ref_name)?;
            }
            crate::workspace::WorkspaceKind::AdHoc => {}
        }
        if let Some(t) = self.target_ref.as_mut() {
            anon.apply(&mut t.ref_name)?;
        }
        if let Some(ri) = self.tip_ref_info.as_mut() {
            anon.apply(&mut ri.ref_name)?;
        }
        if let Some(rn) = self.lower_bound_ref_name.as_mut() {
            anon.apply(rn)?;
        }
        if let Some(ep) = self.entrypoint.as_mut()
            && let Some(rn) = ep.ref_name.as_mut()
        {
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
        self.ctx.remote_tracking = std::mem::take(&mut self.ctx.remote_tracking)
            .into_iter()
            .map(|(mut local, mut remote)| {
                anon.apply(&mut local)?;
                anon.apply(&mut remote)?;
                Ok((local, remote))
            })
            .collect::<anyhow::Result<_>>()?;
        self.commit_graph.anonymize_refs(&mut anon)?;
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
            remotes: remotes
                .iter()
                .map(|r| r.as_ref().as_bstr().to_owned())
                .collect(),
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
    /// Apply `anon` to every reference attached to commits, plus the entrypoint ref,
    /// rebuilding the ref lookup afterwards. The layout is NOT rewritten — an anonymized
    /// graph is a display artifact, not an editing substrate.
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

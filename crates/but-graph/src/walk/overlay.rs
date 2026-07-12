//! Every repository and metadata read the walk performs, with unwritten state merged
//! in. [`OverlayRepo`] answers reference lookups and listings with the
//! [`Overlay`](crate::walk::Overlay)'s extra and dropped references applied, and
//! memoizes the expensive enumerations (worktree branches, prefixed raw refs) so one
//! walk reads the disk once; [`OverlayMetadata`] layers branch/workspace metadata
//! overrides the same way. The walk and build touch the repository ONLY through these
//! two — which is what lets an edit preview re-run the identical code over a
//! hypothetical world.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashSet},
};

use anyhow::bail;
use but_core::{RefMetadata, ref_metadata};
use gix::{prelude::ReferenceExt, refs::Target};

use crate::{
    Worktree, WorktreeKind,
    walk::{
        Entrypoint, Overlay,
        utils::{RefsById, WorktreeByBranch},
    },
};

impl Overlay {
    /// Serve `refs` from memory as if they existed — but only where no real reference with
    /// that name exists (real refs win).
    pub fn with_references_if_new(
        mut self,
        refs: impl IntoIterator<Item = gix::refs::Reference>,
    ) -> Self {
        self.nonoverriding_references = refs.into_iter().collect();
        self
    }

    /// Serve `refs` from memory, overriding any same-named reference in the repository — as if
    /// the reference had been created or set to this value.
    pub fn with_references(mut self, refs: impl IntoIterator<Item = gix::refs::Reference>) -> Self {
        self.overriding_references.extend(refs);
        self
    }

    /// A list of references that should not be picked up anymore in the
    /// re-traversal.
    ///
    /// For example, if the `but_rebase::graph_rebase::Editor` converts a
    /// `Reference` step to a `None` step which is the equivalent of running
    /// `git update-ref -d`, it should no longer be part of the
    /// `Graph`, so we would list the particular reference as a dropped
    /// reference.
    pub fn with_dropped_references(
        mut self,
        refs: impl IntoIterator<Item = gix::refs::FullName>,
    ) -> Self {
        self.dropped_references.extend(refs);
        self
    }

    /// Override the starting position of the traversal by setting it to `id`,
    /// and optionally, by providing the `ref_name` that points to `id`.
    pub fn with_entrypoint(
        mut self,
        id: gix::ObjectId,
        ref_name: Option<gix::refs::FullName>,
    ) -> Self {
        if let Some((_id, ref_name)) = self.entrypoint {
            self.overriding_references
                .retain(|r| Some(&r.name) != ref_name.as_ref())
        }

        if let Some(ref_name) = &ref_name {
            self.overriding_references.push(gix::refs::Reference {
                name: ref_name.to_owned(),
                target: Target::Object(id),
                peeled: Some(id),
            })
        }
        self.entrypoint = Some((id, ref_name));
        self
    }

    /// Serve the given `branches` metadata from memory, as if they existed,
    /// possibly overriding metadata of a ref that already exists.
    pub fn with_branch_metadata_override(
        mut self,
        refs: impl IntoIterator<Item = (gix::refs::FullName, ref_metadata::Branch)>,
    ) -> Self {
        self.meta_branches = refs.into_iter().collect();
        self
    }

    /// Serve the given workspace `metadata` from memory, as if they existed,
    /// possibly overriding metadata of a workspace at that place
    pub fn with_workspace_metadata_override(
        mut self,
        metadata: Option<(gix::refs::FullName, ref_metadata::Workspace)>,
    ) -> Self {
        self.workspace = metadata;
        self
    }

    /// Serve the given ad-hoc branch stack order from memory.
    pub fn with_branch_stack_order_override(
        mut self,
        branches: impl IntoIterator<Item = gix::refs::FullName>,
    ) -> Self {
        self.branch_stack_orders
            .push(branches.into_iter().collect());
        self
    }
}

impl Overlay {
    pub(crate) fn into_parts<'repo, 'meta, T>(
        self,
        repo: &'repo gix::Repository,
        meta: &'meta T,
    ) -> (OverlayRepo<'repo>, OverlayMetadata<'meta, T>, Entrypoint)
    where
        T: RefMetadata,
    {
        let Overlay {
            nonoverriding_references,
            overriding_references,
            dropped_references,
            meta_branches,
            branch_stack_orders,
            workspace,
            entrypoint,
        } = self;
        // Deterministic order: the first mention of a name wins.
        fn first_wins_by_name(refs: Vec<gix::refs::Reference>) -> NameToReference {
            let mut map = NameToReference::new();
            for reference in refs {
                map.entry(reference.name.clone()).or_insert(reference);
            }
            map
        }

        (
            OverlayRepo {
                nonoverriding_references: first_wins_by_name(nonoverriding_references),
                overriding_references: first_wins_by_name(overriding_references),
                dropped_references: dropped_references.into_iter().collect(),
                inner: repo,
                worktree_branches_by_referent: Default::default(),
                raw_refs_by_prefix: Default::default(),
            },
            OverlayMetadata {
                inner: meta,
                meta_branches,
                branch_stack_orders,
                workspace,
            },
            entrypoint,
        )
    }
}

type NameToReference = BTreeMap<gix::refs::FullName, gix::refs::Reference>;

pub(crate) struct OverlayRepo<'repo> {
    inner: &'repo gix::Repository,
    nonoverriding_references: NameToReference,
    overriding_references: NameToReference,
    dropped_references: BTreeSet<gix::refs::FullName>,
    /// [`Self::worktree_branches`] answers, per referent asked for — enumerating worktrees
    /// opens a repository PER linked worktree, and one build asks several times.
    worktree_branches_by_referent:
        std::cell::RefCell<BTreeMap<Option<gix::refs::FullName>, WorktreeByBranch>>,
    /// [`Self::raw_refs_prefixed`] scans, per prefix — loose-ref-heavy namespaces make the
    /// iteration expensive, and both the walker and the builder's input gathering consume it.
    raw_refs_by_prefix: std::cell::RefCell<BTreeMap<String, RawRefs>>,
}

/// One cached namespace scan: each reference with its direct target id (`None` = symbolic,
/// e.g. `origin/HEAD`).
pub(crate) type RawRefs = std::rc::Rc<Vec<(Option<gix::ObjectId>, gix::refs::FullName)>>;

/// Functions returning `'repo` values hand out the bare repository; callers must not use it in
/// ways that bypass the overlay.
impl<'repo> OverlayRepo<'repo> {
    pub fn commit_graph_if_enabled(&self) -> anyhow::Result<Option<gix::commitgraph::Graph>> {
        Ok(self.inner.commit_graph_if_enabled()?)
    }

    pub fn shallow_commits(
        &self,
    ) -> Result<Option<gix::shallow::Commits>, gix::shallow::read::Error> {
        self.inner.shallow_commits()
    }

    pub fn try_find_reference(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<gix::Reference<'repo>>> {
        if self.dropped_references.contains(ref_name) {
            Ok(None)
        } else if let Some(r) = self.overriding_references.get(ref_name) {
            Ok(Some(r.clone().attach(self.inner)))
        } else if let Some(rn) = self.inner.try_find_reference(ref_name)? {
            Ok(Some(rn))
        } else if let Some(r) = self.nonoverriding_references.get(ref_name) {
            Ok(Some(r.clone().attach(self.inner)))
        } else {
            Ok(None)
        }
    }

    pub fn find_reference(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<gix::Reference<'repo>> {
        if self.dropped_references.contains(ref_name) {
            bail!(
                "Failed to find reference {ref_name} due to it being dropped in the traversal overlay"
            );
        }
        if let Some(r) = self.overriding_references.get(ref_name) {
            return Ok(r.clone().attach(self.inner));
        }
        Ok(self
            .inner
            .find_reference(ref_name)
            .or_else(|err| match err {
                gix::reference::find::existing::Error::Find(_) => Err(err),
                gix::reference::find::existing::Error::NotFound { .. } => {
                    if let Some(r) = self.nonoverriding_references.get(ref_name) {
                        Ok(r.clone().attach(self.inner))
                    } else {
                        Err(err)
                    }
                }
            })?)
    }

    pub fn config_snapshot(&self) -> gix::config::Snapshot<'repo> {
        self.inner.config_snapshot()
    }

    pub fn branch_remote_tracking_ref_name(
        &self,
        name: &gix::refs::FullNameRef,
        direction: gix::remote::Direction,
    ) -> Option<
        Result<
            Cow<'_, gix::refs::FullNameRef>,
            gix::repository::branch_remote_tracking_ref_name::Error,
        >,
    > {
        self.inner.branch_remote_tracking_ref_name(name, direction)
    }

    pub fn find_commit(&self, id: gix::ObjectId) -> anyhow::Result<gix::Commit<'repo>> {
        Ok(self.inner.find_commit(id)?)
    }

    pub(crate) fn for_attach_only(&self) -> &'repo gix::Repository {
        self.inner
    }

    pub(crate) fn for_find_only(&self) -> &'repo gix::Repository {
        self.inner
    }

    pub fn remote_names(&self) -> gix::remote::Names<'repo> {
        self.inner.remote_names()
    }

    pub fn upstream_branch_and_remote_for_tracking_branch(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<(gix::refs::FullName, gix::Remote<'repo>)>> {
        Ok(self
            .inner
            .upstream_branch_and_remote_for_tracking_branch(name)?)
    }

    /// Create a mapping of all heads to the object ids they point to.
    /// `workspace_ref_names` is the names of all known workspace references.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn collect_ref_mapping_by_prefix<'a>(
        &self,
        prefixes: impl Iterator<Item = &'a str>,
        workspace_ref_names: &[&gix::refs::FullNameRef],
    ) -> anyhow::Result<RefsById> {
        let mut seen = HashSet::new();
        let ref_filter = |seen: &mut HashSet<gix::refs::FullName>,
                          r: gix::Reference<'_>|
         -> Option<(gix::ObjectId, gix::refs::FullName)> {
            if self.dropped_references.contains(r.name()) {
                return None;
            }

            if workspace_ref_names.contains(&r.name()) {
                return None;
            }
            let id = r.try_id()?;
            let (id, name) = if matches!(r.name().category(), Some(gix::reference::Category::Tag)) {
                // TODO: also make use of the tag name (the tag object has its own name)
                (id.object().ok()?.peel_tags_to_end().ok()?.id, r.inner.name)
            } else {
                (id.detach(), r.inner.name)
            };
            // This is only for overrides.
            seen.insert(name.clone()).then_some((id, name))
        };
        let mut all_refs_by_id = gix::hashtable::HashMap::<_, Vec<_>>::default();
        for prefix in prefixes {
            // apply overrides - they are seen first and take the spot of everything.
            for (commit_id, git_reference) in self
                .overriding_references
                .values()
                .filter(|rn| rn.name.as_bstr().starts_with(prefix.as_bytes()))
                .filter_map(|rn| ref_filter(&mut seen, rn.clone().attach(self.inner)))
            {
                all_refs_by_id
                    .entry(commit_id)
                    .or_default()
                    .push(git_reference);
            }
            if prefix == "refs/tags/" {
                // Tags peel through their tag object — that needs the live reference,
                // so this namespace stays uncached (it is opt-in and small).
                for (commit_id, git_reference) in self
                    .inner
                    .references()?
                    .prefixed(prefix)?
                    .filter_map(Result::ok)
                    .filter_map(|r| ref_filter(&mut seen, r))
                {
                    all_refs_by_id
                        .entry(commit_id)
                        .or_default()
                        .push(git_reference);
                }
            } else {
                for (id, name) in self.raw_refs_prefixed(prefix)?.iter() {
                    if self.dropped_references.contains(name)
                        || workspace_ref_names.contains(&name.as_ref())
                    {
                        continue;
                    }
                    // Symbolic refs (e.g. `origin/HEAD`) have no direct target.
                    let Some(id) = id else { continue };
                    if !seen.insert(name.clone()) {
                        continue;
                    }
                    all_refs_by_id.entry(*id).or_default().push(name.clone());
                }
            }
            // apply overrides (new only)
            for (commit_id, git_reference) in self
                .nonoverriding_references
                .values()
                .filter(|rn| rn.name.as_bstr().starts_with(prefix.as_bytes()))
                .filter_map(|rn| ref_filter(&mut seen, rn.clone().attach(self.inner)))
            {
                all_refs_by_id
                    .entry(commit_id)
                    .or_default()
                    .push(git_reference);
            }
        }
        all_refs_by_id.values_mut().for_each(|v| v.sort());
        Ok(all_refs_by_id)
    }

    /// The RAW (overlay-free) references under `prefix` with their direct target ids
    /// (`None` = symbolic, e.g. `origin/HEAD`), scanned once and cached — consumers apply
    /// their own overlay filtering, exactly as they did over a live iteration.
    #[tracing::instrument(level = "trace", skip_all, fields(prefix = prefix))]
    pub(crate) fn raw_refs_prefixed(&self, prefix: &str) -> anyhow::Result<RawRefs> {
        if let Some(hit) = self.raw_refs_by_prefix.borrow().get(prefix) {
            return Ok(hit.clone());
        }
        let scanned = match scan_refs_fast(self.inner, prefix) {
            Some(mut scanned) => {
                // Determinism, not semantics: every consumer treats the scan as a set.
                scanned.sort_by(|a, b| a.1.cmp(&b.1));
                #[cfg(debug_assertions)]
                {
                    let mut live = live_refs_prefixed(self.inner, prefix)?;
                    live.sort_by(|a, b| a.1.cmp(&b.1));
                    debug_assert_eq!(
                        scanned, live,
                        "BUG: the parallel loose-ref scan must equal gix's live iteration"
                    );
                }
                scanned
            }
            None => live_refs_prefixed(self.inner, prefix)?,
        };
        let scanned = std::rc::Rc::new(scanned);
        self.raw_refs_by_prefix
            .borrow_mut()
            .insert(prefix.to_string(), scanned.clone());
        Ok(scanned)
    }

    /// Map each ref checked out by a worktree (each `HEAD` and its ref chain) to that worktree.
    /// `main_head_referent` substitutes the main worktree's HEAD referent — used when an
    /// overlay pretends HEAD is elsewhere; the worktree ref on disk is unaffected.
    ///
    /// ### Shortcoming
    ///
    /// Only the first HEAD reference can be remapped — proper in-memory overrides would be
    /// needed for more. Callers must not derive `main_head_referent` from an entrypoint that
    /// isn't really HEAD; nothing enforces this.
    #[tracing::instrument(level = "trace", skip_all, fields(referent = ?main_head_referent))]
    pub fn worktree_branches(
        &self,
        main_head_referent: Option<&gix::refs::FullNameRef>,
    ) -> anyhow::Result<WorktreeByBranch> {
        let key = main_head_referent.map(|r| r.to_owned());
        if let Some(cached) = self.worktree_branches_by_referent.borrow().get(&key) {
            return Ok(cached.clone());
        }
        let computed = self.worktree_branches_uncached(main_head_referent)?;
        self.worktree_branches_by_referent
            .borrow_mut()
            .insert(key, computed.clone());
        Ok(computed)
    }

    fn worktree_branches_uncached(
        &self,
        main_head_referent: Option<&gix::refs::FullNameRef>,
    ) -> anyhow::Result<WorktreeByBranch> {
        /// If `main_head_referent` is set, it means this is an overridden reference of the `HEAD` of the repo the graph is built in.
        /// If `None`, `head` belongs to another worktree. Completely unrelated to linked or main.
        fn maybe_insert_head(
            head: Option<gix::Head<'_>>,
            main_head_referent: Option<&gix::refs::FullNameRef>,
            overriding: &NameToReference,
            out: &mut WorktreeByBranch,
            owned_by_repo: bool,
        ) -> anyhow::Result<()> {
            let Some((head, wd)) = head.and_then(|head| {
                head.repo.worktree().map(|wt| {
                    (
                        head,
                        Worktree {
                            kind: match wt.id() {
                                None => WorktreeKind::Main,
                                Some(id) => WorktreeKind::LinkedId(id.to_owned()),
                            },
                            owned_by_repo,
                        },
                    )
                })
            }) else {
                return Ok(());
            };

            out.entry("HEAD".try_into().expect("valid"))
                .or_default()
                .push(wd.clone());
            let mut ref_chain = Vec::new();
            // Is this the repo that the overrides were applied on?
            let mut cursor = if let Some(head_name) = main_head_referent {
                overriding
                    .get(head_name)
                    .map(|overridden_head| overridden_head.clone().attach(head.repo))
                    .or_else(|| head.try_into_referent())
            } else {
                head.try_into_referent()
            };
            while let Some(ref_) = cursor {
                ref_chain.push(ref_.name().to_owned());
                if overriding
                    .get(ref_.name())
                    .is_some_and(|r| r.target.try_name() != ref_.target().try_name())
                {
                    bail!(
                        "SHORTCOMING: cannot deal with {ref_:?} overridden to a different symbolic name to follow"
                    )
                }
                cursor = ref_.follow().transpose()?;
            }
            for name in ref_chain {
                out.entry(name).or_default().push(wd.clone());
            }

            Ok(())
        }

        let mut map = BTreeMap::new();
        maybe_insert_head(
            self.inner.head().ok(),
            main_head_referent,
            &self.overriding_references,
            &mut map,
            true,
        )?;

        let mut repo_is_linked = false;
        let current_dir = std::env::current_dir()?;
        let repo_real_path = gix::path::realpath_opts(
            self.inner.path(),
            &current_dir,
            gix::path::realpath::MAX_SYMLINKS,
        )?;
        for proxy in self.inner.worktrees()? {
            let repo = proxy.into_repo_with_possibly_inaccessible_worktree()?;
            let linked_repo_real_path = gix::path::realpath_opts(
                repo.path(),
                &current_dir,
                gix::path::realpath::MAX_SYMLINKS,
            )?;
            if linked_repo_real_path == repo_real_path {
                repo_is_linked = true;
                continue;
            }
            maybe_insert_head(
                repo.head().ok(),
                None,
                &self.overriding_references,
                &mut map,
                false,
            )?;
        }
        if repo_is_linked && let Ok(main_repo) = self.inner.main_repo() {
            maybe_insert_head(
                main_repo.head().ok(),
                None,
                &self.overriding_references,
                &mut map,
                false,
            )?;
        }
        Ok(map)
    }
}

pub(crate) struct OverlayMetadata<'meta, T> {
    inner: &'meta T,
    meta_branches: Vec<(gix::refs::FullName, ref_metadata::Branch)>,
    branch_stack_orders: Vec<Vec<gix::refs::FullName>>,
    workspace: Option<(gix::refs::FullName, ref_metadata::Workspace)>,
}

impl<T> OverlayMetadata<'_, T>
where
    T: RefMetadata,
{
    pub(crate) fn iter_workspaces(
        &self,
    ) -> impl Iterator<Item = (gix::refs::FullName, ref_metadata::Workspace)> {
        self.inner
            .iter()
            .filter_map(Result::ok)
            .filter_map(|(ref_name, item)| {
                item.downcast::<ref_metadata::Workspace>()
                    .ok()
                    .map(|ws| (ref_name, ws))
            })
            .map(|(ref_name, ws)| {
                if let Some((_ws_ref, ws_override)) = self
                    .workspace
                    .as_ref()
                    .filter(|(ws_ref, _ws_data)| *ws_ref == ref_name)
                {
                    (ref_name, ws_override.clone())
                } else {
                    (ref_name, (*ws).clone())
                }
            })
    }

    pub fn workspace_opt(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<ref_metadata::Workspace>> {
        if let Some((_ws_ref, ws_meta)) = self
            .workspace
            .as_ref()
            .filter(|(ws_ref, _ws_meta)| ws_ref.as_ref() == ref_name)
        {
            return Ok(Some(ws_meta.clone()));
        }
        let opt = self.inner.workspace_opt(ref_name)?;
        Ok(opt.map(|ws_data| ws_data.clone()))
    }

    pub fn branch_opt(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<ref_metadata::Branch>> {
        if let Some(overlay_branch) = self
            .meta_branches
            .iter()
            .find_map(|(rn, branch)| (rn.as_ref() == ref_name).then(|| branch.clone()))
        {
            return Ok(Some(overlay_branch));
        }
        let opt = self.inner.branch_opt(ref_name)?;
        Ok(opt.map(|data| data.clone()))
    }

    pub fn branch_stack_order(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<Vec<gix::refs::FullName>>> {
        if let Some(branches) = self
            .branch_stack_orders
            .iter()
            .find(|branches| branches.iter().any(|branch| branch.as_ref() == ref_name))
        {
            return Ok(Some(branches.clone()));
        }
        self.inner.branch_stack_order(ref_name)
    }

    /// The wrapped metadata WITHOUT the overlay — only for callers that re-apply the overlay
    /// themselves.
    pub(crate) fn for_inner_only(&self) -> &'_ T {
        self.inner
    }
}

/// An owned metadata handle for [`OverlayMetadata`]'s [`RefMetadata`] impl: values are cloned out
/// of the overlay or the underlying store, so this view is read-only.
pub(crate) struct OwnedHandle<V> {
    name: gix::refs::FullName,
    value: V,
    is_default: bool,
}

impl<V> std::ops::Deref for OwnedHandle<V> {
    type Target = V;
    fn deref(&self) -> &V {
        &self.value
    }
}

impl<V> std::ops::DerefMut for OwnedHandle<V> {
    fn deref_mut(&mut self) -> &mut V {
        &mut self.value
    }
}

impl<V> AsRef<gix::refs::FullNameRef> for OwnedHandle<V> {
    fn as_ref(&self) -> &gix::refs::FullNameRef {
        self.name.as_ref()
    }
}

impl<V> ref_metadata::ValueInfo for OwnedHandle<V> {
    fn is_default(&self) -> bool {
        self.is_default
    }
}

/// A read-only [`RefMetadata`] view that substitutes the overlay's workspace and branch overrides,
/// so overlay-aware graph builders can stand in wherever a `RefMetadata` is expected.
impl<T> RefMetadata for OverlayMetadata<'_, T>
where
    T: RefMetadata,
{
    type Handle<V> = OwnedHandle<V>;

    fn iter(
        &self,
    ) -> impl Iterator<Item = anyhow::Result<(gix::refs::FullName, Box<dyn std::any::Any>)>> {
        self.inner.iter().map(|res| {
            res.map(|(name, item)| {
                if item.is::<ref_metadata::Workspace>()
                    && let Some((_, ws)) = self.workspace.as_ref().filter(|(r, _)| *r == name)
                {
                    return (name, Box::new(ws.clone()) as Box<dyn std::any::Any>);
                }
                if item.is::<ref_metadata::Branch>()
                    && let Some((_, b)) = self.meta_branches.iter().find(|(r, _)| *r == name)
                {
                    return (name, Box::new(b.clone()) as Box<dyn std::any::Any>);
                }
                (name, item)
            })
        })
    }

    fn workspace(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Self::Handle<ref_metadata::Workspace>> {
        let value = OverlayMetadata::workspace_opt(self, ref_name)?;
        Ok(OwnedHandle {
            name: ref_name.to_owned(),
            is_default: value.is_none(),
            value: value.unwrap_or_default(),
        })
    }

    fn branch(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Self::Handle<ref_metadata::Branch>> {
        let value = OverlayMetadata::branch_opt(self, ref_name)?;
        Ok(OwnedHandle {
            name: ref_name.to_owned(),
            is_default: value.is_none(),
            value: value.unwrap_or_default(),
        })
    }

    fn set_workspace(
        &mut self,
        _value: &Self::Handle<ref_metadata::Workspace>,
    ) -> anyhow::Result<()> {
        bail!("OverlayMetadata is a read-only view")
    }

    fn set_branch(&mut self, _value: &Self::Handle<ref_metadata::Branch>) -> anyhow::Result<()> {
        bail!("OverlayMetadata is a read-only view")
    }

    fn remove(&mut self, _ref_name: &gix::refs::FullNameRef) -> anyhow::Result<bool> {
        bail!("OverlayMetadata is a read-only view")
    }

    fn rename(
        &mut self,
        _old_ref_name: &gix::refs::FullNameRef,
        _new_ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<()> {
        bail!("OverlayMetadata is a read-only view")
    }

    fn branch_stack_order(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<Vec<gix::refs::FullName>>> {
        OverlayMetadata::branch_stack_order(self, ref_name)
    }
}

/// The reference implementation of the namespace scan: `gix`'s live iteration.
fn live_refs_prefixed(
    repo: &gix::Repository,
    prefix: &str,
) -> anyhow::Result<Vec<(Option<gix::ObjectId>, gix::refs::FullName)>> {
    Ok(repo
        .references()?
        .prefixed(prefix)?
        .filter_map(Result::ok)
        .map(|r| (r.try_id().map(|id| id.detach()), r.inner.name))
        .collect())
}

/// A hand-rolled scan of the `prefix` namespace: enumerate the loose ref files, read them
/// on a small thread pool, then add packed entries the loose refs don't shadow. `gix`'s
/// live iteration reads loose refs strictly one file at a time, which dominates graph
/// builds on repositories with tens of thousands of loose refs (~17µs per file).
///
/// Returns `None` for anything but the plain file store shapes this understands — the
/// caller falls back to the live iteration. Symbolic refs surface as `None` ids and
/// unparseable files are skipped, both exactly like the live iteration; the result is
/// unordered (callers treat it as a set).
fn scan_refs_fast(
    repo: &gix::Repository,
    prefix: &str,
) -> Option<Vec<(Option<gix::ObjectId>, gix::refs::FullName)>> {
    if !prefix.starts_with("refs/") || !prefix.ends_with('/') {
        return None;
    }
    // Loose files first: they shadow packed entries.
    let root = repo.common_dir().join(prefix);
    let mut files = Vec::new();
    if root.is_dir() {
        let _span = tracing::trace_span!("enumerate_loose").entered();
        collect_loose_ref_files(&root, prefix.into(), &mut files).ok()?;
    }
    let parse = |(name, path): (gix::bstr::BString, std::path::PathBuf)| -> Option<(Option<gix::ObjectId>, gix::refs::FullName)> {
        let name = gix::refs::FullName::try_from(name).ok()?;
        let content = std::fs::read(&path).ok()?;
        let line = content
            .split(|&b| b == b'\n')
            .next()
            .unwrap_or_default()
            .trim_ascii_end();
        if line.starts_with(b"ref: ") {
            // Symbolic, like the live iteration's `try_id() == None`.
            return Some((None, name));
        }
        let id = gix::ObjectId::from_hex(line).ok()?;
        Some((Some(id), name))
    };
    let parse_span = tracing::trace_span!("parse_loose", files = files.len()).entered();
    let mut scanned: Vec<(Option<gix::ObjectId>, gix::refs::FullName)> = if files.len() < 128 {
        files.into_iter().filter_map(parse).collect()
    } else {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(8);
        let chunk_size = files.len().div_ceil(threads);
        let mut chunks = Vec::with_capacity(threads);
        let mut rest = files;
        while rest.len() > chunk_size {
            let tail = rest.split_off(chunk_size);
            chunks.push(rest);
            rest = tail;
        }
        chunks.push(rest);
        std::thread::scope(|scope| {
            let handles: Vec<_> = chunks
                .into_iter()
                .map(|chunk| {
                    scope.spawn(move || chunk.into_iter().filter_map(parse).collect::<Vec<_>>())
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("ref parsing does not panic"))
                .collect()
        })
    };
    drop(parse_span);
    // Packed entries not shadowed by a loose ref. A packed-refs read error falls back to
    // the live iteration so errors surface through one path.
    let _span = tracing::trace_span!("merge_packed").entered();
    let loose_names: std::collections::HashSet<&gix::bstr::BStr> =
        scanned.iter().map(|(_, name)| name.as_bstr()).collect();
    let mut packed_entries = Vec::new();
    if let Some(buffer) = repo.refs.cached_packed_buffer().ok()? {
        for record in buffer.iter_prefixed(prefix.into()).ok()? {
            let record = record.ok()?;
            if loose_names.contains(record.name.as_bstr()) {
                continue;
            }
            packed_entries.push((Some(record.target()), record.name.to_owned()));
        }
    }
    drop(loose_names);
    scanned.extend(packed_entries);
    Some(scanned)
}

/// Recursively list the loose ref files under `dir`, carrying the ref-name prefix built
/// so far. Lock files are skipped; name validation happens at parse time.
fn collect_loose_ref_files(
    dir: &std::path::Path,
    name_prefix: gix::bstr::BString,
    out: &mut Vec<(gix::bstr::BString, std::path::PathBuf)>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(file_name) = gix::path::os_string_into_bstring(entry.file_name()) else {
            continue;
        };
        if file_name.ends_with(b".lock") {
            continue;
        }
        let mut name = name_prefix.clone();
        name.extend_from_slice(&file_name);
        let path = entry.path();
        // The dirent's type is free; a symlink pays one stat to follow it, like the
        // live iteration does.
        let mut file_type = entry.file_type()?;
        if file_type.is_symlink() {
            file_type = std::fs::metadata(&path)?.file_type();
        }
        if file_type.is_dir() {
            name.push(b'/');
            collect_loose_ref_files(&path, name, out)?;
        } else {
            out.push((name, path));
        }
    }
    Ok(())
}

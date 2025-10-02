use crate::init::walk::RefsById;
use crate::init::{Entrypoint, Overlay};
use but_core::{RefMetadata, ref_metadata};
use gix::prelude::ReferenceExt;
use gix::refs::Target;
use std::borrow::Cow;
use std::collections::BTreeSet;

impl Overlay {
    /// Serve the given `refs` from memory, as if they would exist.
    /// This is true only, however, if a real reference doesn't exist.
    pub fn with_references_if_new(
        mut self,
        refs: impl IntoIterator<Item = gix::refs::Reference>,
    ) -> Self {
        self.nonoverriding_references = refs.into_iter().collect();
        self
    }

    /// Serve the given `refs` from memory, which is like creating the reference or as if its value was set,
    /// completely overriding the value in the repository.
    pub fn with_references(mut self, refs: impl IntoIterator<Item = gix::refs::Reference>) -> Self {
        self.overriding_references.extend(refs);
        self
    }

    /// Override the starting position of the traversal by setting it to `id`,
    /// and optionally, by providing the `ref_name` that points to `id`.
    pub fn with_entrypoint(
        mut self,
        id: gix::ObjectId,
        ref_name: Option<gix::refs::FullName>,
    ) -> Self {
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

    /// Serve the given `branches` metadata from memory, as if they would exist,
    /// possibly overriding metadata of a ref that already exists.
    pub fn with_branch_metadata_override(
        mut self,
        refs: impl IntoIterator<Item = (gix::refs::FullName, ref_metadata::Branch)>,
    ) -> Self {
        self.meta_branches = refs.into_iter().collect();
        self
    }

    /// Serve the given workspace `metadata` from memory, as if they would exist,
    /// possibly overriding metadata of a workspace at that place
    pub fn with_workspace_metadata_override(
        mut self,
        metadata: Option<(gix::refs::FullName, ref_metadata::Workspace)>,
    ) -> Self {
        self.workspace = metadata;
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
            mut nonoverriding_references,
            mut overriding_references,
            meta_branches,
            workspace,
            entrypoint,
        } = self;
        // Make sure that duplicates from later determine the value.
        nonoverriding_references.reverse();
        overriding_references.reverse();
        (
            OverlayRepo {
                nonoverriding_references: nonoverriding_references.into_iter().collect(),
                overriding_references: overriding_references.into_iter().collect(),
                inner: repo,
            },
            OverlayMetadata {
                inner: meta,
                meta_branches,
                workspace,
            },
            entrypoint,
        )
    }
}

pub(crate) struct OverlayRepo<'repo> {
    inner: &'repo gix::Repository,
    nonoverriding_references: BTreeSet<gix::refs::Reference>,
    overriding_references: BTreeSet<gix::refs::Reference>,
}

/// Note that functions with `'repo` in their return value technically leak the bare repo, and it's
/// up to us to ensure it's not actually used directly, or only such that the in-memory feature isn't bypassed.
impl<'repo> OverlayRepo<'repo> {
    pub fn commit_graph_if_enabled(&self) -> anyhow::Result<Option<gix::commitgraph::Graph>> {
        Ok(self.inner.commit_graph_if_enabled()?)
    }

    pub fn try_find_reference(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<Option<gix::Reference<'repo>>> {
        if let Some(r) = self
            .overriding_references
            .iter()
            .find(|r| r.name.as_ref() == ref_name)
        {
            Ok(Some(r.clone().attach(self.inner)))
        } else if let Some(rn) = self.inner.try_find_reference(ref_name)? {
            Ok(Some(rn))
        } else if let Some(r) = self
            .nonoverriding_references
            .iter()
            .find(|r| r.name.as_ref() == ref_name)
        {
            Ok(Some(r.clone().attach(self.inner)))
        } else {
            Ok(None)
        }
    }

    pub fn find_reference(
        &self,
        ref_name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<gix::Reference<'repo>> {
        if let Some(r) = self
            .overriding_references
            .iter()
            .find(|r| r.name.as_ref() == ref_name)
        {
            return Ok(r.clone().attach(self.inner));
        }
        Ok(self
            .inner
            .find_reference(ref_name)
            .or_else(|err| match err {
                gix::reference::find::existing::Error::Find(_) => Err(err),
                gix::reference::find::existing::Error::NotFound { .. } => {
                    if let Some(r) = self
                        .nonoverriding_references
                        .iter()
                        .find(|r| r.name.as_ref() == ref_name)
                    {
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

    pub fn for_attach_only(&self) -> &'repo gix::Repository {
        self.inner
    }

    pub fn for_find_only(&self) -> &'repo gix::Repository {
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
    pub fn collect_ref_mapping_by_prefix<'a>(
        &self,
        prefixes: impl Iterator<Item = &'a str>,
        workspace_ref_names: &[&gix::refs::FullNameRef],
    ) -> anyhow::Result<RefsById> {
        let mut seen = (!self.nonoverriding_references.is_empty()).then(BTreeSet::new);
        let mut ref_filter =
            |r: gix::Reference<'_>| -> Option<(gix::ObjectId, gix::refs::FullName)> {
                if workspace_ref_names.contains(&r.name()) {
                    return None;
                }
                let id = r.try_id()?;
                let (id, name) =
                    if matches!(r.name().category(), Some(gix::reference::Category::Tag)) {
                        // TODO: also make use of the tag name (the tag object has its own name)
                        (id.object().ok()?.peel_tags_to_end().ok()?.id, r.inner.name)
                    } else {
                        (id.detach(), r.inner.name)
                    };
                // This is only for overrides.
                if let Some(seen) = seen.as_mut() {
                    seen.insert(name.clone()).then_some((id, name))
                } else {
                    Some((id, name))
                }
            };
        let mut all_refs_by_id = gix::hashtable::HashMap::<_, Vec<_>>::default();
        for prefix in prefixes {
            // apply overrides - they are seen first and take the spot of everything.
            for (commit_id, git_reference) in self
                .overriding_references
                .iter()
                .filter(|rn| rn.name.as_bstr().starts_with(prefix.as_bytes()))
                .filter_map(|rn| ref_filter(rn.clone().attach(self.inner)))
            {
                all_refs_by_id
                    .entry(commit_id)
                    .or_default()
                    .push(git_reference);
            }
            for (commit_id, git_reference) in self
                .inner
                .references()?
                .prefixed(prefix)?
                .filter_map(Result::ok)
                .filter_map(&mut ref_filter)
            {
                all_refs_by_id
                    .entry(commit_id)
                    .or_default()
                    .push(git_reference);
            }
            // apply overrides (new only)
            for (commit_id, git_reference) in self
                .nonoverriding_references
                .iter()
                .filter(|rn| rn.name.as_bstr().starts_with(prefix.as_bytes()))
                .filter_map(|rn| ref_filter(rn.clone().attach(self.inner)))
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
}

pub(crate) struct OverlayMetadata<'meta, T> {
    inner: &'meta T,
    meta_branches: Vec<(gix::refs::FullName, ref_metadata::Branch)>,
    workspace: Option<(gix::refs::FullName, ref_metadata::Workspace)>,
}

impl<T> OverlayMetadata<'_, T>
where
    T: RefMetadata,
{
    pub fn iter_workspaces(
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
}

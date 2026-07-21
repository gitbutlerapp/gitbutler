use std::borrow::Cow;

use but_core::{DiffSpec, TreeStatus};

pub const CONTEXT_LINES: u32 = 0;

pub use but_testsupport::{
    read_only_in_memory_scenario, read_only_in_memory_scenario_named, visualize_index,
    writable_scenario, writable_scenario_slow, writable_scenario_with_args,
};

/// Test-only convenience: materialize a [`but_graph::Rebased`] with default
/// options, mirroring the old editor's `materialize()`/`materialize_without_checkout()`.
pub trait TestMaterializeExt: Sized {
    fn materialize(
        self,
        meta: &impl but_core::RefMetadata,
    ) -> anyhow::Result<but_graph::edit::MaterializeOutcome>;
    fn materialize_without_checkout(
        self,
        meta: &impl but_core::RefMetadata,
    ) -> anyhow::Result<but_graph::edit::MaterializeOutcome>;
}

impl TestMaterializeExt for but_graph::Rebased {
    fn materialize(
        self,
        meta: &impl but_core::RefMetadata,
    ) -> anyhow::Result<but_graph::edit::MaterializeOutcome> {
        self.materialize_changes(meta, but_graph::edit::MaterializeOptions::default())
    }
    fn materialize_without_checkout(
        self,
        meta: &impl but_core::RefMetadata,
    ) -> anyhow::Result<but_graph::edit::MaterializeOutcome> {
        self.materialize_changes(
            meta,
            but_graph::edit::MaterializeOptions { checkout: false },
        )
    }
}

/// Test-only convenience mirroring the old `LookupStep` trait over the various
/// stages of the edit lifecycle, keyed by stable [`but_graph::NodeIndex`].
pub trait TestLookupExt {
    fn lookup_pick(&self, index: but_graph::NodeIndex) -> anyhow::Result<gix::ObjectId>;
}

impl TestLookupExt for but_graph::Rebased {
    fn lookup_pick(&self, index: but_graph::NodeIndex) -> anyhow::Result<gix::ObjectId> {
        self.pick_at(index)
            .map(|pick| pick.id)
            .ok_or_else(|| anyhow::anyhow!("Expected selector {index} to point to a pick"))
    }
}

impl TestLookupExt for but_graph::MutableNodeGraph {
    fn lookup_pick(&self, index: but_graph::NodeIndex) -> anyhow::Result<gix::ObjectId> {
        self.pick_at(index)
            .map(|pick| pick.id)
            .ok_or_else(|| anyhow::anyhow!("Expected selector {index} to point to a pick"))
    }
}

impl TestLookupExt for but_graph::edit::MaterializeOutcome {
    fn lookup_pick(&self, index: but_graph::NodeIndex) -> anyhow::Result<gix::ObjectId> {
        match self.graph.nodes()[index].kind() {
            but_graph::NodeKind::Commit { id } => Ok(*id),
            // Unlike `pick_at`, this sealed-graph lookup reads ANY boundary
            // (shallow included) as a pick of its id — preserved verbatim from
            // the old nodes-based `lookup_step` mapping.
            but_graph::NodeKind::Boundary { id, .. } => Ok(*id),
            kind @ (but_graph::NodeKind::Reference(_) | but_graph::NodeKind::None) => {
                anyhow::bail!("Expected selector {index} to point to a pick, got {kind:?}")
            }
        }
    }
}

pub fn refresh_workspace_from_head(
    workspace: &mut but_graph::Workspace,
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<()> {
    *workspace = but_graph::Graph::from_repo(
        repo,
        meta,
        project_meta,
        but_graph::init::Overlay::default(),
    )?
    .into_workspace()?;
    Ok(())
}

pub fn workspace_tip_id(workspace: &but_graph::Workspace) -> Option<gix::ObjectId> {
    match workspace.graph.nodes().get(workspace.id?)?.kind() {
        but_graph::NodeKind::Commit { id } => Some(*id),
        but_graph::NodeKind::Reference(reference) => reference.ref_info.commit_id,
        but_graph::NodeKind::Boundary { .. } | but_graph::NodeKind::None => None,
    }
}

/// Always use all the hunks.
pub fn to_change_specs_whole_file(changes: but_core::WorktreeChanges) -> Vec<DiffSpec> {
    let out: Vec<_> = changes
        .changes
        .into_iter()
        .map(|change| DiffSpec {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path,
            hunk_headers: Vec::new(),
        })
        .collect();
    assert!(
        !out.is_empty(),
        "fixture should contain actual changes to turn into requests"
    );
    out
}

/// Always use all the hunks.
pub fn to_change_specs_all_hunks(
    repo: &gix::Repository,
    changes: but_core::WorktreeChanges,
) -> anyhow::Result<Vec<DiffSpec>> {
    to_change_specs_all_hunks_with_context_lines(repo, changes, CONTEXT_LINES)
}

/// Always use all the hunks.
pub fn to_change_specs_all_hunks_with_context_lines(
    repo: &gix::Repository,
    changes: but_core::WorktreeChanges,
    context_lines: u32,
) -> anyhow::Result<Vec<DiffSpec>> {
    let mut out = Vec::with_capacity(changes.changes.len());
    for change in changes.changes {
        let spec = match change.status {
            // Untracked files must always be taken from disk (they don't have a counterpart in a tree yet)
            TreeStatus::Addition { is_untracked, .. } if is_untracked => DiffSpec {
                path: change.path,
                ..Default::default()
            },
            _ => {
                match change.unified_patch(repo, context_lines)? {
                    Some(but_core::UnifiedPatch::Patch { hunks, .. }) => DiffSpec {
                        previous_path: change.previous_path().map(ToOwned::to_owned),
                        path: change.path,
                        hunk_headers: hunks.into_iter().map(Into::into).collect(),
                    },
                    Some(_) => unreachable!("tests won't be binary or too large"),
                    None => {
                        // Assume it's a submodule or something without content, don't do hunks then.
                        DiffSpec {
                            path: change.path,
                            ..Default::default()
                        }
                    }
                }
            }
        };
        out.push(spec);
    }
    Ok(out)
}

pub fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("statically known valid ref-name")
}

pub fn rc(name: &str) -> Cow<'static, gix::refs::FullNameRef> {
    Cow::Owned(name.try_into().expect("statically known valid ref-name"))
}

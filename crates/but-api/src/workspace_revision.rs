//! Checksum for the state that determines a legacy workspace projection.

use std::{collections::HashMap, fs, path::Path};

use bstr::ByteSlice as _;
use but_core::RepositoryExt as _;
use sha2::{Digest, Sha256};

const VERSION: &[u8] = b"workspace-v1";

/// Compute an opaque checksum of the inputs used to build `head_info`.
///
/// This deliberately excludes Gerrit. The virtual-branch TOML is hashed byte-for-byte, which is
/// safe but can cause an unnecessary refresh when only its formatting changes.
pub fn compute(ctx: &but_ctx::Context) -> anyhow::Result<String> {
    let worktrees_enabled = ctx.settings.feature_flags.worktree_manipulation;
    let mut worktrees = Vec::new();
    if worktrees_enabled {
        for worktree in ctx.worktrees_with_state()? {
            if worktree.archived {
                continue;
            }
            let Some(head) = ctx.worktree_head(worktree.name.as_bstr())? else {
                continue;
            };
            worktrees.push(WorktreeInput {
                name: worktree.name,
                ref_name: head.ref_name,
                id: head.id,
            });
        }
    }

    let metadata = ctx.meta()?;
    let project = ctx.project_meta()?;
    let db = ctx.db.get_cache()?;
    let prs = crate::workspace_state::forge_prs_by_head(&db)?;
    let repo = ctx.repo.get()?;
    compute_from_inputs(
        &repo,
        &metadata,
        project,
        &ctx.project_data_dir(),
        worktrees_enabled,
        worktrees,
        &prs,
    )
}

pub(crate) fn compute_for_workspace<M: but_core::RefMetadata>(
    repo: &gix::Repository,
    metadata: &M,
    workspace: &but_graph::Workspace,
    prs: &HashMap<String, usize>,
) -> anyhow::Result<String> {
    let worktrees = workspace
        .graph
        .worktree_tips
        .iter()
        .map(|tip| WorktreeInput {
            name: tip.name.clone(),
            ref_name: tip.ref_name.clone(),
            id: tip.id,
        })
        .collect();
    compute_from_inputs(
        repo,
        metadata,
        workspace.graph.project_meta.clone(),
        &repo.gitbutler_storage_path()?,
        workspace.graph.options.worktrees,
        worktrees,
        prs,
    )
}

struct WorktreeInput {
    name: bstr::BString,
    ref_name: Option<gix::refs::FullName>,
    id: gix::ObjectId,
}

fn compute_from_inputs<M: but_core::RefMetadata>(
    repo: &gix::Repository,
    metadata: &M,
    project: but_core::ref_metadata::ProjectMeta,
    project_data_dir: &Path,
    worktrees_enabled: bool,
    mut worktrees: Vec<WorktreeInput>,
    prs: &HashMap<String, usize>,
) -> anyhow::Result<String> {
    let mut digest = CanonicalDigest::new();
    let mut local_refs = Vec::new();

    {
        let head = repo.head()?;
        digest.optional(
            b"head-ref",
            head.referent_name().map(|name| name.as_bstr().as_ref()),
        );
        match head.id() {
            Some(id) => digest.optional(b"head-id", Some(id.as_bytes())),
            None => digest.optional(b"head-id", None),
        }

        let mut refs = Vec::new();
        for prefix in ["refs/heads/", "refs/remotes/"] {
            for reference in repo.references()?.prefixed(prefix)?.filter_map(Result::ok) {
                let name = reference.name().as_bstr().to_vec();
                let target = match reference.target() {
                    gix::refs::TargetRef::Object(id) => (b'o', id.as_bytes().to_vec()),
                    gix::refs::TargetRef::Symbolic(name) => (b's', name.as_bstr().to_vec()),
                };
                if prefix == "refs/heads/" {
                    local_refs.push(reference.name().to_owned());
                }
                refs.push((name, target));
            }
        }
        refs.sort_by(|a, b| a.0.cmp(&b.0));
        digest.usize(b"ref-count", refs.len());
        for (name, (kind, target)) in refs {
            digest.field(b"ref-name", &name);
            digest.field(b"ref-kind", &[kind]);
            digest.field(b"ref-target", &target);
        }

        local_refs.sort_by(|a, b| a.as_bstr().cmp(b.as_bstr()));
        for local_ref in &local_refs {
            let tracking = repo
                .branch_remote_tracking_ref_name(local_ref.as_ref(), gix::remote::Direction::Fetch)
                .transpose()?;
            digest.field(b"tracking-local", local_ref.as_bstr());
            digest.optional(
                b"tracking-remote",
                tracking.as_ref().map(|name| name.as_bstr().as_ref()),
            );
        }

        let mut remote_names = repo
            .remote_names()
            .iter()
            .map(|name| name.as_bytes().to_vec())
            .collect::<Vec<_>>();
        remote_names.sort();
        for name in remote_names {
            digest.field(b"remote-name", &name);
        }

        if let Some(shallow) = repo.shallow_commits()? {
            let mut ids = shallow.iter().copied().collect::<Vec<_>>();
            ids.sort();
            for id in ids {
                digest.field(b"shallow", id.as_bytes());
            }
        }
    }

    digest.optional(
        b"project-target-ref",
        project
            .target_ref
            .as_ref()
            .map(|name| name.as_bstr().as_ref()),
    );
    digest.optional(
        b"project-target-id",
        project.target_commit_id.as_ref().map(|id| id.as_bytes()),
    );
    digest.optional(
        b"project-push-remote",
        project.push_remote.as_deref().map(str::as_bytes),
    );

    let metadata_path = project_data_dir.join("virtual_branches.toml");
    match fs::read(metadata_path) {
        Ok(bytes) => digest.optional(b"virtual-branches", Some(&bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            digest.optional(b"virtual-branches", None)
        }
        Err(err) => return Err(err.into()),
    }

    for local_ref in &local_refs {
        let order = metadata.branch_stack_order(local_ref.as_ref())?;
        digest.field(b"branch-order-for", local_ref.as_bstr());
        digest.field(b"branch-order-present", &[u8::from(order.is_some())]);
        digest.usize(b"branch-order-count", order.as_ref().map_or(0, Vec::len));
        if let Some(order) = order {
            for name in order {
                digest.field(b"branch-order-ref", name.as_bstr());
            }
        }
    }

    digest.field(b"worktrees-enabled", &[u8::from(worktrees_enabled)]);
    if worktrees_enabled {
        worktrees.sort_by(|a, b| a.name.cmp(&b.name));
        for worktree in worktrees {
            digest.field(b"worktree-name", &worktree.name);
            digest.optional(
                b"worktree-head-ref",
                worktree
                    .ref_name
                    .as_ref()
                    .map(|name| name.as_bstr().as_ref()),
            );
            digest.optional(b"worktree-head-id", Some(worktree.id.as_bytes()));
        }
    }

    let mut prs = prs.iter().collect::<Vec<_>>();
    prs.sort();
    for (head, number) in prs {
        digest.field(b"forge-head", head.as_bytes());
        digest.usize(b"forge-pr", *number);
    }

    Ok(format!("workspace-v1:{:x}", digest.finish()))
}

struct CanonicalDigest(Sha256);

impl CanonicalDigest {
    fn new() -> Self {
        let mut digest = Self(Sha256::new());
        digest.field(b"version", VERSION);
        digest
    }

    fn field(&mut self, name: &[u8], value: &[u8]) {
        self.0.update(name.len().to_be_bytes());
        self.0.update(name);
        self.0.update(value.len().to_be_bytes());
        self.0.update(value);
    }

    fn optional(&mut self, name: &[u8], value: Option<&[u8]>) {
        self.field(name, &[u8::from(value.is_some())]);
        if let Some(value) = value {
            self.field(name, value);
        }
    }

    fn usize(&mut self, name: &[u8], value: usize) {
        self.field(name, &value.to_be_bytes());
    }

    fn finish(self) -> impl std::fmt::LowerHex {
        self.0.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::CanonicalDigest;

    #[test]
    fn canonical_fields_do_not_alias_at_boundaries() {
        let mut left = CanonicalDigest::new();
        left.field(b"a", b"bc");
        let mut right = CanonicalDigest::new();
        right.field(b"ab", b"c");

        assert_ne!(
            format!("{:x}", left.finish()),
            format!("{:x}", right.finish())
        );
    }
}

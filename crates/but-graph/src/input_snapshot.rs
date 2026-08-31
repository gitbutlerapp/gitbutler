use bstr::ByteSlice as _;
use but_core::{RefMetadata, ref_metadata};

use crate::init::{Options, WorktreeTip};

/// Opaque, canonical snapshot of the semantic inputs used to build a workspace graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceInputSnapshot(Vec<u8>);

impl WorkspaceInputSnapshot {
    /// Canonical bytes suitable for equality checks and versioned hashing by API consumers.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Capture the repository, metadata, traversal, and worktree inputs used by workspace graph
/// construction.
pub fn capture_workspace_inputs(
    repo: &gix::Repository,
    metadata: &impl RefMetadata,
    project: &ref_metadata::ProjectMeta,
    db: &mut but_db::DbHandle,
    options: &Options,
) -> anyhow::Result<WorkspaceInputSnapshot> {
    let worktrees = crate::init::discover_worktree_tips(repo, db, options.worktrees)?;
    capture(repo, metadata, project, options, worktrees)
}

/// Build a workspace from `HEAD` and return its source inputs only when they stayed stable for the
/// whole graph construction.
pub fn workspace_from_head_with_inputs(
    repo: &gix::Repository,
    metadata: &impl RefMetadata,
    project: ref_metadata::ProjectMeta,
    db: &mut but_db::DbHandle,
    options: Options,
) -> anyhow::Result<(crate::Workspace, Option<WorkspaceInputSnapshot>)> {
    let before = capture_workspace_inputs(repo, metadata, &project, db, &options)
        .inspect_err(|err| tracing::warn!(?err, "failed to capture workspace inputs"))
        .ok();
    let workspace = crate::Graph::from_head(repo, metadata, project.clone(), db, options.clone())?
        .into_workspace()?;
    let after = capture_workspace_inputs(repo, metadata, &project, db, &options)
        .inspect_err(|err| tracing::warn!(?err, "failed to capture workspace inputs"))
        .ok();
    let stable = before
        .zip(after)
        .and_then(|(before, after)| (before == after).then_some(after));
    Ok((workspace, stable))
}

fn capture(
    repo: &gix::Repository,
    metadata: &impl RefMetadata,
    project: &ref_metadata::ProjectMeta,
    options: &Options,
    mut worktrees: Vec<WorktreeTip>,
) -> anyhow::Result<WorkspaceInputSnapshot> {
    let mut out = Encoder::default();
    let head = repo.head()?;
    out.optional(
        b"head-ref",
        head.referent_name().map(|name| name.as_bstr().as_ref()),
    );
    match head.id() {
        Some(id) => out.optional(b"head-id", Some(id.as_bytes())),
        None => out.optional(b"head-id", None),
    }

    let mut local_refs = Vec::new();
    let mut refs = Vec::new();
    for prefix in ["refs/heads/", "refs/remotes/"]
        .into_iter()
        .chain(options.collect_tags.then_some("refs/tags/"))
    {
        for reference in repo.references()?.prefixed(prefix)? {
            let reference = reference
                .map_err(|err| anyhow::anyhow!("failed to read workspace reference: {err}"))?;
            let name = reference.name().to_owned();
            let target = match reference.target() {
                gix::refs::TargetRef::Object(id) => (b'o', id.as_bytes().to_vec()),
                gix::refs::TargetRef::Symbolic(name) => (b's', name.as_bstr().to_vec()),
            };
            if prefix == "refs/heads/" {
                local_refs.push(name.clone());
            }
            refs.push((name, target));
        }
    }
    refs.sort_by(|a, b| a.0.as_bstr().cmp(b.0.as_bstr()));
    for (name, (kind, target)) in refs {
        out.field(b"ref-name", name.as_bstr());
        out.field(b"ref-kind", &[kind]);
        out.field(b"ref-target", &target);
    }

    local_refs.sort_by(|a, b| a.as_bstr().cmp(b.as_bstr()));
    for local_ref in &local_refs {
        let tracking = repo
            .branch_remote_tracking_ref_name(local_ref.as_ref(), gix::remote::Direction::Fetch)
            .transpose()?;
        out.field(b"tracking-local", local_ref.as_bstr());
        out.optional(
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
        out.field(b"remote-name", &name);
    }

    if let Some(shallow) = repo.shallow_commits()? {
        let mut ids = shallow.iter().copied().collect::<Vec<_>>();
        ids.sort();
        for id in ids {
            out.field(b"shallow", id.as_bytes());
        }
    }

    encode_project(&mut out, project);
    encode_metadata(&mut out, metadata)?;
    for local_ref in &local_refs {
        let order = metadata.branch_stack_order(local_ref.as_ref())?;
        out.field(b"branch-order-for", local_ref.as_bstr());
        out.optional_bytes(
            b"branch-order",
            order.as_ref().map(|order| {
                let mut encoded = Encoder::default();
                for name in order {
                    encoded.field(b"ref", name.as_bstr());
                }
                encoded.finish()
            }),
        );
    }

    encode_options(&mut out, options);
    worktrees.sort_by(|a, b| a.name.cmp(&b.name));
    for worktree in worktrees {
        out.field(b"worktree-name", &worktree.name);
        out.optional(
            b"worktree-ref",
            worktree
                .ref_name
                .as_ref()
                .map(|name| name.as_bstr().as_ref()),
        );
        out.field(b"worktree-id", worktree.id.as_bytes());
    }

    Ok(WorkspaceInputSnapshot(out.finish()))
}

fn encode_project(out: &mut Encoder, project: &ref_metadata::ProjectMeta) {
    out.optional(
        b"project-target-ref",
        project
            .target_ref
            .as_ref()
            .map(|name| name.as_bstr().as_ref()),
    );
    out.optional(
        b"project-target-id",
        project.target_commit_id.as_ref().map(|id| id.as_bytes()),
    );
    out.optional(
        b"project-push-remote",
        project.push_remote.as_deref().map(str::as_bytes),
    );
}

fn encode_metadata(out: &mut Encoder, metadata: &impl RefMetadata) -> anyhow::Result<()> {
    let mut entries = Vec::new();
    for entry in metadata.iter() {
        let (name, value) = entry?;
        let (kind, bytes) = match value.downcast::<ref_metadata::Workspace>() {
            Ok(workspace) => (b'w', encode_workspace(&workspace)),
            Err(value) => match value.downcast::<ref_metadata::Branch>() {
                Ok(branch) => (b'b', encode_branch(&branch)),
                Err(_) => continue,
            },
        };
        entries.push((name, kind, bytes));
    }
    entries.sort_by(|a, b| a.0.as_bstr().cmp(b.0.as_bstr()).then_with(|| a.1.cmp(&b.1)));
    for (name, kind, bytes) in entries {
        out.field(b"metadata-ref", name.as_bstr());
        out.field(b"metadata-kind", &[kind]);
        out.field(b"metadata-value", &bytes);
    }
    Ok(())
}

fn encode_workspace(workspace: &ref_metadata::Workspace) -> Vec<u8> {
    let mut out = Encoder::default();
    encode_ref_info(&mut out, &workspace.ref_info);
    for stack in &workspace.stacks {
        out.field(b"stack-id", stack.id.0.as_bytes());
        match stack.workspacecommit_relation {
            ref_metadata::WorkspaceCommitRelation::Merged => out.field(b"relation", b"merged"),
            ref_metadata::WorkspaceCommitRelation::MergeFrom { commit_id } => {
                out.field(b"relation", b"merge-from");
                out.optional(
                    b"relation-commit",
                    commit_id.as_ref().map(|id| id.as_bytes()),
                );
            }
            ref_metadata::WorkspaceCommitRelation::Outside => out.field(b"relation", b"outside"),
        }
        for branch in &stack.branches {
            out.field(b"stack-branch", branch.ref_name.as_bstr());
            out.bool(b"stack-branch-archived", branch.archived);
        }
    }
    out.finish()
}

fn encode_branch(branch: &ref_metadata::Branch) -> Vec<u8> {
    let mut out = Encoder::default();
    encode_ref_info(&mut out, &branch.ref_info);
    out.optional(
        b"review-pr",
        branch
            .review
            .pull_request
            .map(|number| (number as u64).to_be_bytes())
            .as_ref()
            .map(|bytes| bytes.as_slice()),
    );
    out.optional(
        b"review-id",
        branch.review.review_id.as_deref().map(str::as_bytes),
    );
    out.finish()
}

fn encode_ref_info(out: &mut Encoder, info: &ref_metadata::RefInfo) {
    encode_time(out, b"created-at", info.created_at);
    encode_time(out, b"updated-at", info.updated_at);
}

fn encode_time(out: &mut Encoder, name: &[u8], time: Option<gix::date::Time>) {
    let bytes = time.map(|time| {
        let mut bytes = Vec::with_capacity(12);
        bytes.extend_from_slice(&time.seconds.to_be_bytes());
        bytes.extend_from_slice(&time.offset.to_be_bytes());
        bytes
    });
    out.optional_bytes(name, bytes);
}

fn encode_options(out: &mut Encoder, options: &Options) {
    out.bool(b"collect-tags", options.collect_tags);
    out.optional_u64(
        b"commits-limit",
        options.commits_limit_hint.map(|value| value as u64),
    );
    let mut recharge = options.commits_limit_recharge_location.clone();
    recharge.sort();
    for id in recharge {
        out.field(b"commits-limit-recharge", id.as_bytes());
    }
    out.optional_u64(b"hard-limit", options.hard_limit.map(|value| value as u64));
    out.optional(
        b"extra-target",
        options
            .extra_target_commit_id
            .as_ref()
            .map(|id| id.as_bytes()),
    );
    out.bool(
        b"skip-postprocessing",
        options.dangerously_skip_postprocessing_for_debugging,
    );
    out.bool(b"worktrees", options.worktrees);
}

#[derive(Default)]
struct Encoder(Vec<u8>);

impl Encoder {
    fn field(&mut self, name: &[u8], value: &[u8]) {
        self.0.extend_from_slice(&(name.len() as u64).to_be_bytes());
        self.0.extend_from_slice(name);
        self.0
            .extend_from_slice(&(value.len() as u64).to_be_bytes());
        self.0.extend_from_slice(value);
    }

    fn optional(&mut self, name: &[u8], value: Option<&[u8]>) {
        self.bool(name, value.is_some());
        if let Some(value) = value {
            self.field(name, value);
        }
    }

    fn optional_bytes(&mut self, name: &[u8], value: Option<Vec<u8>>) {
        self.optional(name, value.as_deref());
    }

    fn optional_u64(&mut self, name: &[u8], value: Option<u64>) {
        self.optional_bytes(name, value.map(u64::to_be_bytes).map(Vec::from));
    }

    fn bool(&mut self, name: &[u8], value: bool) {
        self.field(name, &[u8::from(value)]);
    }

    fn finish(self) -> Vec<u8> {
        self.0
    }
}

use crate::virtual_branches_legacy_types::{CommitOrChangeId, Stack, StackBranch, VirtualBranches};
use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use but_core::{
    RefMetadata, is_workspace_ref_name,
    ref_metadata::{
        Branch, RefInfo, StackId,
        StackKind::{Applied, AppliedAndUnapplied},
        ValueInfo, Workspace, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use gitbutler_reference::RemoteRefname;
use gix::{
    date::SecondsSinceUnixEpoch,
    reference::Category,
    refs::{FullName, FullNameRef},
};
use itertools::Itertools;
use std::collections::{BTreeMap, BTreeSet};
use std::{
    any::Any,
    cell::RefCell,
    collections::HashSet,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    time::Instant,
};
use tracing::instrument;

#[derive(Debug, Clone)]
struct Snapshot {
    /// The time at which the `content` was changed, before it was written to disk.
    changed_at: Option<Instant>,
    content: VirtualBranches,
    path: PathBuf,
}

enum ReconcileWithWorkspace {
    Allow,
    Disallow,
}

impl Snapshot {
    fn from_path(path: PathBuf) -> anyhow::Result<Self> {
        let content = fs::read_toml_file_or_default(&path)?;
        Ok(Self {
            path,
            changed_at: None,
            content,
        })
    }

    fn write_if_changed(&mut self, reconcile: ReconcileWithWorkspace) -> anyhow::Result<()> {
        if self.changed_at.is_some() {
            if self.content == Default::default() {
                std::fs::remove_file(&self.path)?;
            } else {
                if let Some(dir) = self.path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                fs::write(
                    &self.path,
                    toml::to_string(&self.to_consistent_data(reconcile))?,
                )?;
            }
            self.changed_at.take();
        }
        Ok(())
    }

    fn try_write_if_changed(&mut self, reconcile: ReconcileWithWorkspace) {
        let res = self.write_if_changed(reconcile);
        if let Err(err) = res {
            tracing::error!(
                "Could not write back changes to virtual branches toml file to '{}': {err}",
                self.path.display()
            );
        }
    }

    /// Assure we don't think the content changed, so writing it if changed will do nothing.
    fn claim_unchanged(&mut self) {
        self.changed_at.take();
    }

    /// The vb.toml snapshot held internally is marked as changed so it will be written back to disk on drop.
    pub fn set_changed_to_necessitate_write(&mut self) {
        self.changed_at = Some(Instant::now());
    }
}

/// Evil hacks to reconcile the workspace metadata with the workspace as new code sees it,
/// so old code keeps up (it relies only on metadata).
impl Snapshot {
    /// The fixes here aren't relevant for the ref-metadata, but important for storage.
    /// Instead of trying to maintain this, let's just fix it before writing.
    /// `reconcile` controls if the data should also be reconciled.
    fn to_consistent_data(&self, reconcile: ReconcileWithWorkspace) -> VirtualBranches {
        // EVIL HACK: assure we fill-in the CommitIDs of heads or else everything breaks.
        //            this probably won't be needed once no old code is running, and by then
        //            we should move away from this anyway and have a DB backed implementation.
        let repo = gix::discover(self.path.parent().expect("at least a file"))
            .ok()
            .map(|repo| {
                (
                    repo,
                    CommitOrChangeId::CommitId(gix::hash::Kind::Sha1.null().to_string()),
                )
            });
        let mut clone = self.clone();
        if let Some((repo, _)) = repo
            .as_ref()
            .filter(|_| matches!(reconcile, ReconcileWithWorkspace::Allow))
        {
            clone.reconcile_in_workspace_state_of_vb_toml(repo).ok();
        }
        for stack in clone.content.branches.values_mut() {
            if stack.name.is_empty() {
                stack.name = stack
                    .heads
                    // experiments show this is the bottom-most branch
                    .last()
                    .map(|h| h.name.as_str())
                    .unwrap_or_default()
                    .to_string();
            }
            if let Some((repo, null_id)) = repo.as_ref() {
                for segment in &mut stack.heads {
                    if &segment.head == null_id {
                        let Ok(mut r) = repo.find_reference(&segment.name) else {
                            continue;
                        };
                        if let Ok(id) = r.peel_to_id() {
                            segment.head = CommitOrChangeId::CommitId(id.to_string());
                        }
                    }
                }
            }
        }
        clone.content
    }

    #[instrument(level = tracing::Level::DEBUG, skip(repo))]
    fn reconcile_in_workspace_state_of_vb_toml(
        &mut self,
        repo: &gix::Repository,
    ) -> anyhow::Result<()> {
        fn make_heads_match(ws_stack: &but_graph::projection::Stack, vb_stack: &mut Stack) -> bool {
            // Always leave extra segments.

            // Add missing segments
            let segments_to_add: Vec<_> = ws_stack
                .segments
                .iter()
                .filter_map(|s| {
                    s.ref_name().and_then(|rn| {
                        let is_in_vb_stack_branches =
                            vb_stack.heads.iter().any(|sb| sb.name == rn.shorten());
                        (!is_in_vb_stack_branches).then_some((s, rn))
                    })
                })
                .collect();

            for (segment, segment_name) in segments_to_add {
                vb_stack.heads.push(StackBranch {
                    head: CommitOrChangeId::CommitId(
                        segment
                            .commits
                            .first()
                            .map_or(gix::hash::Kind::Sha1.null(), |c| c.id)
                            .to_string(),
                    ),
                    name: segment_name.shorten().to_string(),
                    description: None,
                    pr_number: None,
                    archived: false,
                    review_id: None,
                });
            }

            // finally, put them in order, for good measure.
            let previous_heads = vb_stack.heads.clone();
            let original_positions_by_name: BTreeMap<_, _> = vb_stack
                .heads
                .iter()
                // Use our order
                .rev()
                .enumerate()
                .map(|(idx, s)| (s.name.clone(), idx))
                .collect();
            vb_stack.heads.sort_by_key(|sb| {
                ws_stack
                    .segments
                    .iter()
                    .position(|s| s.ref_name().is_some_and(|rn| rn.shorten() == sb.name))
                    .or_else(|| original_positions_by_name.get(&sb.name).copied())
            });
            // The ws_stack order is top to bottom, the other is bottom to top.
            vb_stack.heads.reverse();
            vb_stack.heads != previous_heads
        }

        let mut reference = repo.find_reference("refs/heads/gitbutler/workspace")?;
        let commit_id = reference.peel_to_commit()?.id();
        let sideeffect_free_meta = std::mem::ManuallyDrop::new(VirtualBranchesTomlMetadata {
            snapshot: Snapshot {
                changed_at: None,
                ..self.clone()
            },
        });
        let graph = but_graph::Graph::from_commit_traversal(
            commit_id,
            reference.name().to_owned(),
            &*sideeffect_free_meta,
            but_graph::init::Options::limited(),
        )?;

        let ws = graph.to_workspace()?;
        let mut seen = BTreeSet::new();

        // Make sure we have a stack id, which is something that may not be the case in
        // single-branch mode or in tests that start off with just a Git repository.
        // Having stack IDs is useful and maybe one day we can pre-generate them just like we do here
        // as they only have to be locally unique.
        let workspace_unique_stack_id = |mut idx: usize| -> StackId {
            let mut stack_id = StackId::from_number_for_testing(idx as u128);
            while self.content.branches.contains_key(&stack_id) {
                idx += 1;
                stack_id = StackId::from_number_for_testing(idx as u128);
            }
            stack_id
        };
        let ws_stacks_to_represent_in_vb_toml: Vec<_> = ws
            .stacks
            .iter()
            .enumerate()
            .map(|(idx, s)| {
                let id = s.id.unwrap_or_else(|| workspace_unique_stack_id(idx + 1));
                (s, id, idx)
            })
            .collect();
        for (ws_stack, in_workspace_stack_id, ws_stack_idx) in ws_stacks_to_represent_in_vb_toml {
            seen.insert(in_workspace_stack_id);
            let mut inserted_new_stack = false;
            let vb_stack = self
                .content
                .branches
                .entry(in_workspace_stack_id)
                .or_insert_with(|| {
                    inserted_new_stack = true;
                    Stack::new_with_just_heads(
                        vec![],
                        gix::date::Time::now_utc().seconds as u128 * 1000,
                        ws_stack_idx,
                        true,
                    )
                });
            let made_heads_match = make_heads_match(ws_stack, vb_stack);
            if !vb_stack.in_workspace {
                tracing::warn!(
                    "Fixing stale metadata of stack {in_workspace_stack_id} to be considered inside the workspace",
                );
                vb_stack.in_workspace = true;
                self.set_changed_to_necessitate_write();
            }
            if made_heads_match {
                tracing::warn!(
                    "Adjusted segments in stack {in_workspace_stack_id} to match what's actually there"
                );
                self.set_changed_to_necessitate_write();
            }
            if inserted_new_stack {
                self.set_changed_to_necessitate_write();
            }
        }

        let stack_ids_to_remove_from_workspace: Vec<_> = sideeffect_free_meta
            .data()
            .branches
            .keys()
            .filter(|stack_id| !seen.contains(stack_id))
            .copied()
            .collect();
        for stack_id_not_in_workspace in stack_ids_to_remove_from_workspace {
            let vb_stack = self
                .content
                .branches
                .get_mut(&stack_id_not_in_workspace)
                .expect("BUG: we just traversed this stack-id");
            if vb_stack.in_workspace {
                tracing::warn!(
                    "Fixing stale metadata of stack {stack_id_not_in_workspace} to be considered outside the workspace",
                );
                vb_stack.in_workspace = false;
                self.set_changed_to_necessitate_write();
            }
        }
        Ok(())
    }
}

/// An implementation to read and write metadata from the `virtual_branches.toml` file, meant to be a short-lived item
/// that is possibly written multiple times. It will write itself on drop only, and log write failures.
///
/// The idea is that it's forgiving and easy to use, while helping to eventually migrate to a database.
#[derive(Debug)]
pub struct VirtualBranchesTomlMetadata {
    // What is currently in memory for query or editing.
    snapshot: Snapshot,
}

/// Lifecycle
impl VirtualBranchesTomlMetadata {
    /// Initialize a store backed by a file on disk.
    ///
    /// Also, set-up a thread for debounced writing.
    pub fn from_path(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let path = path.into();
        Ok(Self {
            snapshot: Snapshot::from_path(path)?,
        })
    }

    /// Return the path at which the toml file is located.
    ///
    /// We will write changes to it on drop.
    pub fn path(&self) -> &Path {
        &self.snapshot.path
    }
}

impl VirtualBranchesTomlMetadata {
    /// Validate and fix workspace stack `in_workspace` status of `virtual_branches.toml`
    /// so they match what's actually in the workspace.
    /// If there is a change, the data is written back once this instance is dropped.
    ///
    /// Errors are silently ignored to allow the application to continue loading even if
    /// the migration fails - the workspace will still be functional, just potentially
    /// with stale metadata that can confuse 'old' code.
    ///
    /// NOTE: This isn't needed for new code - it won't base any decisions on the metadata alone.
    ///
    /// `repo` is expected to be the repository this instance relates to.
    /// Consume this instance to prevent double-reconciliation which also happens on drop.
    pub fn write_reconciled(mut self, repo: &gix::Repository) -> anyhow::Result<()> {
        // First possibly change our data…
        self.snapshot
            .reconcile_in_workspace_state_of_vb_toml(repo)?;
        // Then write changes back.
        self.snapshot
            .write_if_changed(ReconcileWithWorkspace::Disallow)
    }
}

/// Mostly used in testing, and it's fine as it's intermediate, and we are very practical here.
impl VirtualBranchesTomlMetadata {
    /// Return a mutable snapshot of the underlying data. Useful for testing mainly.
    ///
    /// Consider calling [Self::set_changed_to_necessitate_write()] to have the changes written back.
    pub fn data_mut(&mut self) -> &mut VirtualBranches {
        &mut self.snapshot.content
    }

    /// Return a snapshot of the underlying data. Useful for working around (intended) limitations of the RefMetadata trait.
    pub fn data(&self) -> &VirtualBranches {
        &self.snapshot.content
    }

    /// The vb.toml snapshot held internally is marked as changed so it will be written back to disk on drop.
    pub fn set_changed_to_necessitate_write(&mut self) {
        self.snapshot.set_changed_to_necessitate_write();
    }
}

// Emergency-behaviour in case the application winds down, we don't want data-loss (at least a chance).
impl Drop for VirtualBranchesTomlMetadata {
    fn drop(&mut self) {
        self.snapshot
            .try_write_if_changed(ReconcileWithWorkspace::Allow);
    }
}

const INTEGRATION_BRANCH: &str = "refs/heads/gitbutler/workspace";

impl RefMetadata for VirtualBranchesTomlMetadata {
    type Handle<T> = VBTomlMetadataHandle<T>;

    fn iter(&self) -> impl Iterator<Item = anyhow::Result<(FullName, Box<dyn Any>)>> + '_ {
        let data = &self.snapshot.content;
        // Keep it simple - dump everything into a Vec, pre-allocated.
        let mut out = Vec::new();
        if data.branches.is_empty() {
            return out.into_iter();
        }

        // Brute force, but simple.
        for stack in data.branches.values() {
            for branch_ref_name in stack
                .heads
                .iter()
                .filter_map(|branch| full_branch_name(&branch.name))
            {
                out.push(self.branch(branch_ref_name.as_ref()).map(|branch| {
                    (
                        branch_ref_name.clone(),
                        Box::new((*branch).clone()) as Box<dyn Any>,
                    )
                }));
            }
        }

        // Workspace last, also so that journey test has a harder time as it can delete the branches one by one.
        out.push(Ok((
            gix::refs::FullName::try_from(INTEGRATION_BRANCH).expect("known to be valid"),
            Box::new(Self::workspace_from_data(data)),
        )));
        out.into_iter()
    }

    fn workspace(&self, ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Workspace>> {
        if is_workspace_ref_name(ref_name) {
            let value = Self::workspace_from_data(self.data());
            Ok(VBTomlMetadataHandle {
                is_default: value == default_workspace(),
                ref_name: ref_name.to_owned(),
                stack_id: None.into(),
                value,
            })
        } else {
            Ok(VBTomlMetadataHandle {
                is_default: true,
                ref_name: ref_name.to_owned(),
                stack_id: None.into(),
                value: Default::default(),
            })
        }
    }

    fn branch(&self, ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
        let Some((stack, branch)) = self
            .data()
            .branches
            .values()
            // There shouldn't be duplication, but let's be sure it's deterministic if it is.
            .sorted_by_key(|s| s.order)
            .find_map(|stack| {
                stack.heads.iter().find_map(|branch| {
                    full_branch_name(branch.name.as_str()).and_then(|full_name| {
                        (full_name.as_ref() == ref_name).then_some((stack, branch))
                    })
                })
            })
        else {
            return Ok(VBTomlMetadataHandle {
                is_default: true,
                ref_name: ref_name.to_owned(),
                stack_id: None.into(),
                value: Branch::default(),
            });
        };

        let ref_info = RefInfo {
            // keep None, as otherwise it means we created it, which allows us to delete the ref.
            // However, for it's too early for that logic.
            created_at: None,
            updated_at: Some(gix::date::Time {
                seconds: (stack.updated_timestamp_ms / 1000) as SecondsSinceUnixEpoch,
                ..gix::date::Time::now_utc()
            }),
        };
        Ok(VBTomlMetadataHandle {
            is_default: false,
            ref_name: ref_name.to_owned(),
            stack_id: Some(stack.id).into(),
            value: Branch {
                ref_info,
                description: branch.description.clone(),
                review: but_core::ref_metadata::Review {
                    pull_request: branch.pr_number,
                    review_id: branch.review_id.clone(),
                },
            },
        })
    }

    fn set_workspace(&mut self, value: &Self::Handle<Workspace>) -> anyhow::Result<()> {
        let ref_name = value.ref_name.as_ref();
        if !is_workspace_ref_name(ref_name) {
            bail!("This backend doesn't support saving arbitrary workspaces");
        }

        // Find exactly one stack-id per branch name, and assign all branches to it.
        // `stacks` is the target state, and we have to make an actual stack look like it.
        let mut seen_stack_ids = HashSet::new();
        for stack in &value.stacks {
            let mut branches_to_create = Vec::new();
            let mut stack_id = None::<StackId>;
            for stack_branch in &stack.branches {
                let branch = self.branch(stack_branch.ref_name.as_ref())?;
                if branch.is_default() {
                    branches_to_create.push(stack_branch);
                    continue;
                }
                if let Some(stack_id) = *branch.stack_id.borrow() {
                    seen_stack_ids.insert(stack_id);
                }
                if stack_id.is_none() {
                    stack_id = *branch.stack_id.borrow();
                } else if let Some(stack_id) = branch.stack_id.borrow().zip(stack_id).and_then(
                    |(branch_stack_id, stack_id)| (branch_stack_id != stack_id).then_some(stack_id),
                ) {
                    // This branch was in another stack previously, but is now assigned to this one
                    // via the workspace data.
                    // Make sure we move it in the underlying data structure to here.
                    let to_move =
                        self.remove_branch(branch.ref_name.as_ref())?
                            .with_context(|| {
                                format!(
                                    "BUG: couldn't remove branch {branch} from its original stack",
                                    branch = branch.ref_name
                                )
                            })?;
                    self.data_mut()
                        .branches
                        .get_mut(&stack_id)
                        .context(
                            "BUG: stack id we saw should exist for inserting moved stack branch",
                        )?
                        .heads
                        .push(to_move);
                }
            }

            let vb_stack = match stack_id {
                None => {
                    let branch_for_stack = match stack.branches.iter().find(|branch| {
                        !branches_to_create
                            .iter()
                            .any(|other_branch| other_branch.ref_name == branch.ref_name)
                    }) {
                        Some(branch) => branch,
                        None => branches_to_create.pop().context(
                            "BUG: incoming stack is probably empty, caller should have removed the whole stack",
                        )?,
                    };

                    let branch = self.branch(branch_for_stack.ref_name.as_ref())?;
                    self.set_branch(&branch)?;
                    let new_stack_id = branch.stack_id.borrow().expect("was just created");
                    *branch.stack_id.borrow_mut() = Some(stack.id);
                    let mut vb_stack = self
                        .data_mut()
                        .branches
                        .remove(&new_stack_id)
                        .expect("just added");
                    vb_stack.id = stack.id;
                    self.data_mut().branches.insert(stack.id, vb_stack);
                    let vb_stack = self
                        .data_mut()
                        .branches
                        .get_mut(&stack.id)
                        .expect("just added");
                    seen_stack_ids.insert(stack.id);
                    vb_stack
                }
                Some(stack_id) => self
                    .data_mut()
                    .branches
                    .get_mut(&stack_id)
                    .expect("we just looked it up"),
            };
            for branch in branches_to_create {
                vb_stack.heads.push(branch_to_stack_branch(
                    branch.ref_name.as_ref(),
                    &Branch::default(),
                    branch.archived,
                ))
            }
            vb_stack.in_workspace = stack.is_in_workspace();
            vb_stack.heads.sort_by_key(|head| {
                stack.branches.iter().enumerate().find_map(|(idx, branch)| {
                    (branch.ref_name.shorten() == head.name.as_str()).then_some(idx)
                })
            });

            // remove heads that aren't there anymore.
            vb_stack.heads.retain(|head| {
                stack
                    .branches
                    .iter()
                    .any(|branch| branch.ref_name.shorten() == head.name)
            });
            // branches now match our order
            for (vb_stack, stack) in vb_stack.heads.iter_mut().zip(stack.branches.iter()) {
                vb_stack.archived = stack.archived;
            }
            vb_stack.heads.reverse()
        }

        for (stack_idx, stack) in value.stacks.iter().enumerate() {
            if let Some(vb_stack) = self.data_mut().branches.get_mut(&stack.id) {
                vb_stack.order = stack_idx;
            }
        }

        let stacks_to_delete: Vec<_> = self
            .data()
            .branches
            .keys()
            .copied()
            .filter(|sid| !seen_stack_ids.contains(sid))
            .collect();
        for sid in stacks_to_delete {
            self.data_mut().branches.remove(&sid);
        }

        let new_target_branch = value
            .target_ref
            .as_ref()
            .map(|rn| branch_from_ref_name(rn.as_ref()))
            .transpose()?;
        // We don't support initialising this yet, for now just changes.
        let mut changed_target = false;
        match (&mut self.data_mut().default_target, new_target_branch) {
            (existing @ Some(_), None) => {
                // Have to clear everything then due to limitations of the data structure.
                *existing = None;
                changed_target = true;
            }
            (None, Some(_new)) => {
                bail!(
                    "Cannot reasonably set a target in the old data structure as we don't have repo access here"
                )
            }
            (Some(existing), Some(new)) => {
                if existing.branch != new {
                    existing.branch = new;
                    changed_target = true;
                }
                if let Some(new_id) = value.target_commit_id
                    && new_id != existing.sha
                {
                    existing.sha = new_id;
                }
            }
            (None, None) => {}
        }

        if let Some(target) = self.data_mut().default_target.as_mut()
            && target.push_remote_name != value.push_remote
        {
            target.push_remote_name = value.push_remote.clone();
            changed_target = true;
        }

        if changed_target {
            self.snapshot.set_changed_to_necessitate_write();
        }
        Ok(())
    }

    fn set_branch(&mut self, value: &Self::Handle<Branch>) -> anyhow::Result<()> {
        let ref_name = value.ref_name.as_ref();
        let stack_id = *value.stack_id.borrow();
        let ws = self.workspace(INTEGRATION_BRANCH.try_into().unwrap())?;
        match stack_id {
            Some(stack_id) => {
                let stack = self
                    .snapshot
                    .content
                    .branches
                    .get_mut(&stack_id)
                    .with_context(|| format!("Couldn't find stack with id {stack_id}"))?;

                let short_name = ref_name.shorten();
                let StackBranch {
                    description,
                    pr_number,
                    archived,
                    review_id,
                    ..
                } = stack
                    .heads
                    .iter_mut()
                    .find(|b| short_name == b.name.as_str())
                    .expect(
                        "It's not possible anymore to place values at any ref \
                    - one first has to get them, which binds values to their name.",
                    );

                let metadata_stack_indices =
                    ws.find_owner_indexes_by_name(ref_name, AppliedAndUnapplied);
                self.snapshot.changed_at = Some(Instant::now());
                *description = value.description.clone();
                *pr_number = value.review.pull_request;
                *review_id = value.review.review_id.clone();
                if let Some((stack_idx, segment_idx)) = metadata_stack_indices {
                    let meta_stack = &ws.stacks[stack_idx];
                    stack.in_workspace = meta_stack.is_in_workspace();
                    *archived = meta_stack.branches[segment_idx].archived;
                }
                Ok(())
            }
            None => {
                let now_ms = (gix::date::Time::now_utc().seconds * 1000) as u128;
                let stack = Stack::new_with_just_heads(
                    vec![branch_to_stack_branch(ref_name, value, false)],
                    now_ms,
                    self.data().branches.len(),
                    ws.contains_ref(ref_name, Applied),
                );
                *value.stack_id.borrow_mut() = Some(stack.id);
                self.data_mut().branches.insert(stack.id, stack);
                self.snapshot.set_changed_to_necessitate_write();
                Ok(())
            }
        }
    }

    fn remove(&mut self, ref_name: &FullNameRef) -> anyhow::Result<bool> {
        if is_workspace_ref_name(ref_name) {
            // There is only one workspace, and it's the same as deleting everything.
            // The real implementation of this would just delete data associated with a ref, no special case needed there.
            if let Err(err) = std::fs::remove_file(&self.snapshot.path) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    Err(err.into())
                } else {
                    Ok(false)
                }
            } else {
                let existed_as_non_default =
                    Self::workspace_from_data(self.data()) != default_workspace();
                self.snapshot.content = Default::default();
                // Make sure it's not going to be written in its default state.
                self.snapshot.claim_unchanged();
                Ok(existed_as_non_default)
            }
        } else {
            Ok(self.remove_branch(ref_name)?.is_some())
        }
    }
}

fn branch_from_ref_name(ref_name: &FullNameRef) -> anyhow::Result<RemoteRefname> {
    let (category, short_name) = ref_name
        .category_and_short_name()
        .context("couldn't classify supposed remote tracking branch")?;
    if category != Category::RemoteBranch {
        bail!(
            "Cannot set target branches to a branch that isn't a remote tracking branch: '{short_name}'"
        );
    }

    // TODO: remove this as we don't handle symbolic names with slashes correctly.
    //       At least try to not always set this value, but this test is also ambiguous.
    let slash_pos = short_name
        .find_byte(b'/')
        .context("remote branch didn't have '/' in the name, but should be 'origin/foo'")?;
    Ok(RemoteRefname::new(
        short_name[..slash_pos].to_str_lossy().as_ref(),
        short_name[slash_pos + 1..].to_str_lossy().as_ref(),
    ))
}

impl VirtualBranchesTomlMetadata {
    fn workspace_from_data(data: &VirtualBranches) -> Workspace {
        let (target_branch, target_commit_id, push_remote) = data
            .default_target
            .as_ref()
            .map(|target| {
                (
                    gix::refs::FullName::try_from(target.branch.to_string()).ok(),
                    (!target.sha.is_null()).then_some(target.sha),
                    target.push_remote_name.clone(),
                )
            })
            .unwrap_or_default();

        let mut stacks: Vec<_> = data.branches.values().cloned().collect();
        stacks.sort_by_key(|s| s.order);

        Workspace {
            ref_info: managed_ref_info(),
            stacks: stacks
                .iter()
                // We aren't able to handle these well, so let's ignore them.
                .filter(|stack| !stack.heads.is_empty())
                .sorted_by_key(|s| s.order)
                .map(|s| WorkspaceStack {
                    id: s.id,
                    workspacecommit_relation: if s.in_workspace {
                        WorkspaceCommitRelation::Merged
                    } else {
                        WorkspaceCommitRelation::Outside
                    },
                    branches: s
                        .heads
                        .iter()
                        .rev()
                        .filter_map(|sb| {
                            full_branch_name(sb.name.as_str()).map(|ref_name| {
                                WorkspaceStackBranch {
                                    ref_name,
                                    archived: sb.archived,
                                }
                            })
                        })
                        .collect(),
                })
                .collect(),
            target_ref: target_branch,
            target_commit_id,
            push_remote,
        }
    }

    fn remove_branch(&mut self, ref_name: &FullNameRef) -> anyhow::Result<Option<StackBranch>> {
        let branch = self.branch(ref_name)?;
        if branch.is_default() {
            return Ok(None);
        }

        let Some((stack_id, branch_idx)) = self.data().branches.values().find_map(|stack| {
            stack
                .heads
                .iter()
                .enumerate()
                .find_map(|(branch_idx, branch)| {
                    full_branch_name(branch.name.as_str()).and_then(|full_name| {
                        (full_name.as_ref() == ref_name).then_some((stack.id, branch_idx))
                    })
                })
        }) else {
            return Ok(None);
        };

        let stack = self
            .data_mut()
            .branches
            .get_mut(&stack_id)
            .expect("still there");
        let removed = stack.heads.remove(branch_idx);
        if stack.heads.is_empty() {
            self.data_mut().branches.remove(&stack_id);
        }
        self.snapshot.set_changed_to_necessitate_write();
        Ok(Some(removed))
    }
}

pub struct VBTomlMetadataHandle<T> {
    is_default: bool,
    ref_name: gix::refs::FullName,
    // Allow faster lookup next time. This is more like a PoC,
    // other storage backends like database may have similar handles to avoid searches by name.
    stack_id: RefCell<Option<StackId>>,
    value: T,
}

impl<T> VBTomlMetadataHandle<T> {
    /// Return the stack_id of the underlying stack if there is one.
    pub fn stack_id(&self) -> Option<StackId> {
        *self.stack_id.borrow()
    }
}

impl<T> AsRef<FullNameRef> for VBTomlMetadataHandle<T> {
    fn as_ref(&self) -> &FullNameRef {
        self.ref_name.as_ref()
    }
}

impl<T> Deref for VBTomlMetadataHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for VBTomlMetadataHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> ValueInfo for VBTomlMetadataHandle<T> {
    fn is_default(&self) -> bool {
        self.is_default
    }
}

/// We can't store time, so put a placeholder that helps to mimic proper behaviour.
fn standard_time() -> gix::date::Time {
    gix::date::Time::new(1675176957, 0)
}

fn default_workspace() -> Workspace {
    Workspace {
        ref_info: RefInfo {
            created_at: Some(standard_time()),
            updated_at: None,
        },
        ..Default::default()
    }
}

fn full_branch_name(name: &str) -> Option<gix::refs::FullName> {
    gix::refs::FullName::try_from(format!("refs/heads/{name}")).ok()
}

/// Make it appear managed, which it is as we created it. Can only make the date up though,
/// which shouldn't matter yet. Let's hope we never use the time while this store is in play.
fn managed_ref_info() -> RefInfo {
    RefInfo {
        created_at: Some(standard_time()),
        updated_at: None,
    }
}

fn branch_to_stack_branch(
    ref_name: &gix::refs::FullNameRef,
    Branch {
        ref_info: _, // TODO: should change parent stack if it's the top.
        description,
        review,
    }: &Branch,
    archived: bool,
) -> StackBranch {
    StackBranch::new_with_zero_head(
        ref_name.shorten().to_string(),
        description.clone(),
        review.pull_request,
        review.review_id.clone(),
        archived,
    )
}

/// Copied from `gitbutler-fs` - shouldn't be needed anymore in future.
mod fs {
    use std::{
        fs::File,
        io::{Read, Write},
        path::Path,
    };

    use anyhow::Context as _;
    use gix::tempfile::{AutoRemove, ContainingDirectory};
    use serde::de::DeserializeOwned;

    /// Write a single file so that the write either fully succeeds, or fully fails,
    /// assuming the containing directory already exists.
    pub fn write<P: AsRef<Path>>(file_path: P, contents: impl AsRef<[u8]>) -> anyhow::Result<()> {
        let mut temp_file = gix::tempfile::new(
            file_path.as_ref().parent().unwrap(),
            ContainingDirectory::Exists,
            AutoRemove::Tempfile,
        )?;
        temp_file.write_all(contents.as_ref())?;
        Ok(persist_tempfile(temp_file, file_path)?)
    }

    fn persist_tempfile(
        tempfile: gix::tempfile::Handle<gix::tempfile::handle::Writable>,
        to_path: impl AsRef<Path>,
    ) -> std::io::Result<()> {
        match tempfile.persist(to_path) {
            Ok(Some(_opened_file)) => Ok(()),
            Ok(None) => unreachable!(
                "BUG: a signal has caused the tempfile to be removed, but we didn't install a handler"
            ),
            Err(err) => Err(err.error),
        }
    }

    /// Reads and parses the state file.
    ///
    /// If the file does not exist, it will be created.
    pub fn read_toml_file_or_default<T: DeserializeOwned + Default>(
        path: &Path,
    ) -> anyhow::Result<T> {
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
            Err(err) => return Err(err.into()),
        };
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let value: T = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(value)
    }
}

use anyhow::{Context, bail};
use but_core::RefMetadata;
use but_core::ref_metadata::{
    Branch, RefInfo, ValueInfo, Workspace, WorkspaceStack, WorkspaceStackBranch,
};
use gitbutler_stack::{StackId, VirtualBranchesState};
use gix::date::SecondsSinceUnixEpoch;
use gix::refs::{FullName, FullNameRef};
use std::any::Any;
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Snapshot {
    /// The time at which the `content` was changed, before it was written to disk.
    changed_at: Option<Instant>,
    content: VirtualBranchesState,
    path: PathBuf,
}

impl Snapshot {
    fn from_path(path: PathBuf) -> anyhow::Result<Self> {
        let content = gitbutler_fs::read_toml_file_or_default(&path)?;
        Ok(Self {
            path,
            changed_at: None,
            content,
        })
    }

    fn write_if_changed(&mut self) -> anyhow::Result<()> {
        if self.changed_at.is_some() {
            if self.content == Default::default() {
                std::fs::remove_file(&self.path)?;
            } else {
                gitbutler_fs::write(&self.path, toml::to_string(&self.content)?)?;
            }
            self.changed_at.take();
        }
        Ok(())
    }

    fn try_write_if_changed(&mut self) {
        let res = self.write_if_changed();
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
}

/// An implementation to read and write metadata from the `virtual_branches.toml` file, meant to be a short-lived item
/// that is possibly written multiple times. It will write itself on drop only, and log write failures.
///
/// The idea is that it's forgiving and easy to use, while helping to eventually migrate to a database.
pub struct VirtualBranchesTomlMetadata {
    // What is currently in memory for query or edits.
    snapshot: Snapshot,
}

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

// Emergency-behaviour in case the application winds down, we don't want data-loss (at least a chance).
impl Drop for VirtualBranchesTomlMetadata {
    fn drop(&mut self) {
        self.snapshot.try_write_if_changed();
    }
}

const INTEGRATION_BRANCH_LEGACY: &str = "refs/heads/gitbutler/integration";
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
                .filter_map(|branch| full_branch_name(branch.name()))
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
        if is_workspace_ref(ref_name) {
            let value = Self::workspace_from_data(&self.snapshot.content);
            Ok(VBTomlMetadataHandle {
                is_default: value == default_workspace(),
                stack_id: None.into(),
                value,
            })
        } else {
            bail!("This backend doesn't support arbitrary workspaces");
        }
    }

    fn branch(&self, ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
        let Some((stack, branch)) = self.snapshot.content.branches.values().find_map(|stack| {
            stack.heads.iter().find_map(|branch| {
                full_branch_name(branch.name().as_str()).and_then(|full_name| {
                    (full_name.as_ref() == ref_name).then_some((stack, branch))
                })
            })
        }) else {
            return Ok(VBTomlMetadataHandle {
                is_default: true,
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
                ..gix::date::Time::now_local_or_utc()
            }),
        };
        Ok(VBTomlMetadataHandle {
            is_default: false,
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

    fn set_workspace(
        &mut self,
        ref_name: &FullNameRef,
        value: &Self::Handle<Workspace>,
    ) -> anyhow::Result<()> {
        if !is_workspace_ref(ref_name) {
            bail!("This backend doesn't support arbitrary workspaces");
        }

        // Find exactly one stack-id per branch name, and assign all branches to it.
        // `stacks` is the target state, and we have to make an actual stack look like it.
        for stack in &value.stacks {
            let stack_branches = &stack.branches;
            let mut branches_without_data = Vec::new();
            let mut stack_id = None::<StackId>;
            for stack_branch in stack_branches {
                let branch = self.branch(stack_branch.ref_name.as_ref())?;
                if branch.is_default() {
                    branches_without_data.push(stack_branch);
                    continue;
                }
                if stack_id.is_none() {
                    stack_id = *branch.stack_id.borrow();
                } else if stack_id != *branch.stack_id.borrow() {
                    *branch.stack_id.borrow_mut() = stack_id;
                    self.set_branch(stack_branch.ref_name.as_ref(), &branch)?;
                }
            }

            let stack = match stack_id {
                None => {
                    todo!("create a new stack with all ref-names")
                }
                Some(stack_id) => {
                    let stack = self
                        .snapshot
                        .content
                        .branches
                        .get_mut(&stack_id)
                        .expect("we just looked it up");

                    for branch in branches_without_data {
                        stack.heads.push(branch_to_stack_branch(
                            branch.ref_name.as_ref(),
                            &Branch::default(),
                            branch.archived,
                        ))
                    }
                    stack.in_workspace = !stack.heads.is_empty();
                    stack
                }
            };
            stack.heads.sort_by_key(|head| {
                stack_branches.iter().enumerate().find_map(|(idx, branch)| {
                    (branch.ref_name.shorten() == head.name().as_str()).then_some(idx)
                })
            });
            stack.heads.reverse()
        }
        Ok(())
    }

    fn set_branch(
        &mut self,
        ref_name: &FullNameRef,
        value: &Self::Handle<Branch>,
    ) -> anyhow::Result<()> {
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
                match stack
                    .heads
                    .iter_mut()
                    .find(|b| short_name == b.name().as_str())
                {
                    None => {
                        todo!("insert into existing stack")
                    }
                    Some(gitbutler_stack::StackBranch {
                        description,
                        pr_number,
                        archived,
                        review_id,
                        ..
                    }) => {
                        let stack_branch = ws.find_branch(ref_name);
                        self.snapshot.changed_at = Some(Instant::now());
                        *description = value.description.clone();
                        *pr_number = value.review.pull_request;
                        *review_id = value.review.review_id.clone();
                        stack.in_workspace = stack_branch.is_some();
                        if let Some(stack_branch) = stack_branch {
                            *archived = stack_branch.archived;
                        }
                        Ok(())
                    }
                }
            }
            None => {
                let now_ms = (gix::date::Time::now_local_or_utc().seconds * 1000) as u128;
                let stack = gitbutler_stack::Stack {
                    id: StackId::default(),
                    created_timestamp_ms: now_ms,
                    updated_timestamp_ms: now_ms,
                    order: self.snapshot.content.branches.len(),
                    allow_rebasing: true, //  default in V2
                    in_workspace: ws.contains_ref(ref_name),
                    heads: vec![branch_to_stack_branch(ref_name, value, false)],

                    // Don't keep redundant information
                    tree: git2::Oid::zero(),
                    head: git2::Oid::zero(),
                    source_refname: None,
                    upstream: None,
                    upstream_head: None,

                    // Unused - everything is defined by the top-most branch name.
                    name: "".to_string(),
                    notes: "".to_string(),

                    // Related to ownership, obsolete.
                    selected_for_changes: None,
                    // unclear, obsolete
                    not_in_workspace_wip_change_id: None,
                    // unclear
                    post_commits: false,
                    ownership: Default::default(),
                };
                *value.stack_id.borrow_mut() = Some(stack.id);
                self.snapshot.content.branches.insert(stack.id, stack);
                self.snapshot.changed_at = Some(Instant::now());
                Ok(())
            }
        }
    }

    fn remove(&mut self, ref_name: &FullNameRef) -> anyhow::Result<bool> {
        if is_workspace_ref(ref_name) {
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
                    Self::workspace_from_data(&self.snapshot.content) != default_workspace();
                self.snapshot.content = Default::default();
                // Make sure it's not going to be written in its default state.
                self.snapshot.claim_unchanged();
                Ok(existed_as_non_default)
            }
        } else {
            let branch = self.branch(ref_name)?;
            if branch.is_default() {
                return Ok(false);
            }

            let Some((stack_id, branch_idx)) =
                self.snapshot.content.branches.values().find_map(|stack| {
                    stack
                        .heads
                        .iter()
                        .enumerate()
                        .find_map(|(branch_idx, branch)| {
                            full_branch_name(branch.name().as_str()).and_then(|full_name| {
                                (full_name.as_ref() == ref_name).then_some((stack.id, branch_idx))
                            })
                        })
                })
            else {
                return Ok(false);
            };

            let stack = self
                .snapshot
                .content
                .branches
                .get_mut(&stack_id)
                .expect("still there");
            stack.heads.remove(branch_idx);
            if stack.heads.is_empty() {
                self.snapshot.content.branches.remove(&stack_id);
            }
            self.snapshot.changed_at = Some(Instant::now());
            Ok(true)
        }
    }
}

impl VirtualBranchesTomlMetadata {
    fn workspace_from_data(data: &VirtualBranchesState) -> Workspace {
        let target_branch = data
            .default_target
            .as_ref()
            .and_then(|target| gix::refs::FullName::try_from(target.branch.to_string()).ok());

        let mut stacks: Vec<_> = data.branches.values().cloned().collect();
        stacks.sort_by_key(|s| s.order);

        let workspace = but_core::ref_metadata::Workspace {
            ref_info: managed_ref_info(),
            stacks: stacks
                .iter()
                .filter(|s| s.in_workspace)
                .map(|s| WorkspaceStack {
                    branches: s
                        .heads
                        .iter()
                        .rev()
                        .filter_map(|sb| {
                            full_branch_name(sb.name()).map(|ref_name| WorkspaceStackBranch {
                                ref_name,
                                archived: sb.archived,
                            })
                        })
                        .collect(),
                })
                .collect(),
            target_ref: target_branch,
        };
        workspace
    }
}

pub struct VBTomlMetadataHandle<T> {
    is_default: bool,
    // Allow faster lookup next time. This is more like a PoC,
    // other storage backends like database may have similar handles to avoid searches by name.
    stack_id: RefCell<Option<StackId>>,
    value: T,
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

fn is_workspace_ref(ref_name: &FullNameRef) -> bool {
    ref_name.as_bstr() == INTEGRATION_BRANCH || ref_name.as_bstr() == INTEGRATION_BRANCH_LEGACY
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
) -> gitbutler_stack::StackBranch {
    gitbutler_stack::StackBranch {
        name: ref_name.shorten().to_string(),
        description: description.clone(),
        pr_number: review.pull_request,
        archived,
        review_id: review.review_id.clone(),

        // Redundant, unused.
        head: gitbutler_stack::CommitOrChangeId::CommitId(git2::Oid::zero().to_string()),
    }
}

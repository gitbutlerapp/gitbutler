use core::fmt;
use std::{
    borrow::Cow,
    cmp::max,
    collections::{BTreeSet, HashMap, HashSet},
    fmt::Debug,
    vec,
};

use anyhow::{Context as _, Result, bail};
use bstr::{BStr, BString, ByteSlice};
use but_ctx::{Context, access::WorktreeReadPermission};
use but_oxidize::{git2_to_gix_object_id, gix_to_git2_oid};
use but_serde::BStringForFrontend;
use gitbutler_branch::{BranchIdentity, ReferenceExtGix};
use gitbutler_diff::DiffByPathMap;
use gitbutler_reference::{RemoteRefname, normalize_branch_name};
use gitbutler_stack::{StackId, Target};
use gix::{object::tree::diff::Action, prelude::TreeDiffChangeExt, reference::Category};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{RemoteBranchFile, VirtualBranchesExt, gravatar::gravatar_url_from_email};

#[instrument(level = tracing::Level::DEBUG, skip(ctx, _permission))]
pub(crate) fn get_uncommitted_files_raw(
    ctx: &Context,
    _permission: &WorktreeReadPermission,
) -> Result<DiffByPathMap> {
    gitbutler_diff::workdir(
        &*ctx.git2_repo.get()?,
        ctx.git2_repo.get()?.head()?.peel_to_commit()?.id(),
    )
    .context("Failed to list uncommitted files")
}

pub(crate) fn get_uncommitted_files(
    context: &Context,
    _permission: &WorktreeReadPermission,
) -> Result<Vec<RemoteBranchFile>> {
    let files = get_uncommitted_files_raw(context, _permission)?
        .into_values()
        .map(|file| file.into())
        .collect();

    Ok(files)
}

/// Returns a list of branches associated with this project.
pub fn list_branches(
    ctx: &Context,
    filter: Option<BranchListingFilter>,
    filter_branch_names: Option<Vec<BranchIdentity>>,
) -> Result<Vec<BranchListing>> {
    let mut repo = ctx.repo.get()?.clone();
    repo.object_cache_size_if_unset(1024 * 1024);
    let has_filter = filter.is_some();
    let filter = filter.unwrap_or_default();
    let platform = repo.references()?;
    let mut branches: Vec<GroupBranch> = vec![];
    for reference in platform.all()?.filter_map(Result::ok) {
        // Loosely match on branch names
        if let Some(branch_names) = &filter_branch_names {
            let has_matching_name = branch_names.iter().any(|branch_name| {
                reference
                    .name()
                    .as_bstr()
                    .ends_with_str(branch_name.as_bstr())
            });

            if !has_matching_name {
                continue;
            }
        }

        let is_local_branch = match reference.name().category() {
            Some(Category::LocalBranch) => true,
            Some(Category::RemoteBranch) => false,
            _ => continue,
        };
        branches.push(if is_local_branch {
            GroupBranch::Local(reference)
        } else {
            GroupBranch::Remote(reference)
        });
    }

    let vb_handle = ctx.legacy_project.virtual_branches();
    let remote_names = repo.remote_names();
    let stacks = {
        if let Some(workspace_ref) = repo.try_find_reference("refs/heads/gitbutler/workspace")? {
            // Let's get this here for convenience, and hope this isn't ever called by a writer (or there will be a deadlock).
            let read_guard = ctx.shared_worktree_access();
            let meta = ctx.meta(read_guard.read_permission())?;
            let info = but_workspace::ref_info(
                workspace_ref,
                &meta,
                but_workspace::ref_info::Options {
                    traversal: but_graph::init::Options::limited(),
                    expensive_commit_info: false,
                },
            )?;
            info.stacks
                .into_iter()
                .filter_map(|s| GitButlerStack::try_new(s, &remote_names).transpose())
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        }
    };

    branches.extend(stacks.iter().map(|s| GroupBranch::Virtual(s.clone())));
    let mut branches = combine_branches(branches, &repo, vb_handle.get_default_target()?)?;

    // Apply the filter
    branches.retain(|branch| !has_filter || matches_all(branch, filter));

    // Filter out virtual branches which have no local or remote branches
    branches.retain(|branch| {
        // If there is no virtual branch, keep the grouping
        let Some(virtual_branch) = &branch.stack else {
            return true;
        };

        // If the virtual branch is applied, keep the grouping
        if virtual_branch.in_workspace {
            return true;
        }

        // If the virtual branch has a local branch, keep the grouping
        if branch.has_local {
            return true;
        };

        // If the virtual branch has remotes, keep the grouping
        if !branch.remotes.is_empty() {
            return true;
        };

        // Otherwise, drop the grouping
        false
    });

    if let Some(branch_names) = filter_branch_names {
        branches.retain(|branch_listing| branch_names.contains(&branch_listing.name))
    }

    // We want to exclude branches that are already part of a stack.
    // To do this, we build up a list of all the branch identities that are
    // part of a stack and then filter out any branches that have been grouped
    // without a stack and match one of these identities.
    let branch_identities_to_exclude: HashSet<BString> = stacks
        .into_iter()
        .flat_map(|s| {
            s.unarchived_segments
                .into_iter()
                .map(|b| b.short_name().into())
                .chain(Some(s.name.into()))
        })
        .collect::<HashSet<_>>();

    branches.retain(|branch| {
        if branch.stack.is_some() {
            return true;
        }

        if branch_identities_to_exclude.contains(&(*branch.name).to_owned()) {
            return false;
        }

        true
    });

    Ok(branches)
}

fn matches_all(branch: &BranchListing, filter: BranchListingFilter) -> bool {
    let mut conditions = vec![];
    if let Some(applied) = filter.applied {
        if let Some(vb) = branch.stack.as_ref() {
            conditions.push(applied == vb.in_workspace);
        } else {
            conditions.push(!applied);
        }
    }
    if let Some(local) = filter.local {
        conditions.push((branch.has_local || branch.stack.is_some()) && local);
    }
    conditions.iter().all(|&x| x)
}

fn combine_branches(
    group_branches: Vec<GroupBranch>,
    repo: &gix::Repository,
    target_branch: Target,
) -> Result<Vec<BranchListing>> {
    let remotes = repo.remote_names();
    let packed = repo.refs.cached_packed_buffer()?;

    // Group branches by identity
    let mut groups: HashMap<BranchIdentity, Vec<GroupBranch>> = HashMap::new();
    for branch in group_branches {
        // Skip the target branch, like 'main' or 'master'
        if branch.is_remote_branch(&target_branch.branch) {
            continue;
        }

        let Some(identity) = branch.identity(&remotes) else {
            continue;
        };
        // Skip branches that should not be listed, e.g. the gitbutler technical branches like 'gitbutler/workspace'
        if !should_list_git_branch(&identity) {
            continue;
        }
        groups.entry(identity).or_default().push(branch);
    }

    // Convert to Branch entries for the API response, filtering out any errors
    Ok(groups
        .into_iter()
        .filter_map(|(identity, group_branches)| {
            let res = branch_group_to_branch(
                &identity,
                group_branches,
                repo,
                packed.as_ref().map(|p| &***p),
                &remotes,
                &target_branch,
            );
            match res {
                Ok(branch_entry) => branch_entry,
                Err(err) => {
                    tracing::warn!(
                        "Failed to process branch group {:?} to branch entry: {}",
                        identity,
                        err
                    );
                    None
                }
            }
        })
        .collect())
}

/// Converts a group of branches with the same identity into a single branch entry
fn branch_group_to_branch(
    identity: &BranchIdentity,
    group_branches: Vec<GroupBranch>,
    repo: &gix::Repository,
    packed: Option<&gix::refs::packed::Buffer>,
    remotes: &BTreeSet<Cow<'_, BStr>>,
    target: &Target,
) -> Result<Option<BranchListing>> {
    let (local_branches, remote_branches, mut vbranches) =
        group_branches
            .into_iter()
            .fold((Vec::new(), Vec::new(), Vec::new()), |mut acc, item| {
                match item {
                    GroupBranch::Local(branch) => acc.0.push(branch),
                    GroupBranch::Remote(branch) => acc.1.push(branch),
                    GroupBranch::Virtual(branch) => acc.2.push(branch),
                }
                acc
            });

    let virtual_branch = if vbranches.len() > 1 {
        vbranches.sort_by_key(|virtual_branch| virtual_branch.updated_timestamp_ms);
        vbranches.last()
    } else {
        vbranches.first()
    };

    if virtual_branch.is_none()
        && local_branches.iter().any(|b| {
            b.name()
                .identity(remotes)
                .as_deref()
                .ok()
                .is_some_and(|identity| identity == target.branch.branch())
        })
    {
        return Ok(None);
    }

    // Virtual branch associated with this branch
    let virtual_branch_reference = virtual_branch.map(|stack| {
        let unarchived_branches = stack.unarchived_segments.iter();
        StackReference {
            given_name: stack.name.clone(),
            id: stack.id,
            in_workspace: stack.in_workspace,
            branches: unarchived_branches
                .clone()
                .map(|b| b.short_name())
                .collect(),
            pull_requests: unarchived_branches
                .filter_map(|b| b.pr_or_mr.map(|pr| (b.short_name().to_owned(), pr)))
                .collect(),
        }
    });

    let mut remotes: Vec<gix::remote::Name<'static>> = Vec::new();
    for branch in remote_branches.iter() {
        if let Some(remote_name) = branch.remote_name(gix::remote::Direction::Fetch) {
            remotes.push(remote_name.to_owned());
        }
    }

    // Virtual branches always have local branches
    let has_local = !local_branches.is_empty() || virtual_branch.is_some();

    // The head commit for which we calculate statistics.
    // If there is a virtual branch let's get it's head. Alternatively, pick the first local branch and use it's head.
    // If there are no local branches, pick the first remote branch.
    let head_commit = if let Some(vbranch) = virtual_branch {
        Some(vbranch.head_oid(repo)?)
    } else if let Some(mut branch) = local_branches.into_iter().next() {
        branch.peel_to_id_packed(packed).ok()
    } else if let Some(mut branch) = remote_branches.into_iter().next() {
        branch.peel_to_id_packed(packed).ok()
    } else {
        None
    }
    .context("Could not get any valid reference in order to build branch stats")?;

    let head = gix_to_git2_oid(head_commit.detach());
    let head_commit = head_commit.object()?.try_into_commit()?;
    let head_commit = head_commit.decode()?;
    let last_modified_ms = max(
        (head_commit.time()?.seconds * 1000) as u128,
        virtual_branch.map_or(0, |x| x.updated_timestamp_ms),
    );
    let last_commiter = head_commit.author()?.into();

    Ok(Some(BranchListing {
        name: identity.to_owned(),
        remotes,
        stack: virtual_branch_reference,
        updated_at: last_modified_ms,
        last_commiter,
        has_local,
        head,
    }))
}

/// A sum type of branch that can be a plain git branch or a virtual branch
enum GroupBranch<'a> {
    Local(gix::Reference<'a>),
    Remote(gix::Reference<'a>),
    Virtual(GitButlerStack),
}

/// A type to just keep the parts we currently need.
#[derive(Debug, Clone)]
struct GitButlerStack {
    id: StackId,
    /// `true` if the stack is applied to the workspace.
    in_workspace: bool,
    /// The short name of the top-most segment.
    name: String,
    /// The full ref name of the top-most segment.
    source_refname: Option<gix::refs::FullName>,
    /// The full ref name of the remote tracking branch of the top-most segment.
    upstream: Option<but_workspace::ui::ref_info::RemoteTrackingReference>,
    /// The time at which anything in the stack was last updated.
    updated_timestamp_ms: u128,
    // All segments of the stack, as long as they are not archived.
    // The tip comes first.
    unarchived_segments: Vec<GitbutlerStackSegment>,
}

#[derive(Debug, Clone)]
struct GitbutlerStackSegment {
    /// The name of the segment, without support for these to be anonymous (which is a problem).
    tip: gix::refs::FullName,
    /// The PR or MR associated with it.
    pr_or_mr: Option<usize>,
}

impl GitbutlerStackSegment {
    fn short_name(&self) -> String {
        self.tip.shorten().to_string()
    }
}

impl GitButlerStack {
    fn try_new(
        stack: but_workspace::branch::Stack,
        names: &gix::remote::Names,
    ) -> anyhow::Result<Option<Self>> {
        let Some(id) = stack.id else { return Ok(None) };
        let first_segment = stack.segments.first();
        Ok(Some(GitButlerStack {
            id,
            // The ones we have reachable are never
            in_workspace: true,
            name: stack
                .name()
                .map(|rn| rn.shorten().to_string())
                // Hack it - the datastructure isn't suitable and this needs a `gitbutler->but` port.
                .unwrap_or_default(),
            source_refname: stack.ref_name().map(|rn| rn.to_owned()),
            upstream: first_segment
                .and_then(|s| {
                    s.remote_tracking_ref_name.as_ref().map(|rn| {
                        but_workspace::ui::ref_info::RemoteTrackingReference::for_ui(
                            rn.clone(),
                            names,
                        )
                    })
                })
                .transpose()?,
            updated_timestamp_ms: first_segment
                .and_then(|s| {
                    let md = s.metadata.as_ref()?;
                    Some(md.ref_info.updated_at?.seconds as u128 * 1_000)
                })
                .unwrap_or_default(),
            unarchived_segments: stack
                .segments
                .iter()
                .map(|s| GitbutlerStackSegment {
                    tip: s.ref_info.clone().map(|ri| ri.ref_name).unwrap_or_else(|| {
                        gix::refs::FullName::try_from(
                            "refs/heads/unnamed-ref-and-we-fake-a-name-fix-me",
                        )
                        .expect("known to be valid statically")
                    }),
                    pr_or_mr: s.metadata.as_ref().and_then(|md| md.review.pull_request),
                })
                .collect(),
        }))
    }
}

impl GitButlerStack {
    /// Return the top-most stack's commit id.
    fn head_oid<'repo>(&self, repo: &'repo gix::Repository) -> anyhow::Result<gix::Id<'repo>> {
        let tip_ref = self
            .unarchived_segments
            .iter()
            .map(|s| s.tip.as_ref())
            .next()
            .with_context(|| format!("Stack {} didn't have a tip ref name", self.id))?;
        repo.find_reference(tip_ref)?
            .try_id()
            .with_context(|| format!("'{}' was as symbolic reference", tip_ref.shorten()))
    }
}

impl fmt::Debug for GroupBranch<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupBranch::Local(branch) | GroupBranch::Remote(branch) => formatter
                .debug_struct("GroupBranch::Local/Remote")
                .field(
                    "0",
                    &format!(
                        "id: {:?}, name: {}",
                        branch.target(),
                        branch.name().as_bstr()
                    )
                    .as_str(),
                )
                .finish(),
            GroupBranch::Virtual(branch) => formatter
                .debug_struct("GroupBranch::Virtual")
                .field("0", branch)
                .finish(),
        }
    }
}

impl GroupBranch<'_> {
    /// A name identifier for the branch. When multiple branches (e.g. virtual, local, remote) have the same identity,
    /// they are grouped together under the same `Branch` entry.
    /// `None` means an identity could not be obtained, which makes this branch odd enough to ignore.
    fn identity(&self, remotes: &BTreeSet<Cow<'_, BStr>>) -> Option<BranchIdentity> {
        match self {
            GroupBranch::Local(branch) | GroupBranch::Remote(branch) => {
                branch.name().identity(remotes).ok()
            }
            // The identity of a Virtual branch is derived from the source refname, upstream or the branch given name, in that order
            GroupBranch::Virtual(branch) => {
                let name_from_source = branch.source_refname.as_ref().map(|n| n.shorten());
                let name_from_upstream = branch
                    .upstream
                    .as_ref()
                    .map(|n| n.display_name.as_str().into());

                // If we have a source refname or upstream, use those directly
                if let Some(name) = name_from_source.or(name_from_upstream) {
                    return name.try_into().ok();
                }

                // Only fall back to the normalized rich name if no source/upstream is available
                let rich_name = normalize_branch_name(&branch.name).ok()?;
                Some(rich_name.as_str().into())
            }
        }
    }

    /// Determines if the branch is a remote branch by ref name
    fn is_remote_branch(&self, ref_name: &RemoteRefname) -> bool {
        if let GroupBranch::Remote(branch) = self {
            ref_name == branch.name()
        } else {
            false
        }
    }
}

/// Determines if a branch should be listed in the UI.
/// This excludes the target branch as well as gitbutler specific branches.
fn should_list_git_branch(identity: &BranchIdentity) -> bool {
    // Exclude gitbutler technical branches (not useful for the user)
    const TECHNICAL_IDENTITIES: &[&[u8]] = &[
        b"gitbutler/edit",
        b"gitbutler/workspace",
        b"gitbutler/integration", // Remove me after transition.
        b"gitbutler/target",
        b"gitbutler/oplog",
        b"HEAD",
    ];
    !TECHNICAL_IDENTITIES.contains(&identity.as_bytes())
}

/// A filter that can be applied to the branch listing
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchListingFilter {
    /// If the value is true, the listing will only include branches that have local references or virtual branches.
    /// If the value is false, the listing will include only branches that have local references or virtual branches.
    pub local: Option<bool>,
    /// If the value is true, the listing will only include branches that are applied in the workspace.
    /// If the value is false, the listing will only include branches that are not applied in the workspace.
    pub applied: Option<bool>,
}

/// Represents a branch that exists for the repository
/// This also combines the concept of a remote, local and virtual branch in order to provide a unified interface for the UI
/// Branch entry is not meant to contain all the data a branch can have (e.g. full commit history, all files and diffs, etc.).
/// It is intended a summary that can be quickly retrieved and displayed in the UI.
/// For more detailed information, each branch can be queried individually for it's `BranchData`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchListing {
    /// The `identity` of the branch (e.g. `main`, `feature/branch`), excluding the remote name.
    pub name: BranchIdentity,
    /// This is a list of remotes that this branch can be found on (e.g. `origin`, `upstream` etc.),
    /// by collecting remotes from all local branches with the same identity that have a tracking setup.
    #[serde(serialize_with = "but_serde::as_string_lossy_vec_remote_name")]
    pub remotes: Vec<gix::remote::Name<'static>>,
    /// The branch may or may not have a virtual branch associated with it.
    pub stack: Option<StackReference>,
    /// Timestamp in milliseconds since the branch was last updated.
    /// This includes any commits, uncommitted changes or even updates to the branch metadata (e.g. renaming).
    pub updated_at: u128,
    /// The person who committed the head commit.
    pub last_commiter: Author,
    /// Whether there is a local branch under the name.
    pub has_local: bool,
    /// The head of interest for the branch group, used for calculating branch statistics.
    /// If there is a virtual branch, a local branch and remote branches, the head is determined in the following order:
    /// 1. The head of the virtual branch
    /// 2. The head of the first local branch
    /// 3. The head of the first remote branch
    #[serde(skip)]
    pub head: git2::Oid,
}

/// Represents a "commit author" or "signature", based on the data from the git history
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    /// The name of the author as configured in the git config
    pub name: Option<BStringForFrontend>,
    /// The email of the author as configured in the git config
    pub email: Option<BStringForFrontend>,

    pub gravatar_url: Option<BStringForFrontend>,
}

impl From<git2::Signature<'_>> for Author {
    fn from(value: git2::Signature) -> Self {
        let name = value.name().map(str::to_string).map(Into::into);
        let email = value.email().map(str::to_string).map(Into::into);

        let gravatar_url = match value.email() {
            Some(email) => gravatar_url_from_email(email)
                .map(|url| url.as_ref().into())
                .ok(),
            None => None,
        };

        Author {
            name,
            email,
            gravatar_url,
        }
    }
}

impl From<gix::actor::SignatureRef<'_>> for Author {
    fn from(value: gix::actor::SignatureRef<'_>) -> Self {
        let gravatar_url = {
            gravatar_url_from_email(&value.email.to_str_lossy())
                .map(|url| url.as_ref().into())
                .ok()
        };

        Author {
            name: Some(value.name.to_owned().into()),
            email: Some(value.email.to_owned().into()),
            gravatar_url,
        }
    }
}

/// Represents a reference to an associated virtual branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StackReference {
    /// A non-normalized name of the branch, set by the user
    pub given_name: String,
    /// Virtual Branch UUID identifier
    pub id: StackId,
    /// Determines if the virtual branch is applied in the workspace
    pub in_workspace: bool,
    /// List of branches that are part of the stack
    /// Ordered from newest to oldest (the most recent branch is first in the list)
    pub branches: Vec<String>,
    /// Pull Request numbers by branch name associated with the stack
    pub pull_requests: HashMap<String, usize>,
}

/// Takes a list of `branch_names` (the given name, as returned by `BranchListing`) and returns
/// a list of enriched branch data.
pub fn get_branch_listing_details(
    ctx: &Context,
    branch_names: impl IntoIterator<Item = impl TryInto<BranchIdentity>>,
) -> Result<Vec<BranchListingDetails>> {
    let branch_names: Vec<_> = branch_names
        .into_iter()
        .map(TryInto::try_into)
        .filter_map(Result::ok)
        .collect();
    let repo = ctx.clone_repo_for_merging()?;
    let branches = list_branches(ctx, None, Some(branch_names))?;

    let (default_target_current_upstream_commit_id, default_target_seen_at_last_update) = {
        let target = ctx
            .legacy_project
            .virtual_branches()
            .get_default_target()
            .context("failed to get default target")?;
        let target_branch_name = &target.branch.fullname();
        let target_branch_name = target_branch_name.as_str();
        let mut target_branch = repo.find_reference(target_branch_name)?;

        (
            gix_to_git2_oid(target_branch.peel_to_commit()?.id),
            target.sha,
        )
    };

    let mut enriched_branches = Vec::new();
    let (diffstats, merge_bases) = {
        let (start, start_rx) = std::sync::mpsc::channel::<(
            std::sync::mpsc::Receiver<gix::object::tree::diff::ChangeDetached>,
            std::sync::mpsc::Sender<(usize, usize, usize)>,
        )>();
        let diffstats = std::thread::Builder::new()
            .name("gitbutler-diff-stats".into())
            .spawn({
                let repo = repo.clone();
                move || -> Result<()> {
                    let mut resource_cache = repo.diff_resource_cache_for_tree_diff()?;
                    for (change_rx, res_tx) in start_rx {
                        let (mut number_of_files, mut lines_added, mut lines_removed) = (0, 0, 0);
                        for change in change_rx {
                            if let Some(counts) = change
                                .attach(&repo, &repo)
                                .diff(&mut resource_cache)
                                .ok()
                                .and_then(|mut platform| platform.line_counts().ok())
                                .flatten()
                            {
                                number_of_files += 1;
                                lines_added += counts.insertions as usize;
                                lines_removed += counts.removals as usize;
                            }
                            // Let's not attempt to reuse the cache as it's only useful if we know the diff repeats
                            // over different objects, like when doing rename tracking.
                            resource_cache.clear_resource_cache_keep_allocation();
                        }
                        if res_tx
                            .send((number_of_files, lines_added, lines_removed))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(())
                }
            })?;

        let all_other_branch_commit_ids: Vec<_> = branches
            .iter()
            .map(|branch| {
                (
                    branch
                        .stack
                        .as_ref()
                        .and_then(|vb| {
                            vb.in_workspace
                                .then_some(default_target_seen_at_last_update)
                        })
                        .unwrap_or(default_target_current_upstream_commit_id),
                    branch.head,
                )
            })
            .collect();
        let (merge_tx, merge_rx) = std::sync::mpsc::channel();
        let merge_bases = std::thread::Builder::new()
            .name("gitbutler-mergebases".into())
            .spawn({
                let repo = repo.clone().into_sync();
                move || -> anyhow::Result<()> {
                    let mut repo = repo.to_thread_local();
                    repo.object_cache_size_if_unset(50 * 1024 * 1024);
                    let cache = repo.commit_graph_if_enabled()?;
                    let mut graph = repo.revision_graph(cache.as_ref());
                    for (other_branch_commit_id, branch_head) in all_other_branch_commit_ids {
                        let branch_head = git2_to_gix_object_id(branch_head);
                        let base = repo
                            .merge_base_with_graph(
                                git2_to_gix_object_id(other_branch_commit_id),
                                branch_head,
                                &mut graph,
                            )
                            .ok()
                            .map(gix::Id::detach);
                        let res = match base {
                            Some(base) => {
                                let mut num_commits = 0;
                                let mut authors = HashSet::new();
                                for attempt in 1..=2 {
                                    let mut revwalk =
                                        repo.rev_walk(Some(branch_head)).with_boundary(Some(base));
                                    if attempt == 2 {
                                        revwalk = revwalk
                                            .sorting(gix::revision::walk::Sorting::BreadthFirst);
                                    }
                                    let revwalk = revwalk.all()?;
                                    for commit_info in revwalk {
                                        let commit_info = commit_info?;
                                        let commit = repo.find_commit(commit_info.id)?;
                                        authors.insert(commit.author()?.into());
                                        num_commits += 1;
                                    }
                                    if num_commits > 0 {
                                        break;
                                    }
                                }
                                merge_tx.send(Some((base, authors, num_commits)))
                            }
                            None => merge_tx.send(None),
                        };
                        if res.is_err() {
                            break;
                        }
                    }
                    Ok(())
                }
            })?;

        for branch in branches {
            let Some((base, authors, num_commits)) = merge_rx.recv()? else {
                continue;
            };

            let branch_head = git2_to_gix_object_id(branch.head);
            let base_commit = repo.find_object(base)?.try_into_commit()?;
            let base_tree = base_commit.tree()?;
            let head_tree = repo.find_object(branch_head)?.peel_to_tree()?;

            let ((change_tx, change_rx), (res_tx, rex_rx)) =
                (std::sync::mpsc::channel(), std::sync::mpsc::channel());
            if start.send((change_rx, res_tx)).is_err() {
                bail!("diffing-thread crashed");
            };
            base_tree
                .changes()?
                .options(|opts| {
                    opts.track_rewrites(None);
                })
                // NOTE: `stats(head_tree)` is also possible, but we have a separate thread for that.
                .for_each_to_obtain_tree(&head_tree, move |change| -> anyhow::Result<Action> {
                    change_tx.send(change.detach()).ok();
                    Ok(Action::Continue)
                })?;
            let (number_of_files, lines_added, lines_removed) = rex_rx.recv()?;

            let branch_data = BranchListingDetails {
                name: branch.name,
                lines_added,
                lines_removed,
                number_of_files,
                authors: authors.into_iter().collect(),
                number_of_commits: num_commits,
                stack: branch.stack,
            };
            enriched_branches.push(branch_data);
        }
        (diffstats, merge_bases)
    };
    diffstats.join().expect("no panic")?;
    merge_bases.join().expect("no panic")?;
    Ok(enriched_branches)
}

/// Represents a fat struct with all the data associated with a branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchListingDetails {
    /// The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name
    pub name: BranchIdentity,
    /// The number of lines added within the branch
    /// Since the virtual branch, local branch and the remote one can have different number of lines removed,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple).
    /// If this branch has a virtual branch, lines_added does NOT include the uncommitted lines.
    pub lines_added: usize,
    /// The number of lines removed within the branch
    /// Since the virtual branch, local branch and the remote one can have different number of lines removed,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    /// If this branch has a virtual branch, lines_removed does NOT include the uncommitted lines.
    pub lines_removed: usize,
    /// The number of files that were modified within the branch
    /// Since the virtual branch, local branch and the remote one can have different number files modified,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    pub number_of_files: usize,
    /// The number of commits associated with a branch
    /// Since the virtual branch, local branch and the remote one can have different number of commits,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    pub number_of_commits: usize,
    /// A list of authors that have contributes commits to this branch.
    /// In the case of multiple remote tracking branches, or branches whose commits are evaluated,
    /// it takes the full list of unique authors, without applying a mailmap.
    pub authors: Vec<Author>,
    /// The branch may or may not have a virtual branch associated with it.
    pub stack: Option<StackReference>,
}

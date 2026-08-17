//! A cheap, complete listing of all local and remote branches, grouped into stacks for display.
//!
//! The listing is produced from a single ref enumeration and a single graph traversal
//! that includes every relevant branch tip. All commit counts are answered from graph
//! memory; no tree or blob diffs are computed here. Expensive per-branch data belongs
//! to the selected-branch APIs, like `but_workspace::ui::diff::changes_in_branch()`.
#![deny(missing_docs)]

mod walk;

use std::collections::{BTreeMap, BTreeSet};

use bstr::{BStr, BString, ByteSlice};
use but_core::{RefMetadata, WORKSPACE_REF_NAME};
use but_graph::{Graph, SegmentIndex, Workspace};
use gix::{
    prelude::ObjectIdExt,
    refs::{Category, FullName},
};
use walk::OwnedHistory;

/// Options for [`list()`].
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// The project metadata carrying the target ref and last-seen target commit.
    pub project_meta: but_core::ref_metadata::ProjectMeta,
    /// Stop the traversal after roughly this many commits, for very large repositories.
    ///
    /// A hit limit sets [`BranchListing::incomplete`], and branches whose fork point
    /// lies beyond it report `None` counts instead of wrong ones.
    pub hard_limit: Option<usize>,
}

/// All branches of the repository, grouped into stacks for display.
#[derive(Debug, Clone)]
pub struct BranchListing {
    /// The stacks of branches, most recently updated first, tie-broken by the tip
    /// branch's ref name; stacks without any readable tip commit sort last.
    ///
    /// Presentation order beyond recency is the caller's: group by
    /// [`status`](ListedStack::status) to lead with workspace-related stacks.
    pub stacks: Vec<ListedStack>,
    /// The full name of the configured target branch, if set, e.g. `refs/remotes/origin/main`.
    pub target_ref: Option<FullName>,
    /// If `true`, the traversal hit its hard limit: at least one branch reports
    /// `None` counts, and stacks whose relation lies beyond the limit are not inferred.
    pub incomplete: bool,
}

/// How a stack of branches relates to the GitButler workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListedStackStatus {
    /// The branches are applied to the workspace as a stack.
    Applied,
    /// The branches form a stack known to workspace metadata, but it is not applied.
    Unapplied,
    /// The stack represents the configured target branch.
    Target,
    /// The branches are not related to the workspace.
    Standalone,
}

/// One or more stacked branches that should be displayed as a unit.
///
/// A branch belongs to exactly one stack, so the tip branch's
/// [`ref_name`](ListedBranch::ref_name) uniquely identifies the stack.
#[derive(Debug, Clone)]
pub struct ListedStack {
    /// How the stack relates to the workspace.
    pub status: ListedStackStatus,
    /// The branches of the stack, from the tip (newest) to the base (oldest).
    ///
    /// When flattening branches across stacks, carry [`status`](Self::status) along:
    /// facts like "this is the target branch" exist only at the stack level.
    pub branches: Vec<ListedBranch>,
    /// The committer timestamp of the newest branch tip in milliseconds since the epoch.
    pub updated_at_ms: Option<i64>,
}

/// A single branch, unifying its local ref and all remote-tracking refs of the same name.
#[derive(Debug, Clone)]
pub struct ListedBranch {
    /// The full name of the primary ref: the local branch if it exists,
    /// otherwise the first remote-tracking branch.
    pub ref_name: FullName,
    /// The branch name without ref prefix or remote name, for display and identity.
    pub display_name: BString,
    /// The commit at the tip of the primary ref.
    pub tip: gix::ObjectId,
    /// Whether a local branch of this name exists.
    ///
    /// Every listed branch is backed by at least one ref: when this is `false`,
    /// [`remote_refs`](Self::remote_refs) is never empty.
    pub has_local: bool,
    /// All remote-tracking refs that share the branch name, e.g. `refs/remotes/origin/foo`.
    pub remote_refs: Vec<FullName>,
    /// The number of commits this branch adds on its own: from its tip down to the
    /// next listed branch below it, the workspace, the target, or another branch
    /// fork. Contrast [`commits_ahead_of_target`](Self::commits_ahead_of_target),
    /// which for a stacked branch also counts the commits of the branches below.
    ///
    /// `None` when the commit graph ends before the fork point was found — because
    /// a traversal hard limit cut it short (see [`BranchListing::incomplete`]) or
    /// the repository is a shallow clone; an exact-looking number would be wrong.
    pub commit_count: Option<usize>,
    /// The number of commits reachable from the branch but not from the target branch.
    ///
    /// For a branch stacked on top of others this includes the commits of those
    /// lower branches, so it is the branch's distance from the target rather than
    /// its own contribution (see [`commit_count`](Self::commit_count)).
    ///
    /// `None` when no target is configured ([`BranchListing::target_ref`] is `None`),
    /// or when the connection to the target lies beyond the end of the traversed
    /// graph, due to a traversal hard limit or a shallow clone. With a target and a
    /// fully traversed graph this is always `Some`.
    pub commits_ahead_of_target: Option<usize>,
    /// The author of the tip commit, identifying who worked on the branch.
    ///
    /// Note that [`updated_at_ms`](Self::updated_at_ms) deliberately uses the
    /// *committer* time of the same commit: the author says who, the committer
    /// time says when the branch last changed.
    pub last_author: Option<gix::actor::Signature>,
    /// The committer timestamp of the tip commit in milliseconds since the epoch.
    pub updated_at_ms: Option<i64>,
}

/// List all local and remote branches of `repo`, grouped into stacks where known,
/// using `meta` for workspace metadata and `options` for target and traversal control.
///
/// Branches that are applied to the workspace are returned as [`ListedStackStatus::Applied`]
/// stacks rather than being dropped, so callers can decide how to present them.
/// A local branch and remote-tracking branches of the same name become one
/// [`ListedBranch`]. GitButler-internal refs are excluded.
pub fn list(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    db: &mut but_db::DbHandle,
    options: Options,
) -> anyhow::Result<BranchListing> {
    let remote_names = repo.remote_names();
    let refs_by_identity = enumerate_branch_refs(repo, &remote_names)?;

    let head = repo.head()?;
    let (head_id, head_ref) = match head.id() {
        Some(id) => (id, head.referent_name().map(|name| name.to_owned())),
        // An unborn HEAD cannot seed the traversal; start from any enumerated
        // branch tip instead so e.g. a freshly fetched repository still lists.
        // The tip stays unnamed on purpose: in a repository like that the first
        // enumerated ref is typically a remote-tracking one, which the traversal
        // refuses as a start position. The ref is seeded as an extra tip below.
        None => match refs_by_identity.values().flatten().next() {
            Some(r) => (r.tip.attach(repo), None),
            None => {
                return Ok(BranchListing {
                    stacks: Vec::new(),
                    target_ref: options.project_meta.target_ref,
                    incomplete: false,
                });
            }
        },
    };
    let applied_ref_names = applied_branch_ref_names(meta)?;
    let local_identities: BTreeSet<&BString> = refs_by_identity
        .values()
        .flatten()
        .filter(|r| r.remote.is_none())
        .map(|r| &r.identity)
        .collect();

    let extra_tips = refs_by_identity.values().flatten().filter_map(|r| {
        let is_covered_by_workspace = applied_ref_names.contains(&r.ref_name);
        // Remote-tracking refs of local branches are discovered while traversing the
        // local branch and must not be seeded again.
        let is_tracking_a_local = r.remote.is_some() && local_identities.contains(&r.identity);
        let is_target = Some(&r.ref_name) == options.project_meta.target_ref.as_ref();
        (!is_covered_by_workspace && !is_tracking_a_local && !is_target)
            .then(|| (r.tip, r.ref_name.clone()))
    });

    // Force the target to be traversed as integrated history even if no workspace
    // metadata brings it in, so commit counts stop at integrated commits.
    let target_tip_for_traversal = options
        .project_meta
        .target_ref
        .as_ref()
        .and_then(|target_ref| repo.try_find_reference(target_ref.as_ref()).ok().flatten())
        .and_then(|reference| reference.into_fully_peeled_id().ok())
        .map(|id| id.detach());
    // The extra target marks a commit whose history counts as integrated even
    // without workspace metadata, so commit counts stop at integrated commits;
    // the hard limit bounds the traversal for very large repositories.
    let (ws, incomplete) = walk::build_workspace(
        head_id,
        head_ref,
        extra_tips,
        meta,
        options.project_meta.clone(),
        db,
        but_graph::init::Options {
            extra_target_commit_id: target_tip_for_traversal,
            hard_limit: options.hard_limit,
            ..Default::default()
        },
    )?;

    let mut tips: BTreeSet<gix::ObjectId> =
        refs_by_identity.values().flatten().map(|r| r.tip).collect();
    tips.extend(
        ws.stacks
            .iter()
            .flat_map(|stack| stack.segments.iter())
            .filter_map(walk::segment_tip),
    );
    let tip_infos = tip_commit_infos(repo, tips);

    let ctx = ListingContext::new(
        &ws,
        remote_names,
        refs_by_identity,
        tip_infos,
        walk::target_of(&ws),
    );

    let mut consumed = BTreeSet::<BString>::new();
    let mut stacks = Vec::new();
    for stack in &ws.stacks {
        stacks.extend(ctx.applied_stack(stack, &mut consumed));
    }
    stacks.extend(ctx.unapplied_stacks(&mut consumed));
    stacks.extend(ctx.standalone_stacks(&consumed));
    stacks.sort_by(|a, b| {
        b.updated_at_ms.cmp(&a.updated_at_ms).then_with(|| {
            fn tip_ref(stack: &ListedStack) -> Option<&gix::refs::FullName> {
                stack.branches.first().map(|branch| &branch.ref_name)
            }
            tip_ref(a).cmp(&tip_ref(b))
        })
    });

    Ok(BranchListing {
        stacks,
        target_ref: ws
            .target_ref
            .as_ref()
            .map(|target| target.ref_name.clone())
            .or(options.project_meta.target_ref),
        incomplete,
    })
}

/// A local or remote branch ref resolved to its tip.
struct BranchRef {
    ref_name: FullName,
    /// The symbolic name of the remote this ref belongs to, if it is a remote-tracking ref,
    /// as extracted from the ref itself.
    remote: Option<String>,
    /// The branch name without ref prefix or remote name.
    identity: BString,
    tip: gix::ObjectId,
}

/// A branch while the listing is being assembled.
struct Participant {
    identity: BString,
    /// The local branch ref if one exists, otherwise the first remote-tracking ref.
    primary_ref: FullName,
    has_local: bool,
    remote_refs: Vec<FullName>,
    tip: gix::ObjectId,
}

/// Everything the stack builders read, gathered once per listing, including
/// precomputed graph indexes so per-branch queries stay cheap even with tens
/// of thousands of branches.
struct ListingContext<'a> {
    ws: &'a Workspace,
    remote_names: gix::remote::Names,
    /// All enumerated branch refs, grouped by branch name, in enumeration order.
    refs_by_identity: BTreeMap<BString, Vec<BranchRef>>,
    /// The author and committer time of every branch tip, read up front.
    tip_infos: BTreeMap<gix::ObjectId, (gix::actor::Signature, i64)>,
    /// The segment each ref name belongs to, along with the commit the ref points to:
    /// the segment's first commit, or for empty segments the peeled id recorded on the
    /// ref itself. Refs found on non-first commits map to the owning segment.
    segment_by_ref: BTreeMap<FullName, (SegmentIndex, Option<gix::ObjectId>)>,
    /// All segments whose commits the target branch has integrated.
    target_reachable: BTreeSet<SegmentIndex>,
    /// The commit at the tip of the target branch, if there is a target.
    target_tip: Option<gix::ObjectId>,
}

impl<'a> ListingContext<'a> {
    fn new(
        ws: &'a Workspace,
        remote_names: gix::remote::Names,
        refs_by_identity: BTreeMap<BString, Vec<BranchRef>>,
        tip_infos: BTreeMap<gix::ObjectId, (gix::actor::Signature, i64)>,
        target: Option<(gix::ObjectId, SegmentIndex)>,
    ) -> Self {
        ListingContext {
            segment_by_ref: walk::ref_index(&ws.graph),
            target_reachable: target
                .map(|(_, start)| walk::reachable_from(&ws.graph, start))
                .unwrap_or_default(),
            ws,
            remote_names,
            refs_by_identity,
            tip_infos,
            target_tip: target.map(|(tip, _)| tip),
        }
    }

    /// Count the commits reachable from `start` that the target has not integrated,
    /// or `None` if there is no target to compare against or a traversal limit
    /// makes the count unknowable.
    fn commits_ahead_of_target(&self, start: SegmentIndex) -> Option<usize> {
        self.target_tip?;
        walk::count_outside(self.graph(), &self.target_reachable, start)
    }

    fn owned_history(&self, start: SegmentIndex, identity: &BString) -> OwnedHistory {
        walk::owned_history(
            self.graph(),
            start,
            identity,
            &self.remote_names,
            self.target_tip,
        )
    }

    /// How `name` relates to the graph, given the `tip` it was enumerated at.
    fn anchor_of(&self, name: &FullName, tip: gix::ObjectId) -> Option<walk::Anchor> {
        let (segment, commit) = self.segment_by_ref.get(name).copied()?;
        Some(walk::Anchor::classify(segment, commit, tip))
    }

    /// List one applied workspace stack, marking its identities as consumed.
    fn applied_stack(
        &self,
        stack: &but_graph::workspace::Stack,
        consumed: &mut BTreeSet<BString>,
    ) -> Option<ListedStack> {
        let mut branches = Vec::new();
        for segment in &stack.segments {
            let Some(ref_name) = segment.ref_name().map(|name| name.to_owned()) else {
                continue;
            };
            if ref_name.category() == Some(Category::RemoteBranch) {
                // Remote-tracking segments are context of their local sibling,
                // which already lists the remote in its `remote_refs`.
                continue;
            }
            let Some(tip) = walk::segment_tip(segment) else {
                continue;
            };
            let display_name = display_identity(&ref_name, &self.remote_names);
            consumed.insert(display_name.clone());
            let (last_author, updated_at_ms) = self.tip_info(tip);
            // All enumerated remotes of the same name, like standalone branches
            // have, not just the segment's configured tracking ref.
            let remote_refs: Vec<FullName> = self
                .refs_of(&display_name)
                .filter(|r| r.remote.is_some())
                .map(|r| r.ref_name.clone())
                .collect();
            // The projection's commits look exact even when the bottom graph
            // segment was cut short, as in a shallow clone.
            let clipped = segment
                .commits_by_segment
                .last()
                .is_some_and(|&(sidx, _)| walk::traversal_was_clipped(self.graph(), sidx));
            branches.push(ListedBranch {
                display_name,
                has_local: ref_name.category() == Some(Category::LocalBranch),
                remote_refs,
                ref_name,
                tip,
                commit_count: (!clipped).then_some(segment.commits.len()),
                commits_ahead_of_target: self.commits_ahead_of_target(segment.id),
                last_author,
                updated_at_ms,
            });
        }
        if branches.is_empty() {
            return None;
        }
        Some(listed_stack(ListedStackStatus::Applied, branches))
    }

    /// List all unapplied metadata stacks, marking their identities as consumed.
    fn unapplied_stacks(&self, consumed: &mut BTreeSet<BString>) -> Vec<ListedStack> {
        let Some(metadata) = &self.ws.metadata else {
            return Vec::new();
        };
        let mut stacks = Vec::new();
        for stack in metadata
            .stacks
            .iter()
            .filter(|stack| !stack.is_in_workspace())
        {
            let mut members = Vec::new();
            for branch in stack.branches.iter().filter(|branch| !branch.archived) {
                let identity = branch.ref_name.shorten().to_owned();
                let Some(local) = self
                    .refs_of(&identity)
                    .find(|r| r.remote.is_none() && r.ref_name == branch.ref_name)
                else {
                    // The branch was deleted outside GitButler; its metadata is stale.
                    continue;
                };
                consumed.insert(identity.clone());
                members.extend(self.participant(identity, Some(local)));
            }
            if members.is_empty() {
                continue;
            }
            let branches = self.listed_branches(members);
            stacks.push(listed_stack(ListedStackStatus::Unapplied, branches));
        }
        stacks
    }

    /// Stack up all branches that neither the workspace nor its metadata accounts for.
    ///
    /// The target branch becomes its own [`ListedStackStatus::Target`] stack. Everything else is
    /// grouped by unambiguous first-parent chains of branch tips: if the history of branch
    /// `A` runs directly into branch `B` and no other listed branch does, they form one
    /// stack. Forks and shared tips stay separate.
    fn standalone_stacks(&self, consumed: &BTreeSet<BString>) -> Vec<ListedStack> {
        let mut by_identity = BTreeMap::<BString, Participant>::new();
        for identity in self.refs_by_identity.keys() {
            if consumed.contains(identity) {
                continue;
            }
            let local = self.refs_of(identity).find(|r| r.remote.is_none());
            if let Some(participant) = self.participant(identity.clone(), local) {
                by_identity.insert(identity.clone(), participant);
            }
        }

        let target_stack = self
            .ws
            .target_ref
            .as_ref()
            .map(|target| display_identity(&target.ref_name, &self.remote_names))
            .and_then(|identity| by_identity.remove(&identity));

        let mut stacks = Vec::new();
        if let Some(target) = target_stack {
            let branches = self.listed_branches(vec![target]);
            stacks.push(listed_stack(ListedStackStatus::Target, branches));
        }
        for chain in self.infer_chains(&by_identity) {
            let members: Vec<Participant> = chain
                .into_iter()
                .filter_map(|identity| by_identity.remove(&identity))
                .collect();
            if members.is_empty() {
                continue;
            }
            let branches = self.listed_branches(members);
            stacks.push(listed_stack(ListedStackStatus::Standalone, branches));
        }
        stacks
    }

    /// Order `participants` into stacks, single branches remaining their own stack.
    ///
    /// A chain edge `A -> B` exists if walking down from `A` along the first parent, the
    /// first named segment reached is `B`'s tip. Only when `B` is claimed by exactly one
    /// branch are the two stacked; a fork onto `B` keeps everything separate. The returned
    /// chains are ordered tip-first and identified by participant identity.
    fn infer_chains(&self, participants: &BTreeMap<BString, Participant>) -> Vec<Vec<BString>> {
        // Only own tips can anchor a walk; a ref that resolved to a commit inside
        // another segment has no exclusive history of its own.
        let anchors: Vec<(&BString, SegmentIndex)> = participants
            .values()
            .filter_map(|p| match self.anchor_of(&p.primary_ref, p.tip) {
                Some(walk::Anchor::OwnsTip(segment)) => Some((&p.identity, segment)),
                _ => None,
            })
            .collect();
        let identity_by_segment: BTreeMap<SegmentIndex, &BString> = anchors
            .iter()
            .map(|&(identity, segment)| (segment, identity))
            .collect();

        // `A -> B` when `B` is the first named segment below `A`.
        let mut below = BTreeMap::<&BString, &BString>::new();
        let mut claims = BTreeMap::<&BString, usize>::new();
        for &(identity, segment) in &anchors {
            let Some(child) = self
                .owned_history(segment, identity)
                .boundary
                .and_then(|boundary| identity_by_segment.get(&boundary).copied())
            else {
                continue;
            };
            below.insert(identity, child);
            *claims.entry(child).or_default() += 1;
        }
        below.retain(|_, child| claims.get(*child) == Some(&1));

        let children: BTreeSet<&BString> = below.values().copied().collect();
        participants
            .keys()
            .filter(|identity| !children.contains(identity))
            .map(|identity| {
                let mut chain = vec![identity.clone()];
                let mut cursor = identity;
                while let Some(child) = below.get(cursor) {
                    chain.push((*child).clone());
                    cursor = child;
                }
                chain
            })
            .collect()
    }

    /// Produce the listed branches of one stack from its members, tip-first, computing
    /// exclusive commit counts and target divergence from graph memory.
    fn listed_branches(&self, members: Vec<Participant>) -> Vec<ListedBranch> {
        members
            .into_iter()
            .map(|member| {
                let (commit_count, commits_ahead_of_target) =
                    match self.anchor_of(&member.primary_ref, member.tip) {
                        Some(anchor) => {
                            let commit_count = match anchor {
                                walk::Anchor::OwnsTip(segment) => {
                                    let history = self.owned_history(segment, &member.identity);
                                    (!history.clipped).then_some(history.commit_count)
                                }
                                walk::Anchor::MidHistory(_) => Some(0),
                                walk::Anchor::Unreached(_) => None,
                            };
                            let ahead = self.commits_ahead_of_target(anchor.segment());
                            (commit_count, ahead)
                        }
                        // The tip is not part of the graph at all; nothing exact
                        // can be said about it.
                        None => (None, None),
                    };
                let (last_author, updated_at_ms) = self.tip_info(member.tip);
                ListedBranch {
                    display_name: member.identity,
                    has_local: member.has_local,
                    remote_refs: member.remote_refs,
                    tip: member.tip,
                    commit_count,
                    commits_ahead_of_target,
                    last_author,
                    updated_at_ms,
                    ref_name: member.primary_ref,
                }
            })
            .collect()
    }

    /// All enumerated refs that share `identity`, in enumeration order.
    fn refs_of(&self, identity: &BString) -> impl Iterator<Item = &BranchRef> {
        self.refs_by_identity.get(identity).into_iter().flatten()
    }

    /// Assemble a participant for `identity`, collecting all remote refs of the same name.
    ///
    /// Without a `local` ref this is a remote-only branch whose tip comes from the first
    /// remote ref in enumeration order. Returns `None` if no ref of that identity exists.
    fn participant(&self, identity: BString, local: Option<&BranchRef>) -> Option<Participant> {
        let remotes: Vec<&BranchRef> = self
            .refs_of(&identity)
            .filter(|r| r.remote.is_some())
            .collect();
        let primary = local.or_else(|| remotes.first().copied())?;
        Some(Participant {
            identity,
            primary_ref: primary.ref_name.clone(),
            has_local: local.is_some(),
            remote_refs: remotes.iter().map(|r| r.ref_name.clone()).collect(),
            tip: primary.tip,
        })
    }

    /// The author and committer time of the tip commit, if it could be read.
    fn tip_info(&self, tip: gix::ObjectId) -> (Option<gix::actor::Signature>, Option<i64>) {
        match self.tip_infos.get(&tip) {
            Some((author, time_ms)) => (Some(author.clone()), Some(*time_ms)),
            None => (None, None),
        }
    }

    fn graph(&self) -> &Graph {
        &self.ws.graph
    }
}

/// Assemble a stack whose freshness is that of its newest member tip.
fn listed_stack(status: ListedStackStatus, branches: Vec<ListedBranch>) -> ListedStack {
    ListedStack {
        status,
        updated_at_ms: branches.iter().filter_map(|b| b.updated_at_ms).max(),
        branches,
    }
}

/// Return `true` if `identity` is a GitButler-internal branch name that must not
/// be shown or treated as a user branch.
pub fn is_technical_branch_name(identity: &BStr) -> bool {
    const TECHNICAL_IDENTITIES: &[&[u8]] = &[
        b"HEAD",
        b"gitbutler/edit",
        b"gitbutler/integration",
        b"gitbutler/oplog",
        b"gitbutler/target",
        b"gitbutler/workspace",
    ];
    debug_assert!(
        TECHNICAL_IDENTITIES.is_sorted(),
        "binary search requires sorted technical identities"
    );
    TECHNICAL_IDENTITIES
        .binary_search(&identity.as_bytes())
        .is_ok()
        || identity.starts_with(b"gitbutler/rename-backup/")
}

/// Enumerate all local and remote branch refs once, peeled to their tips and
/// grouped by branch name, preserving enumeration order within each name.
fn enumerate_branch_refs(
    repo: &gix::Repository,
    remote_names: &gix::remote::Names,
) -> anyhow::Result<BTreeMap<BString, Vec<BranchRef>>> {
    let platform = repo.references()?;
    let mut out = BTreeMap::<BString, Vec<BranchRef>>::new();
    for reference in platform.all()?.filter_map(Result::ok) {
        let ref_name = reference.name().to_owned();
        let (remote, identity) = match ref_name.category() {
            Some(Category::LocalBranch) => (None, ref_name.shorten().to_owned()),
            Some(Category::RemoteBranch) => {
                let Some((remote, identity)) =
                    but_core::extract_remote_name_and_short_name(ref_name.as_ref(), remote_names)
                else {
                    continue;
                };
                (Some(remote), identity)
            }
            _ => continue,
        };
        if is_technical_branch_name(identity.as_bstr()) {
            continue;
        }
        let Ok(tip) = reference.into_fully_peeled_id() else {
            // Broken or unborn refs can't participate in a commit graph.
            continue;
        };
        out.entry(identity.clone()).or_default().push(BranchRef {
            ref_name,
            remote,
            identity,
            tip: tip.detach(),
        });
    }
    Ok(out)
}

/// The refs of all branches that are applied to the workspace, according to metadata.
fn applied_branch_ref_names(meta: &impl RefMetadata) -> anyhow::Result<BTreeSet<FullName>> {
    let ws_ref: FullName = WORKSPACE_REF_NAME.try_into()?;
    let ws_md = meta.workspace(ws_ref.as_ref())?;
    Ok(ws_md
        .stacks
        .iter()
        .filter(|stack| stack.is_in_workspace())
        .flat_map(|stack| stack.branches.iter())
        .map(|branch| branch.ref_name.clone())
        .collect())
}

/// Read the author and the committer time of the branch tip; `None` if the commit
/// is unreadable. The author identifies who worked on the branch, while the
/// committer time reflects when it last changed.
fn tip_commit_info(
    repo: &gix::Repository,
    tip: gix::ObjectId,
) -> Option<(gix::actor::Signature, i64)> {
    let commit = repo.find_commit(tip).ok()?;
    let time_ms = commit.committer().ok()?.time().ok()?.seconds * 1000;
    Some((commit.author().ok()?.to_owned().ok()?, time_ms))
}

/// Read the author and committer time of every tip, in parallel when there are many.
fn tip_commit_infos(
    repo: &gix::Repository,
    tips: BTreeSet<gix::ObjectId>,
) -> BTreeMap<gix::ObjectId, (gix::actor::Signature, i64)> {
    if tips.len() < 200 {
        return tips
            .into_iter()
            .filter_map(|tip| tip_commit_info(repo, tip).map(|info| (tip, info)))
            .collect();
    }

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(8);
    let tips: Vec<_> = tips.into_iter().collect();
    let chunk_size = tips.len().div_ceil(threads);
    let mut infos = BTreeMap::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = tips
            .chunks(chunk_size)
            .map(|chunk| {
                let repo = repo.clone();
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|&tip| tip_commit_info(&repo, tip).map(|info| (tip, info)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            let chunk_infos = match handle.join() {
                Ok(infos) => infos,
                Err(panic) => std::panic::resume_unwind(panic),
            };
            infos.extend(chunk_infos);
        }
    });
    infos
}

/// The branch name used for identity and display: the short name without a remote prefix.
pub(crate) fn display_identity(ref_name: &FullName, remote_names: &gix::remote::Names) -> BString {
    but_core::extract_remote_name_and_short_name(ref_name.as_ref(), remote_names)
        .map(|(_, short)| short)
        .unwrap_or_else(|| ref_name.shorten().to_owned())
}

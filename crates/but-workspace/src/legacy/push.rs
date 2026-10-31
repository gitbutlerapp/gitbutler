//! This module makes an attempt to build a push implementation that is as
//! non-legacy as possible without doing some larger re-plumbing around the
//! gitbutler-git crate.
//!
//! This module should avoid legacy data structures where possible.

use std::str::FromStr as _;

use anyhow::{Context as _, Result};
use bstr::{BStr, BString, ByteSlice, ByteVec};
use but_core::extract_remote_name_and_short_name;
use but_db::DbHandle;
use gitbutler_git::{PushResult, push_with_askpass};
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::hooks;
use gix::{prelude::ObjectIdExt as _, refs::Category};
use indexmap::IndexMap;

use crate::{
    RefInfo, WorkspaceCommit,
    ref_info::{LocalCommitRelation, Segment},
    ui::PushStatus,
};

/// Push a given branch and its ancestors.
///
/// Refuses before pushing anything when that would publish GitButler's workspace commit.
#[allow(clippy::too_many_arguments)]
pub fn workspace_branch_and_ancestors_push(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    ref_info: &RefInfo,
    db: &mut DbHandle,
    gerrit_mode: bool,
    with_force: bool,
    skip_force_push_protection: bool,
    force_push_protection: bool,
    branch: &gix::refs::FullNameRef,
    run_hooks: bool,
    run_husky_hooks: bool,
    push_opts: Vec<but_gerrit::PushFlag>,
) -> Result<PushResult> {
    let graph = &ws.graph;
    if pushes_workspace_commit(repo, ref_info, branch) {
        anyhow::bail!(
            "{} is built on GitButler's workspace commit, which merges every applied branch, so \
             pushing it would publish all of them. Rebase its own commits onto the target branch \
             first.",
            branch.shorten()
        );
    }

    let remote_names = repo.remote_names();
    let target_branch_name = if gerrit_mode {
        let upstream = ws
            .graph
            .project_meta
            .remote_target_ref(repo)?
            .context("Gerrit push requires a target upstream")?;
        Some(target_branch_name_from_ref_name(
            upstream.as_ref(),
            &remote_names,
        )?)
    } else {
        None
    };
    let push_remote = ws
        .push_remote_name(repo)?
        .context("failed to get target push remote name")?;
    let mut result = PushResult {
        remote: push_remote.clone(),
        branch_to_remote: vec![],
        branch_sha_updates: vec![],
    };

    for (segment, ref_name) in branches_to_push(ref_info, branch).rev() {
        let Some(local_sha) = graph.tip_skip_empty(segment.id) else {
            continue;
        };

        let remote_refname = match &segment.remote_tracking_ref_name {
            Some(r) => r.clone(),
            None => format_remote_refname(ref_name, &push_remote)?,
        };
        let (remote_name, remote_branch_name) =
            extract_remote_name_and_short_name(remote_refname.as_ref(), &remote_names)
                .with_context(|| {
                    format!("failed to determine remote name for `{remote_refname}`")
                })?;
        let before_sha = remote_before_sha(repo, remote_refname.as_ref())?;
        let remote = repo.find_remote(remote_name.as_str())?;
        let remote_url = remote
            .url(gix::remote::Direction::Push)
            .or_else(|| remote.url(gix::remote::Direction::Fetch))
            .with_context(|| format!("Remote named {remote_name} didn't have a URL"))?;

        if run_hooks {
            match hooks::pre_push(
                repo,
                &remote_name,
                &remote_url.to_bstring().to_str_lossy(),
                local_sha.id,
                &RemoteRefname::from_str(&remote_refname.as_bstr().to_str_lossy())?,
                run_husky_hooks,
            )? {
                hooks::HookResult::Success | hooks::HookResult::NotConfigured => Ok(()),
                hooks::HookResult::Failure(error_data) => Err(anyhow::anyhow!(
                    "pre-push hook failed: {}",
                    error_data.error
                )),
            }?;
        }

        let gerrit_push_args = gerrit_push_args(
            gerrit_mode,
            local_sha.id,
            target_branch_name.as_ref().map(|name| name.as_bstr()),
            &push_opts,
        )?;
        let push_output = push_with_askpass(
            repo,
            local_sha.id,
            remote_refname.as_ref(),
            with_force,
            force_push_protection && !skip_force_push_protection,
            gerrit_push_args.refspec,
            // Historically we have tried to pass a stackId to the frontend when
            // doing askpass... but as far as I can tell, it's never used in a
            // meaningful way and doesn't seem to actually be required.
            Some(None),
            gerrit_push_args.push_opts,
        )?;

        maybe_record_gerrit_push_metadata(repo, db, gerrit_mode, segment, &push_output)?;
        if !gerrit_mode {
            track_pushed_branch(repo, ref_name, remote_refname.as_ref());
        }
        let branch_name = ref_name.shorten().to_str_lossy().to_string();
        result.branch_to_remote.push((
            branch_name.clone(),
            remote_refname,
            remote_branch_name.to_str_lossy().to_string(),
        ));
        result.branch_sha_updates.push((
            branch_name,
            before_sha.to_string(),
            local_sha.id.to_string(),
        ));
    }

    Ok(result)
}

/// Make `remote_refname`, just pushed, the upstream of `ref_name` where its config names none,
/// as `git push -u` does, so Git outside GitButler knows where the branch goes. The push
/// succeeded, so failing to record that only logs.
fn track_pushed_branch(
    repo: &gix::Repository,
    ref_name: &gix::refs::FullNameRef,
    remote_refname: &gix::refs::FullNameRef,
) {
    if repo
        .branch_remote_tracking_ref_name(ref_name, gix::remote::Direction::Fetch)
        .is_some()
    {
        return;
    }
    let tracked = crate::branch::setup_local_tracking_configuration(repo, ref_name, remote_refname)
        .and_then(|(config, _)| Ok(config.commit()?));
    if let Err(err) = tracked {
        tracing::warn!(
            ?err,
            "couldn't make {remote_refname} the upstream of {ref_name}"
        );
    }
}

/// Return the segment of the selected local branch and every segment beneath it in top-to-base
/// order, crossing into the lane a worktree rests on.
///
/// This is the logical scope of a push operation: every commit the push transfers sits in one of
/// these segments, whether or not a branch names it. [`branches_to_push()`] picks the ones pushed
/// as refs.
pub fn branch_and_ancestor_segments<'a>(
    ref_info: &'a RefInfo,
    branch: &gix::refs::FullNameRef,
) -> IndexMap<but_graph::SegmentIndex, &'a Segment> {
    ref_info
        .lane_chain(branch)
        .into_iter()
        .flat_map(|(lane, index)| lane.segments_from(index))
        .map(|segment| (segment.id, segment))
        .collect()
}

/// The segments of [`branch_and_ancestor_segments()`] that pushing `branch` pushes, in top-to-base
/// order, each with the local branch naming it: those not integrated and not already current on
/// the remote.
///
/// [`workspace_branch_and_ancestors_push()`] pushes exactly these, so a preview of the push must
/// list them too.
pub fn branches_to_push<'a>(
    ref_info: &'a RefInfo,
    branch: &gix::refs::FullNameRef,
) -> impl DoubleEndedIterator<Item = (&'a Segment, &'a gix::refs::FullNameRef)> + use<'a> {
    branch_and_ancestor_segments(ref_info, branch)
        .into_values()
        .filter(|segment| {
            !matches!(
                segment.push_status,
                PushStatus::Integrated | PushStatus::NothingToPush
            )
        })
        .filter_map(|segment| {
            let ref_name = segment
                .ref_name()
                .filter(|name| name.category() == Some(Category::LocalBranch))?;
            Some((segment, ref_name))
        })
}

/// Whether pushing `branch` sends GitButler's workspace commit, which merges every applied
/// branch. A branch made with `git checkout -b` on the workspace holds it, and a worktree made
/// from the main checkout's `HEAD`, or one stacked on such a worktree, rests on it. A workspace
/// commit the remote already has publishes nothing new.
fn pushes_workspace_commit(
    repo: &gix::Repository,
    ref_info: &RefInfo,
    branch: &gix::refs::FullNameRef,
) -> bool {
    let managed = |id: gix::ObjectId| {
        WorkspaceCommit::from_id(id.attach(repo)).is_ok_and(|commit| commit.is_managed())
    };
    let chain = ref_info.lane_chain(branch);
    let mut unpushed = chain
        .iter()
        .flat_map(|(lane, index)| lane.segments_from(*index))
        .flat_map(|segment| &segment.commits)
        .filter(|c| !matches!(c.relation, LocalCommitRelation::LocalAndRemote(remote) if remote == c.id));
    unpushed.any(|c| managed(c.id))
        || chain
            .iter()
            .any(|(lane, _)| lane.rests_on.is_some_and(managed))
}

struct GerritPushArgs {
    refspec: Option<String>,
    push_opts: Vec<String>,
}

fn gerrit_push_args(
    gerrit_mode: bool,
    head: gix::ObjectId,
    target_branch_name: Option<&BStr>,
    push_flags: &[but_gerrit::PushFlag],
) -> Result<GerritPushArgs> {
    Ok(if gerrit_mode {
        let target_branch_name =
            target_branch_name.context("Gerrit push requires a target upstream")?;
        GerritPushArgs {
            refspec: Some(format!("{head}:refs/for/{target_branch_name}")),
            push_opts: push_flags.iter().map(|flag| flag.to_string()).collect(),
        }
    } else {
        GerritPushArgs {
            refspec: None,
            push_opts: vec![],
        }
    })
}

/// Derive the target branch name while tolerating stale target remotes.
fn target_branch_name_from_ref_name(
    target_ref_name: &gix::refs::FullNameRef,
    remote_names: &gix::remote::Names,
) -> Result<BString> {
    let (category, shorthand_name) = target_ref_name
        .category_and_short_name()
        .context("Target branch could not be categorized")?;
    if matches!(category, Category::RemoteBranch) {
        if let Some((_remote, short_name)) =
            extract_remote_name_and_short_name(target_ref_name, remote_names)
        {
            return Ok(short_name);
        }
        let remote_ref: RemoteRefname = target_ref_name.to_string().parse()?;
        return Ok(remote_ref.branch().into());
    }
    Ok(shorthand_name.to_owned())
}

fn maybe_record_gerrit_push_metadata(
    repo: &gix::Repository,
    db: &mut DbHandle,
    gerrit_mode: bool,
    segment: &Segment,
    push_output: &str,
) -> Result<()> {
    if !gerrit_mode {
        return Ok(());
    }

    let push_output = but_gerrit::parse::push_output(push_output)?;
    but_gerrit::record_push_metadata(
        repo,
        db,
        segment.commits.iter().map(|c| c.id).collect::<Vec<_>>(),
        push_output,
    )
}

fn format_remote_refname(
    reference: &gix::refs::FullNameRef,
    remote_name: &str,
) -> Result<gix::refs::FullName> {
    let mut out: BString = b"refs/remotes/".into();
    out.push_str(format!("{remote_name}/"));
    out.push_str(reference.shorten());

    Ok(out.try_into()?)
}

fn remote_before_sha(
    repo: &gix::Repository,
    remote_refname: &gix::refs::FullNameRef,
) -> Result<gix::ObjectId> {
    Ok(repo
        .try_find_reference(remote_refname)?
        .map(|mut reference| reference.peel_to_commit())
        .transpose()?
        .map(|commit| commit.id)
        .unwrap_or(repo.object_hash().null()))
}

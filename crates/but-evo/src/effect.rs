use anyhow::Result;
use bitflags::bitflags;
use but_core::{
    ChangeState, Commit, TreeChange, UnifiedPatch,
    changeset::{ChangesetCommit as _, id_for_tree_diff},
    diff::commit_changes,
};
use gix::bstr::{BString, ByteSlice as _};

bitflags! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub(crate) struct EffectFlags: u32 {
        const DESCRIPTION = 1;
        const EXTRA_METADATA = 2;
        const PARENTS = 4;
        const DIFF = 8;
        const USER = 16;
        const DATE = 32;
    }
}

const UNTRACKED_HEADERS: [&str; 3] = ["gpgsig", "gpgsig-sha256", "gitbutler-headers-version"];

pub(crate) fn effect_flags(prec: &Commit<'_>, succs: &[Commit<'_>]) -> Result<EffectFlags> {
    let mut flags = EffectFlags::empty();
    for succ in succs {
        flags |= metadata_flags(prec, succ);
        if !introduces_same_change(prec, succ)? {
            flags |= EffectFlags::DIFF;
        }
    }
    Ok(flags)
}

fn metadata_flags(prec: &Commit<'_>, succ: &Commit<'_>) -> EffectFlags {
    let (lhs, rhs) = (&prec.author, &succ.author);
    let mut flags = EffectFlags::empty();
    flags.set(EffectFlags::DESCRIPTION, prec.message() != succ.message());
    flags.set(
        EffectFlags::EXTRA_METADATA,
        tracked_headers(prec) != tracked_headers(succ),
    );
    flags.set(EffectFlags::PARENTS, prec.parents != succ.parents);
    flags.set(
        EffectFlags::USER,
        lhs.name != rhs.name || lhs.email != rhs.email,
    );
    flags.set(EffectFlags::DATE, lhs.time != rhs.time);
    flags
}

fn tracked_headers<'a>(commit: &'a Commit<'_>) -> Vec<&'a (BString, BString)> {
    let mut headers: Vec<_> = commit
        .extra_headers
        .iter()
        .filter(|(name, _)| !UNTRACKED_HEADERS.iter().any(|ignored| name == ignored))
        .collect();
    headers.sort();
    headers
}

fn introduces_same_change(prec: &Commit<'_>, succ: &Commit<'_>) -> Result<bool> {
    let repo = prec.id.repo;
    let parent_tree = |commit: &Commit<'_>| -> Result<Option<gix::ObjectId>> {
        commit
            .first_parent_id()
            .map(|parent| Ok(repo.find_commit(parent)?.tree_id()?.detach()))
            .transpose()
    };
    if prec.tree == succ.tree && parent_tree(prec)? == parent_tree(succ)? {
        return Ok(true);
    }
    let changeset =
        |commit: &Commit<'_>| id_for_tree_diff(repo, commit.first_parent_id(), commit.id.detach());
    if changeset(prec)? == changeset(succ)? {
        return Ok(true);
    }
    let prec_changes = commit_changes(prec.id)?.into_tree_changes();
    let succ_changes = commit_changes(succ.id)?.into_tree_changes();
    if prec_changes.len() != succ_changes.len() {
        return Ok(false);
    }
    for (lhs, rhs) in prec_changes.iter().zip(&succ_changes) {
        if !same_file_change(repo, lhs, rhs)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn same_file_change(repo: &gix::Repository, lhs: &TreeChange, rhs: &TreeChange) -> Result<bool> {
    let (lhs_old, lhs_new) = states(lhs);
    let (rhs_old, rhs_new) = states(rhs);
    let kind = |state: Option<ChangeState>| state.map(|state| state.kind);
    let id = |state: Option<ChangeState>| state.map(|state| state.id);
    if lhs.path != rhs.path
        || lhs.previous_path() != rhs.previous_path()
        || kind(lhs_old) != kind(rhs_old)
        || kind(lhs_new) != kind(rhs_new)
    {
        return Ok(false);
    }
    if id(lhs_old) == id(rhs_old) && id(lhs_new) == id(rhs_new) {
        return Ok(true);
    }
    match (patch_lines(repo, lhs)?, patch_lines(repo, rhs)?) {
        (Some(lhs), Some(rhs)) => Ok(lhs == rhs),
        _ => Ok(false),
    }
}

fn states(change: &TreeChange) -> (Option<ChangeState>, Option<ChangeState>) {
    let previous = change.status.previous_state_and_path();
    (previous.map(|(state, _)| state), change.status.state())
}

fn patch_lines(repo: &gix::Repository, change: &TreeChange) -> Result<Option<Vec<BString>>> {
    let Some(UnifiedPatch::Patch { hunks, .. }) = change.unified_patch(repo, 1)? else {
        return Ok(None);
    };
    let without_header = |diff: &BString| {
        diff.find_byte(b'\n')
            .map_or_else(BString::default, |newline| diff[newline..].into())
    };
    Ok(Some(
        hunks
            .iter()
            .map(|hunk| without_header(&hunk.diff))
            .collect(),
    ))
}

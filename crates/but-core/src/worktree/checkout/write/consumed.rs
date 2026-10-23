//! Moving the index forward by the changes a commit consumed from the worktree, see
//! [`carry_into_index()`].

use bstr::{BStr, BString};
use gix::{index::entry::Mode, objs::Write as _};

use super::plan::Content;
use crate::{
    RepositoryExt as _,
    index::{Edit, Stat},
};

/// Move `index` forward by the changes from `head_tree` to `override_tree`, which a commit or an
/// amend consumed from the worktree, and return `true` if it changed.
///
/// A path with `head_tree`'s entry takes `override_tree`'s, and one with `override_tree`'s keeps
/// it. Other staged content is merged with the change if both are files and the merge is clean,
/// and otherwise replaced by `override_tree`'s entry. Entries at other paths keep their stats.
pub(in crate::worktree::checkout) fn carry_into_index(
    repo: &gix::Repository,
    index: &mut gix::index::State,
    [head_tree, override_tree]: [gix::ObjectId; 2],
) -> anyhow::Result<bool> {
    let mut edit = Edit::new(index);
    for (path, sides) in changes(repo, [head_tree, override_tree])? {
        let path = path.as_ref();
        match carried(repo, path, staged(edit.state(), path), sides)? {
            Carry::Keep => {}
            Carry::Set(Some(Content { id, mode })) => edit.upsert(path, id, mode, Stat::Unknown),
            Carry::Set(None) => edit.remove(path),
        }
    }
    Ok(edit.finish())
}

/// What the index holds at a path.
enum Staged {
    Entry(Option<Content>),
    Conflicted,
}

/// What the index should hold at a path.
enum Carry {
    /// What it holds already.
    Keep,
    Set(Option<Content>),
}

/// The files, symlinks and submodules that differ from `old` to `new`, with what each tree holds.
fn changes(
    repo: &gix::Repository,
    [old, new]: [gix::ObjectId; 2],
) -> anyhow::Result<Vec<(BString, [Option<Content>; 2])>> {
    use gix::diff::tree::recorder::Change;
    let (old, new) = (repo.find_tree(old)?, repo.find_tree(new)?);
    let mut recorder = gix::diff::tree::Recorder::default();
    gix::diff::tree(
        gix::objs::TreeRefIter::from_bytes(&old.data, repo.object_hash()),
        gix::objs::TreeRefIter::from_bytes(&new.data, repo.object_hash()),
        gix::diff::tree::State::default(),
        &repo.objects,
        &mut recorder,
    )?;
    let side = |mode: gix::objs::tree::EntryMode, id| {
        (!mode.is_tree()).then(|| Content {
            id,
            mode: mode.into(),
        })
    };
    Ok(recorder
        .records
        .into_iter()
        .map(|change| match change {
            Change::Addition {
                entry_mode,
                oid,
                path,
                ..
            } => (path, [None, side(entry_mode, oid)]),
            Change::Deletion {
                entry_mode,
                oid,
                path,
                ..
            } => (path, [side(entry_mode, oid), None]),
            Change::Modification {
                previous_entry_mode,
                previous_oid,
                entry_mode,
                oid,
                path,
            } => (
                path,
                [
                    side(previous_entry_mode, previous_oid),
                    side(entry_mode, oid),
                ],
            ),
        })
        .filter(|(_, [old, new])| old.is_some() || new.is_some())
        .collect())
}

fn staged(index: &gix::index::State, path: &BStr) -> Staged {
    let entries = index
        .entry_range(path)
        .and_then(|range| index.entries().get(range))
        .unwrap_or_default();
    match entries {
        [] => Staged::Entry(None),
        [entry] if entry.stage() == gix::index::entry::Stage::Unconflicted => {
            Staged::Entry(Some(Content::of(entry)))
        }
        _ => Staged::Conflicted,
    }
}

/// What the index should hold at `path`, which holds `staged`, given what the head tree and the
/// override tree hold there. An entry that holds it already keeps its stat.
fn carried(
    repo: &gix::Repository,
    path: &BStr,
    staged: Staged,
    [head, overridden]: [Option<Content>; 2],
) -> anyhow::Result<Carry> {
    let Staged::Entry(entry) = staged else {
        return Ok(Carry::Set(overridden));
    };
    let merged = match (head, entry, overridden) {
        (Some(base), Some(ours), Some(theirs)) => merge_files(repo, path, [base, ours, theirs])?,
        _ => None,
    };
    let carried = merged.or(overridden);
    Ok(match carried == entry {
        true => Carry::Keep,
        false => Carry::Set(carried),
    })
}

/// The staged `ours` with the change from `base` to `theirs`, if all are files and they merge
/// cleanly.
fn merge_files(
    repo: &gix::Repository,
    path: &BStr,
    [base, ours, theirs]: [Content; 3],
) -> anyhow::Result<Option<Content>> {
    let is_file = |content: &Content| matches!(content.mode, Mode::FILE | Mode::FILE_EXECUTABLE);
    if ![base, ours, theirs].iter().all(is_file) {
        return Ok(None);
    }
    let Some(mode) = merged([base.mode, ours.mode, theirs.mode]) else {
        return Ok(None);
    };
    let id = match merged([base.id, ours.id, theirs.id]) {
        Some(id) => id,
        None => match merge_blobs(repo, path, [base, ours, theirs])? {
            Some(id) => id,
            None => return Ok(None),
        },
    };
    Ok(Some(Content { id, mode }))
}

/// The side that changed from `base`, if only one did or both changed alike.
fn merged<T: PartialEq>([base, ours, theirs]: [T; 3]) -> Option<T> {
    if ours == base {
        Some(theirs)
    } else if theirs == base || theirs == ours {
        Some(ours)
    } else {
        None
    }
}

/// The clean merge of the blobs `ours` and `theirs` at `path` from `base`, written to the object
/// database on disk, as the index needs it there.
fn merge_blobs(
    repo: &gix::Repository,
    path: &BStr,
    sides: [Content; 3],
) -> anyhow::Result<Option<gix::ObjectId>> {
    use gix::merge::blob::{Resolution, ResourceKind};
    let mut platform = repo.merge_resource_cache(Default::default())?;
    let kinds = [
        ResourceKind::CommonAncestorOrBase,
        ResourceKind::CurrentOrOurs,
        ResourceKind::OtherOrTheirs,
    ];
    for (Content { id, mode }, kind) in sides.into_iter().zip(kinds) {
        let entry_kind = mode.to_tree_entry_mode().expect("a file").kind();
        platform.set_resource(id, entry_kind, path, kind, &repo.objects)?;
    }
    let prepared = platform.prepare_merge(&repo.objects, repo.blob_merge_options()?)?;
    let mut buf = Vec::new();
    let (pick, resolution) = prepared.merge(
        &mut buf,
        repo.default_merge_labels(),
        &repo.command_context()?,
    )?;
    if resolution != Resolution::Complete {
        return Ok(None);
    }
    let mut on_disk = repo.objects.clone();
    on_disk.take_object_memory();
    Ok(prepared.id_by_pick(pick, &buf, |data| {
        on_disk.write_buf(gix::object::Kind::Blob, data)
    })?)
}

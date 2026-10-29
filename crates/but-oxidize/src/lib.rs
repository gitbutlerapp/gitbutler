//! A crate with various utilities to make the migration to `gitoxide` less cumbersome and repetitive.

use std::path::Path;

/// Open the repository at `git_dir` with `git2` so that it finds every tree `gix` refers to.
///
/// `gix` never stores the empty tree, and `libgit2` only knows the SHA-1 one without it being stored.
pub fn open_git2_repo(git_dir: &Path) -> anyhow::Result<git2::Repository> {
    let repo = git2::Repository::open(git_dir)?;
    if repo.object_format() != git2::ObjectFormat::Sha1 {
        store_empty_tree(&repo)?;
    }
    Ok(repo)
}

fn store_empty_tree(repo: &git2::Repository) -> Result<git2::Oid, git2::Error> {
    repo.treebuilder(None)?.write()
}

fn git2_to_gix_object_id(id: git2::Oid) -> gix::ObjectId {
    gix::ObjectId::try_from(id.as_bytes()).expect("git2 oid is always valid")
}

pub trait OidExt {
    fn to_gix(self) -> gix::ObjectId;
}

impl OidExt for git2::Oid {
    fn to_gix(self) -> gix::ObjectId {
        git2_to_gix_object_id(self)
    }
}

fn gix_to_git2_oid(id: impl Into<gix::ObjectId>) -> git2::Oid {
    git2::Oid::from_bytes(id.into().as_bytes()).expect("always valid")
}

pub trait ObjectIdExt {
    fn to_git2(self) -> git2::Oid;
}

impl ObjectIdExt for gix::ObjectId {
    fn to_git2(self) -> git2::Oid {
        gix_to_git2_oid(self)
    }
}

impl ObjectIdExt for gix::Id<'_> {
    fn to_git2(self) -> git2::Oid {
        gix_to_git2_oid(self.detach())
    }
}

/// Convert a `gix` index into a `git2` one, while skipping over entries that are marked for removal.
///
/// Note that this is quite inefficient as it will have to re-allocate all paths.
///
/// ## Note
///
/// * Flags aren't fully supported right now, they are truncated, but good enough to get the *stage* right.
pub fn gix_to_git2_index(index: &gix::index::State) -> anyhow::Result<git2::Index> {
    let mut out = git2::Index::new()?;
    for entry @ gix::index::Entry {
        stat:
            gix::index::entry::Stat {
                mtime,
                ctime,
                dev,
                ino,
                uid,
                gid,
                size,
            },
        id,
        flags,
        mode,
        ..
    } in index.entries()
    {
        if flags.contains(gix::index::entry::Flags::REMOVE) {
            continue;
        }
        let git2_entry = git2::IndexEntry {
            ctime: git2::IndexTime::new(ctime.secs as i32, ctime.nsecs),
            mtime: git2::IndexTime::new(mtime.secs as i32, mtime.nsecs),
            dev: *dev,
            ino: *ino,
            mode: mode.bits(),
            uid: *uid,
            gid: *gid,
            file_size: *size,
            id: gix_to_git2_oid(*id),
            flags: flags.bits() as u16,
            flags_extended: 0,
            path: entry.path(index).to_owned().into(),
        };
        out.add(&git2_entry)?
    }
    Ok(out)
}

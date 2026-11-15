//! A crate with various utilities to make the migration to `gitoxide` less cumbersome and repetitive.

use std::path::Path;

/// Open the repository at `git_dir` with `git2` so that it finds every tree `gix` refers to.
///
/// `gix` never stores the empty tree, and `libgit2` only knows the SHA-1 one without it being stored.
/// A repository another user owns fails with [`but_error::Code::RepoOwnership`], as only the user
/// can mark it safe.
pub fn open_git2_repo(git_dir: &Path) -> anyhow::Result<git2::Repository> {
    let repo = git2::Repository::open(git_dir).map_err(|err| match err.code() {
        git2::ErrorCode::Owner => {
            // `safe.directory` names the worktree, a linked one too, which `gix` still finds.
            let repo = gix::open(git_dir).ok();
            let path = (repo.as_ref()).and_then(|repo| repo.workdir()).unwrap_or(git_dir);
            anyhow::anyhow!(
                "The git directory is considered unsafe as it's not owned by the current user. Use `git config --global --add safe.directory '{}'` to allow it",
                path.display()
            )
            .context(but_error::Code::RepoOwnership)
        }
        _ => err.into(),
    })?;
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

pub trait TimeExt {
    fn to_gix(self) -> gix::date::Time;
}

impl TimeExt for git2::Time {
    fn to_gix(self) -> gix::date::Time {
        gix::date::Time::new(self.seconds(), self.offset_minutes() * 60)
    }
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

pub trait RepoExt {
    /// Reopen a `git2` repository as a `gix` repository with the normal Git configuration chain.
    ///
    /// Use this for helpers that inspect worktree state or run filters and therefore must see
    /// global ignores, `core.autocrlf`, and globally configured filters like Git LFS.
    fn to_gix_repo(&self) -> anyhow::Result<gix::Repository>;

    /// Reopen a `git2` repository as an isolated `gix` repository for helpers that only need
    /// object access and repository-local configuration.
    fn to_isolated_gix_repo(&self) -> anyhow::Result<gix::Repository>;
}

impl RepoExt for &git2::Repository {
    fn to_gix_repo(&self) -> anyhow::Result<gix::Repository> {
        let repo = gix::open(self.path())?;
        Ok(repo)
    }

    fn to_isolated_gix_repo(&self) -> anyhow::Result<gix::Repository> {
        let repo = gix::open_opts(self.path(), gix::open::Options::isolated())?;
        Ok(repo)
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

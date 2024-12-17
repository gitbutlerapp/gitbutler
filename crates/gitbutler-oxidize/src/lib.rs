//! A crate with various utilities to make the migration to `gitoxide` less cumbersome and repetitive.

use anyhow::Context;
use gix::bstr::ByteSlice;
use std::borrow::Borrow;

mod ext;
pub use ext::GixRepositoryExt;

pub fn gix_time_to_git2(time: gix::date::Time) -> git2::Time {
    git2::Time::new(time.seconds, time.offset)
}

pub fn git2_to_gix_object_id(id: git2::Oid) -> gix::ObjectId {
    gix::ObjectId::try_from(id.as_bytes()).expect("git2 oid is always valid")
}

pub fn gix_to_git2_oid(id: impl Into<gix::ObjectId>) -> git2::Oid {
    git2::Oid::from_bytes(id.into().as_bytes()).expect("always valid")
}

pub fn git2_signature_to_gix_signature<'a>(
    input: impl Borrow<git2::Signature<'a>>,
) -> gix::actor::Signature {
    let input = input.borrow();
    gix::actor::Signature {
        name: input.name_bytes().into(),
        email: input.email_bytes().into(),
        time: gix::date::Time {
            seconds: input.when().seconds(),
            offset: input.when().offset_minutes() * 60,
            sign: input.when().offset_minutes().into(),
        },
    }
}

/// Convert `actor` to a `git2` representation or fail if that's not possible.
/// Note that the current time as provided by `gix` is also used as it.
pub fn gix_to_git2_signature(
    actor: gix::actor::SignatureRef<'_>,
) -> anyhow::Result<git2::Signature<'static>> {
    let offset_in_minutes = actor.time.offset / 60;
    let time = git2::Time::new(actor.time.seconds, offset_in_minutes);
    Ok(git2::Signature::new(
        actor
            .name
            .to_str()
            .with_context(|| format!("Could not process actor name: {}", actor.name))?,
        actor
            .email
            .to_str()
            .with_context(|| format!("Could not process actor email: {}", actor.email))?,
        &time,
    )?)
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

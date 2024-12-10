use bstr::BStr;

use crate::commit_headers::HasCommitHeaders;

/// Extension trait for `git2::Commit`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Commit`
pub trait CommitExt {
    /// Obtain the commit-message as bytes, but without assuming any encoding.
    fn message_bstr(&self) -> &BStr;
    fn change_id(&self) -> Option<String>;
    fn is_signed(&self) -> bool;
    fn is_conflicted(&self) -> bool;
}

impl CommitExt for git2::Commit<'_> {
    fn message_bstr(&self) -> &BStr {
        self.message_bytes().as_ref()
    }

    fn change_id(&self) -> Option<String> {
        self.gitbutler_headers().map(|headers| headers.change_id)
    }
    fn is_signed(&self) -> bool {
        self.header_field_bytes("gpgsig").is_ok()
    }

    fn is_conflicted(&self) -> bool {
        self.gitbutler_headers()
            .and_then(|headers| headers.conflicted.map(|conflicted| conflicted > 0))
            .unwrap_or(false)
    }
}

impl CommitExt for gix::Commit<'_> {
    fn message_bstr(&self) -> &BStr {
        self.message_raw()
            .expect("valid commit that can be parsed: TODO - allow it to return errors?")
    }

    fn change_id(&self) -> Option<String> {
        self.gitbutler_headers().map(|headers| headers.change_id)
    }

    fn is_signed(&self) -> bool {
        self.decode().map_or(false, |decoded| {
            decoded.extra_headers().pgp_signature().is_some()
        })
    }

    fn is_conflicted(&self) -> bool {
        self.gitbutler_headers()
            .and_then(|headers| headers.conflicted.map(|conflicted| conflicted > 0))
            .unwrap_or(false)
    }
}

fn contains<'a, I>(iter: I, item: &git2::Commit<'a>) -> bool
where
    I: IntoIterator<Item = git2::Commit<'a>>,
{
    iter.into_iter().any(|iter_item| {
        // Return true if the commits match by commit id, or alternatively if both have a change id and they match.
        if iter_item.id() == item.id() {
            return true;
        }
        matches!((iter_item.change_id(), item.change_id()), (Some(iter_item_id), Some(iter_id)) if iter_item_id == iter_id)
    })
}

pub trait CommitVecExt {
    /// Returns `true` if the provided commit is part of the commits in this series.
    /// Compares the commits by commit id first and also by change ID
    fn contains_by_commit_or_change_id(&self, commit: &git2::Commit) -> bool;
}

impl CommitVecExt for Vec<git2::Commit<'_>> {
    fn contains_by_commit_or_change_id(&self, commit: &git2::Commit) -> bool {
        contains(self.iter().cloned(), commit)
    }
}

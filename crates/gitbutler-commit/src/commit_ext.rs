use bstr::BStr;
use but_core::{ChangeId, commit::Headers};

/// Extension trait for `gix::Commit`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Commit`
pub trait CommitExt {
    fn change_id(&self) -> Option<ChangeId>;
    fn is_conflicted(&self) -> bool;
}

pub trait CommitMessageBstr {
    /// Obtain the commit-message as bytes, but without assuming any encoding.
    fn message_bstr(&self) -> &BStr;
}

impl CommitExt for gix::Commit<'_> {
    fn change_id(&self) -> Option<ChangeId> {
        let commit = self.decode().ok()?;
        Headers::try_from_commit_headers(|| commit.extra_headers())?.change_id
    }

    fn is_conflicted(&self) -> bool {
        but_core::Commit::try_from(self.clone()).is_ok_and(|commit| commit.is_conflicted())
    }
}

impl CommitMessageBstr for gix::Commit<'_> {
    fn message_bstr(&self) -> &BStr {
        self.message_raw()
            .expect("valid commit that can be parsed: TODO - allow it to return errors?")
    }
}

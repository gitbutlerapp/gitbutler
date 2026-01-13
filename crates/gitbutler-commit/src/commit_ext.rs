use bstr::{BStr, ByteSlice};
use but_core::{ChangeId, commit::Headers};

/// Extension trait for `git2::Commit`.
///
/// For now, it collects useful methods from `gitbutler-core::git::Commit`
pub trait CommitExt {
    fn change_id(&self) -> Option<ChangeId>;
    fn is_signed(&self) -> bool;
    fn is_conflicted(&self) -> bool;
}

pub trait CommitMessageBstr {
    /// Obtain the commit-message as bytes, but without assuming any encoding.
    fn message_bstr(&self) -> &BStr;
}

impl CommitExt for gix::Commit<'_> {
    fn change_id(&self) -> Option<ChangeId> {
        let commit = self.decode().ok()?;
        let commit = commit.to_owned().ok()?;
        Headers::try_from_commit(&commit)?.change_id
    }

    fn is_signed(&self) -> bool {
        self.decode()
            .is_ok_and(|decoded| decoded.extra_headers().pgp_signature().is_some())
    }

    fn is_conflicted(&self) -> bool {
        self.decode()
            .ok()
            .and_then(|commit| {
                let commit = commit.to_owned().ok()?;
                let headers = Headers::try_from_commit(&commit)?;
                Some(headers.conflicted? > 0)
            })
            .unwrap_or(false)
    }
}

impl CommitMessageBstr for gix::Commit<'_> {
    fn message_bstr(&self) -> &BStr {
        self.message_raw()
            .expect("valid commit that can be parsed: TODO - allow it to return errors?")
    }
}

impl CommitMessageBstr for git2::Commit<'_> {
    fn message_bstr(&self) -> &BStr {
        self.message_bytes().as_bstr()
    }
}

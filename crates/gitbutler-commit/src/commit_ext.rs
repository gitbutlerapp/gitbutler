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

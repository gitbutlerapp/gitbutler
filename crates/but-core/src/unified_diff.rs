use super::UnifiedDiff;
use bstr::BString;

/// A hunk as used in a [UnifiedDiff].
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// The 1-based line number at which the previous version of the file started.
    pub old_start: u32,
    /// The non-zero amount of lines included in the previous version of the file.
    pub old_lines: u32,
    /// The 1-based line number at which the new version of the file started.
    pub new_start: u32,
    /// The non-zero amount of lines included in the new version of the file.
    pub new_lines: u32,
    /// A unified-diff formatted patch like:
    ///
    /// ```diff
    /// @@ -1,6 +1,8 @@
    /// This is the first line of the original text.
    /// -Line to be removed.
    /// +Line that has been replaced.
    ///  This is another line in the file.
    /// +This is a new line added at the end.
    /// ```
    ///
    /// The line separator is the one used in the original file and may be `LF` or `CRLF`.
    /// Note that the file-portion of the header isn't used here.
    pub diff: BString,
}

impl UnifiedDiff {
    /// Given a worktree-relative `path` to a resource already tracked in Git, or one that is currently untracked,
    /// create a patch in unified diff format that turns `previous_state` into `current_state_or_null` with the given
    /// amount of `context_lines`.
    /// `current_state_or_null` is either the hash of the state we know the resource currently has, or is a null-hash
    /// if the current state lives in the filesystem of the current worktree.
    /// If it is `None`, then there is no current state, and as long as a previous state is given, this will produce a
    /// unified diff for a deletion.
    /// `previous_state`, if `None`, indicates the file is new so there is nothing to compare to.
    /// Otherwise, it's the hash of the previously known state. It is never the null-hash.
    pub fn compute(
        repo: &gix::Repository,
        path: BString,
        current_state_or_null: impl Into<Option<gix::ObjectId>>,
        previous_state: impl Into<Option<gix::ObjectId>>,
        context_lines: usize,
    ) -> anyhow::Result<Self> {
        todo!()
    }
}

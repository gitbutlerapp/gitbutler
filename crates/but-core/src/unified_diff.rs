use super::{ChangeState, UnifiedDiff};
use bstr::{BStr, BString, ByteSlice};
use gix::diff::blob::platform::prepare_diff::Operation;
use gix::diff::blob::unified_diff::ContextSize;
use gix::diff::blob::ResourceKind;
use serde::Serialize;

/// A hunk as used in a [UnifiedDiff], which also contains all added and removed lines.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
    #[serde(serialize_with = "gitbutler_serde::bstring_lossy::serialize")]
    pub diff: BString,
}

impl UnifiedDiff {
    /// Given a worktree-relative `path` to a resource already tracked in Git, or one that is currently untracked,
    /// create a patch in unified diff format that turns `previous_state` into `current_state`, with the given
    /// amount of `context_lines`.
    /// If `previous_path` is not `None`, this indicates a rename happened, and would require both states to be given.
    /// If `None`, both resources are assumed to live in (or have lived in) `path`.
    /// Note that the path is relevant for reading `.gitattributes`, typically related to worktree or diff filters.
    ///
    /// `current_state` is either the state we know the resource currently has, or is `None`, if there is no current state.
    /// `previous_state`, if `None`, indicates the file is new so there is nothing to compare to.
    /// Otherwise, it's the state of the resource as previously known.
    ///
    /// ### Special Types
    ///
    /// Note that *Submodules* won't render as patches, they have to be caught in the UI to render their previous hash
    /// and current hash directly.
    /// Type-changes, from file to submodule or vice-versa for instance, should be shown as typechange only, probably showing
    /// the old and the new type, without diff preview for now.
    pub fn compute(
        repo: &gix::Repository,
        path: &BStr,
        previous_path: Option<&BStr>,
        current_state: impl Into<Option<ChangeState>>,
        previous_state: impl Into<Option<ChangeState>>,
        context_lines: u32,
    ) -> anyhow::Result<Self> {
        let current_state = current_state.into();
        let previous_state = previous_state.into();
        let mut cache = repo.diff_resource_cache(
            gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent,
            gix::diff::blob::pipeline::WorktreeRoots {
                old_root: None,
                new_root: current_state
                    .filter(|state| state.id.is_null())
                    .and_then(|_| repo.work_dir().map(ToOwned::to_owned)),
            },
        )?;

        cache.set_resource(
            current_state.map_or(repo.object_hash().null(), |state| state.id),
            current_state.map_or_else(
                || {
                    previous_state
                        .expect("BUG: at least one non-none state")
                        .kind
                },
                |state| state.kind,
            ),
            path.as_bstr(),
            ResourceKind::NewOrDestination,
            repo,
        )?;
        cache.set_resource(
            previous_state.map_or(repo.object_hash().null(), |state| state.id),
            previous_state.map_or_else(
                || {
                    current_state
                        .expect("BUG: at least one non-none state")
                        .kind
                },
                |state| state.kind,
            ),
            previous_path.unwrap_or(path.as_bstr()),
            ResourceKind::OldOrSource,
            repo,
        )?;

        let prep = cache.prepare_diff()?;
        Ok(match prep.operation {
            Operation::InternalDiff { algorithm } => {
                #[derive(Default)]
                struct ProduceDiffHunk {
                    hunks: Vec<DiffHunk>,
                }
                impl gix::diff::blob::unified_diff::ConsumeHunk for ProduceDiffHunk {
                    type Out = Vec<DiffHunk>;

                    fn consume_hunk(
                        &mut self,
                        before_hunk_start: u32,
                        before_hunk_len: u32,
                        after_hunk_start: u32,
                        after_hunk_len: u32,
                        header: &str,
                        hunk: &[u8],
                    ) -> std::io::Result<()> {
                        self.hunks.push(DiffHunk {
                            old_start: before_hunk_start,
                            old_lines: before_hunk_len,
                            new_start: after_hunk_start,
                            new_lines: after_hunk_len,
                            diff: {
                                let mut buf = Vec::with_capacity(header.len() + hunk.len());
                                buf.extend_from_slice(header.as_bytes());
                                buf.extend_from_slice(hunk);
                                buf.into()
                            },
                        });
                        Ok(())
                    }

                    fn finish(self) -> Self::Out {
                        self.hunks
                    }
                }
                let input = prep.interned_input();
                UnifiedDiff::Patch {
                    hunks: gix::diff::blob::diff(
                        algorithm,
                        &input,
                        gix::diff::blob::UnifiedDiff::new(
                            &input,
                            ProduceDiffHunk::default(),
                            gix::diff::blob::unified_diff::NewlineSeparator::AfterHeaderAndWhenNeeded("\n"),
                            ContextSize::symmetrical(context_lines),
                        ),
                    )?,
                }
            }
            Operation::ExternalCommand { .. } => {
                unreachable!("BUG: `gix` disables this, as it knows we always need to be able to run our own diff machinery")
            }
            Operation::SourceOrDestinationIsBinary => {
                use gix::diff::blob::platform::resource::Data;
                fn size_for_data(data: Data<'_>) -> Option<u64> {
                    match data {
                        Data::Missing | Data::Buffer(_) => None,
                        Data::Binary { size } => Some(size),
                    }
                }
                let (old, new) = cache
                    .resources()
                    .expect("prepare would have failed if a resource is missing");
                let size = size_for_data(old.data)
                    .or(size_for_data(new.data))
                    .expect("BUG: one of the resources must have been binary/too big");
                let big_file_size = repo.big_file_threshold()?;
                if size > big_file_size {
                    UnifiedDiff::TooLarge {
                        size_in_bytes: size,
                    }
                } else {
                    UnifiedDiff::Binary
                }
            }
        })
    }
}

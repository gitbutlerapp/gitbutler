use bstr::{BStr, BString, ByteSlice};
use gix::diff::blob::{
    ResourceKind,
    platform::prepare_diff::Operation,
    unified_diff::{ConsumeBinaryHunk, ContextSize, HunkHeader},
};
use serde::Serialize;

use super::{ChangeState, UnifiedPatch};

/// A hunk as used in a [UnifiedPatch], which also contains all added and removed lines.
#[derive(Clone, Serialize)]
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
    ///
    /// Also note that this has possibly been decoded lossily, assuming UTF8 if the encoding couldn't be determined,
    /// replacing invalid codepoints with markers.
    #[serde(serialize_with = "gitbutler_serde::bstring_lossy::serialize")]
    pub diff: BString,
}

impl std::fmt::Debug for DiffHunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, r#"DiffHunk("{}")"#, self.diff)
        } else {
            write!(f, r#"DiffHunk("{:?}")"#, self.diff)
        }
    }
}

impl UnifiedPatch {
    /// Determine how resources are converted to their form used for diffing.
    ///
    /// `ToGit` means that we want to see manifests of `git-lfs` for instance, or generally the result of 'clean' filters.
    /// Doing so also yields a more 'universal' form that is certainly helpful when displaying it in a user interface.
    pub const CONVERSION_MODE: gix::diff::blob::pipeline::Mode =
        gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent;

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
    /// Return `None` if the given states cannot produce a diff, typically because a submodule is involved.
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
    ) -> anyhow::Result<Option<Self>> {
        let current_state = current_state.into();
        let mut cache = filter_from_state(repo, current_state, Self::CONVERSION_MODE)?;
        Self::compute_with_filter(
            repo,
            path,
            previous_path,
            current_state,
            previous_state,
            context_lines,
            &mut cache,
        )
    }

    /// Similar to [`Self::compute()`], but uses `diff_filter` to obtain the diff content.
    ///
    /// This is useful to assure it's clear which content is ultimately used for the produced uni-diff,
    /// as `filter` is responsible for that.
    pub fn compute_with_filter(
        repo: &gix::Repository,
        path: &BStr,
        previous_path: Option<&BStr>,
        current_state: impl Into<Option<ChangeState>>,
        previous_state: impl Into<Option<ChangeState>>,
        context_lines: u32,
        diff_filter: &mut gix::diff::blob::Platform,
    ) -> anyhow::Result<Option<Self>> {
        use gix::diff::blob;
        let current_state = current_state.into();
        let previous_state = previous_state.into();
        match diff_filter.set_resource(
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
        ) {
            Ok(()) => {}
            Err(
                blob::platform::set_resource::Error::InvalidMode { .. }
                | blob::platform::set_resource::Error::ConvertToDiffable(
                    blob::pipeline::convert_to_diffable::Error::InvalidEntryKind { .. },
                ),
            ) => {
                return Ok(None);
            }
            Err(err) => return Err(err.into()),
        };
        match diff_filter.set_resource(
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
        ) {
            Ok(()) => {}
            Err(
                blob::platform::set_resource::Error::InvalidMode { .. }
                | blob::platform::set_resource::Error::ConvertToDiffable(
                    blob::pipeline::convert_to_diffable::Error::InvalidEntryKind { .. },
                ),
            ) => {
                return Ok(None);
            }
            Err(err) => return Err(err.into()),
        };

        let prep = diff_filter.prepare_diff()?;
        Ok(Some(match prep.operation {
            Operation::InternalDiff { algorithm } => {
                #[derive(Default)]
                struct ProduceDiffHunk {
                    hunks: Vec<DiffHunk>,
                }
                impl gix::diff::blob::unified_diff::ConsumeBinaryHunkDelegate for ProduceDiffHunk {
                    fn consume_binary_hunk(
                        &mut self,
                        header: HunkHeader,
                        header_str: &str,
                        hunk: &[u8],
                    ) -> std::io::Result<()> {
                        self.hunks.push(DiffHunk {
                            old_start: header.before_hunk_start,
                            old_lines: header.before_hunk_len,
                            new_start: header.after_hunk_start,
                            new_lines: header.after_hunk_len,
                            diff: {
                                let mut buf = Vec::with_capacity(header_str.len() + hunk.len());
                                buf.extend_from_slice(header_str.as_bytes());
                                buf.extend_from_slice(hunk);
                                detect_and_convert_to_utf8(buf.into())
                            },
                        });
                        Ok(())
                    }
                }
                let input = prep.interned_input();
                let uni_diff = gix::diff::blob::UnifiedDiff::new(
                    &input,
                    ConsumeBinaryHunk::new(ProduceDiffHunk::default(), "\n"),
                    ContextSize::symmetrical(context_lines),
                );
                let hunks = gix::diff::blob::diff(algorithm, &input, uni_diff)?.hunks;
                let (lines_added, lines_removed) = compute_line_changes(&hunks);
                UnifiedPatch::Patch {
                    is_result_of_binary_to_text_conversion: prep.old_or_new_is_derived,
                    hunks,
                    lines_added,
                    lines_removed,
                }
            }
            Operation::ExternalCommand { .. } => {
                unreachable!(
                    "BUG: `gix` disables this, as it knows we always need to be able to run our own diff machinery"
                )
            }
            Operation::SourceOrDestinationIsBinary => {
                use gix::diff::blob::platform::resource::Data;
                fn size_for_data(data: Data<'_>) -> Option<u64> {
                    match data {
                        Data::Missing | Data::Buffer { .. } => None,
                        Data::Binary { size } => Some(size),
                    }
                }
                let (old, new) = diff_filter
                    .resources()
                    .expect("prepare would have failed if a resource is missing");
                let size = size_for_data(old.data)
                    .or(size_for_data(new.data))
                    .expect("BUG: one of the resources must have been binary/too big");
                let big_file_size = repo.big_file_threshold()?;
                if size > big_file_size {
                    UnifiedPatch::TooLarge {
                        size_in_bytes: size,
                    }
                } else {
                    UnifiedPatch::Binary
                }
            }
        }))
    }
}

/// Detect the encoding of the given byte content and convert it to UTF-8, after attempting to guess its encoding.
/// Even if decoding failed, we always return the original `content` in the wirst case.
fn detect_and_convert_to_utf8(content: BString) -> BString {
    // Use chardet to detect the encoding
    let mut detect = chardetng::EncodingDetector::new();
    detect.feed(&content, true);
    let (encoding, high_confidence) = detect.guess_assess(None, true);
    if encoding.name() == "UTF-8" || !high_confidence {
        return content;
    }

    // Convert to UTF-8
    let (decoded, _actual_encoding, _used_replacement_chars) = encoding.decode(&content);
    decoded.into_owned().into()
}

fn compute_line_changes(hunks: &Vec<DiffHunk>) -> (u32, u32) {
    let mut lines_added = 0;
    let mut lines_removed = 0;
    for hunk in hunks {
        hunk.diff.lines().for_each(|line| {
            if line.starts_with(b"+") {
                lines_added += 1;
            } else if line.starts_with(b"-") {
                lines_removed += 1;
            }
        });
    }
    (lines_added, lines_removed)
}

/// Produce a filter from `repo` and `state` using `mode` that is able to perform diffs of `state`.
pub fn filter_from_state(
    repo: &gix::Repository,
    state: Option<ChangeState>,
    filter_mode: gix::diff::blob::pipeline::Mode,
) -> anyhow::Result<gix::diff::blob::Platform> {
    Ok(repo.diff_resource_cache(
        filter_mode,
        gix::diff::blob::pipeline::WorktreeRoots {
            old_root: None,
            new_root: state
                .filter(|state| state.id.is_null())
                .and_then(|_| repo.workdir().map(ToOwned::to_owned)),
        },
    )?)
}

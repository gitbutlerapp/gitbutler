use bstr::BString;
use but_ctx::Context;
use gix::refs::FullName;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{Purpose, ResolvedCliIdArg},
        diff2::Platform,
    },
    bad_input,
    id::{CommitId, CommittedFileId, IdAndHunk, UncommittedHunkOrFile},
    theme::{Paint as _, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        diff_rendering::{
            self, DetailsLine, DiffLineWriter, IdGen, WithSyntaxHighlighting, load_syntax_set,
        },
        string_interning::Strings,
    },
};

const CLEAR_TO_END_OF_LINE: &str = "\x1b[0K";

#[derive(Debug)]
pub struct DiffOutcome<'a> {
    ctx: &'a mut Context,
    target: DiffOperation,
}

impl CliOutputHuman for DiffOutcome<'_> {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self { ctx, target } = self;

        let syntax_set = load_syntax_set();
        let syntax_theme = theme.load_syntax_highlighting_theme()?;

        let strings = Strings::default();
        let writer = DiffWriter { out, theme };
        let mut writer =
            WithSyntaxHighlighting::new(writer, strings.clone(), &syntax_set, &syntax_theme);
        let mut id_gen = IdGen::new(strings);

        let options = diff_rendering::Options {
            skip_commit_header: true,
            skip_line_stats: true,
        };

        match target {
            DiffOperation::Uncommitted => {
                diff_rendering::render_uncommitted(ctx, theme, &mut id_gen, options, &mut writer)?;
            }
            DiffOperation::Commit { commit } => {
                diff_rendering::render_commit(
                    commit.commit_id,
                    commit.change_id,
                    ctx,
                    theme,
                    &mut id_gen,
                    options,
                    &mut writer,
                )?;
            }
            DiffOperation::Branch { branch } => {
                let branch = branch.shorten().to_string();
                diff_rendering::render_branch(
                    branch,
                    ctx,
                    theme,
                    &mut id_gen,
                    options,
                    &mut writer,
                )?;
            }
            DiffOperation::UncommittedHunkOrFile { hunk } => {
                diff_rendering::render_uncommitted_hunk(
                    *hunk,
                    theme,
                    &mut id_gen,
                    options,
                    &mut writer,
                )?;
            }
            DiffOperation::CommittedFile { commit, path } => {
                diff_rendering::render_committed_file(
                    commit.commit_id,
                    path,
                    ctx,
                    theme,
                    &mut id_gen,
                    options,
                    &mut writer,
                )?;
            }
            DiffOperation::PathPrefix { id, hunks } => {
                diff_rendering::render_path_prefix(
                    &id,
                    hunks,
                    ctx,
                    theme,
                    &mut id_gen,
                    options,
                    &mut writer,
                )?;
            }
        }

        Ok(())
    }
}

impl CliOutput for DiffOutcome<'_> {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            changes: Vec<Change>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Change {
            #[serde(skip_serializing_if = "Option::is_none")]
            id: Option<String>,
            path: String,
            status: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            old_path: Option<String>,
            diff: Diff,
        }

        #[derive(Serialize)]
        #[serde(tag = "type", rename_all = "camelCase")]
        enum Diff {
            Binary,
            TooLarge {
                size_in_bytes: u64,
            },
            Patch {
                hunks: Vec<Hunk>,
                #[serde(skip_serializing_if = "std::ops::Not::not")]
                is_binary_to_text: bool,
            },
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Hunk {
            old_start: u32,
            old_lines: u32,
            new_start: u32,
            new_lines: u32,
            diff: String,
        }

        fn hunk_to_json_hunk(hunk: &but_core::unified_diff::DiffHunk) -> Hunk {
            use bstr::ByteSlice as _;

            Hunk {
                old_start: hunk.old_start,
                old_lines: hunk.old_lines,
                new_start: hunk.new_start,
                new_lines: hunk.new_lines,
                diff: hunk.diff.to_str_lossy().into_owned(),
            }
        }

        fn hunk_to_change(id: Option<&str>, hunk: &but_core::SingleHunk) -> Change {
            use bstr::ByteSlice as _;

            let diff = if let (Some(diff), Some(header)) = (&hunk.diff, &hunk.hunk_header) {
                Diff::Patch {
                    hunks: vec![hunk_to_json_hunk(&but_core::unified_diff::DiffHunk {
                        old_start: header.old_start,
                        old_lines: header.old_lines,
                        new_start: header.new_start,
                        new_lines: header.new_lines,
                        diff: diff.clone(),
                    })],
                    is_binary_to_text: false,
                }
            } else {
                Diff::Patch {
                    hunks: vec![],
                    is_binary_to_text: false,
                }
            };

            Change {
                id: id.map(str::to_owned),
                path: hunk.path.to_str_lossy().into_owned(),
                status: "modified".to_owned(),
                old_path: None,
                diff,
            }
        }

        fn hunk_changes(mut hunks: Vec<(&str, &but_core::SingleHunk)>) -> Vec<Change> {
            hunks.sort_by(|(_, a_hunk), (_, b_hunk)| {
                a_hunk
                    .path
                    .cmp(&b_hunk.path)
                    .then_with(|| a_hunk.hunk_header.cmp(&b_hunk.hunk_header))
            });
            hunks
                .into_iter()
                .map(|(id, hunk)| hunk_to_change(Some(id), hunk))
                .collect()
        }

        fn tree_change_to_change(ctx: &Context, change: but_core::ui::TreeChange) -> Change {
            use but_core::{UnifiedPatch, ui::TreeStatus};

            let (status, old_path) = match &change.status {
                TreeStatus::Addition { .. } => ("added", None),
                TreeStatus::Deletion { .. } => ("deleted", None),
                TreeStatus::Modification { .. } => ("modified", None),
                TreeStatus::Rename { previous_path, .. } => {
                    ("renamed", Some(previous_path.to_string()))
                }
            };

            let patch = but_api::diff::tree_change_diffs(ctx, change.clone())
                .ok()
                .flatten();
            let diff = match patch {
                Some(UnifiedPatch::Binary) => Diff::Binary,
                Some(UnifiedPatch::TooLarge { size_in_bytes }) => Diff::TooLarge { size_in_bytes },
                Some(UnifiedPatch::Patch {
                    hunks,
                    is_result_of_binary_to_text_conversion,
                    ..
                }) => Diff::Patch {
                    hunks: hunks.iter().map(hunk_to_json_hunk).collect(),
                    is_binary_to_text: is_result_of_binary_to_text_conversion,
                },
                None => Diff::Patch {
                    hunks: vec![],
                    is_binary_to_text: false,
                },
            };

            Change {
                id: None,
                path: change.path_bytes.to_string(),
                status: status.to_owned(),
                old_path,
                diff,
            }
        }

        fn commit_changes(
            ctx: &Context,
            commit: gix::ObjectId,
            path: Option<&BString>,
        ) -> anyhow::Result<Vec<Change>> {
            let details =
                but_api::diff::commit_details(ctx, commit, but_api::diff::ComputeLineStats::No)?;
            Ok(details
                .diff_with_first_parent
                .into_iter()
                .filter(|change| path.is_none_or(|path| path == &change.path))
                .map(|change| tree_change_to_change(ctx, change.into()))
                .collect())
        }

        fn build_output(ctx: &Context, target: &DiffOperation) -> anyhow::Result<Output> {
            let changes = match target {
                DiffOperation::Uncommitted => {
                    let id_map = IdMap::legacy_new_from_context(ctx)?;
                    hunk_changes(
                        id_map
                            .uncommitted_hunks
                            .iter()
                            .map(|(id, hunk)| (id.as_str(), &hunk.hunk))
                            .collect(),
                    )
                }
                DiffOperation::Commit { commit } => commit_changes(ctx, commit.commit_id, None)?,
                DiffOperation::Branch { branch } => {
                    let branch = branch.shorten().to_string();
                    let branch_diff = but_api::branch::branch_diff(ctx, branch)?;
                    branch_diff
                        .changes
                        .into_iter()
                        .map(|change| tree_change_to_change(ctx, change))
                        .collect()
                }
                DiffOperation::UncommittedHunkOrFile { hunk } => hunk_changes(
                    hunk.hunks
                        .iter()
                        .map(|hunk| (hunk.id.as_str(), &hunk.hunk))
                        .collect(),
                ),
                DiffOperation::CommittedFile { commit, path } => {
                    commit_changes(ctx, commit.commit_id, Some(path))?
                }
                DiffOperation::PathPrefix { hunks, .. } => hunk_changes(
                    hunks
                        .iter()
                        .map(|hunk| (hunk.id.as_str(), &hunk.hunk))
                        .collect(),
                ),
            };

            Ok(Output { changes })
        }

        struct DeferredOutput<'a> {
            ctx: &'a Context,
            target: DiffOperation,
        }

        impl Serialize for DeferredOutput<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let output = build_output(self.ctx, &self.target)
                    .map_err(<S::Error as serde::ser::Error>::custom)?;
                output.serialize(serializer)
            }
        }

        let Self { ctx, target } = self;
        DeferredOutput { ctx, target }
    }
}

struct DiffWriter<'a> {
    out: &'a mut dyn WriteWithUtils,
    theme: &'static Theme,
}

impl DiffLineWriter for DiffWriter<'_> {
    fn write(&mut self, line: DetailsLine) -> anyhow::Result<()> {
        match line {
            DetailsLine::Text { line, .. } => {
                let line_style = line.style;
                for span in line.spans {
                    let rendered = line_style.patch(span.style).paint(&span.content);
                    write!(self.out, "{rendered}")?;
                }
                writeln!(self.out)?;
            }
            DetailsLine::TextToWrap { id: _, text } => {
                writeln!(self.out, "{text}")?;
            }
            DetailsLine::Code(code_line) => {
                let syntax_highlighted_line = code_line.syntax_highlighted_line.borrow();
                let syntax_highlighted_line = syntax_highlighted_line
                    .as_ref()
                    .expect("WithSyntaxHighlighting ensures the line is highlighted");

                let line_style = syntax_highlighted_line.style;
                for span in syntax_highlighted_line {
                    let rendered = line_style.patch(span.style).paint(&span.content);
                    write!(self.out, "{rendered}")?;
                }
                if line_style.bg.is_some() && colored::control::SHOULD_COLORIZE.should_colorize() {
                    write!(self.out, "{}", line_style.paint(CLEAR_TO_END_OF_LINE))?;
                }
                writeln!(self.out)?;
            }
            DetailsLine::SectionSeparator => {
                writeln!(self.out)?;
            }
            DetailsLine::HunkHeader { width, line, .. } => {
                for _ in 0..width {
                    write!(self.out, "{}", self.theme.border.paint("─"))?;
                }
                writeln!(self.out, "{}", self.theme.border.paint("╮"))?;

                for span in line {
                    let rendered = span.style.paint(&span.content);
                    write!(self.out, "{rendered}")?;
                }
                writeln!(self.out)?;

                for _ in 0..width {
                    write!(self.out, "{}", self.theme.border.paint("─"))?;
                }
                writeln!(self.out, "{}", self.theme.border.paint("╯"))?;

                writeln!(self.out, " ")?;
            }
        }

        Ok(())
    }
}

pub fn diff<'a>(
    ctx: &'a mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<DiffOutcome<'a>> {
    let guard = ctx.shared_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let op = resolve(ctx, &id_map, args)?;
    Ok(run(ctx, op)?)
}

fn resolve(ctx: &Context, id_map: &IdMap, args: Platform) -> CliResult<DiffOperation> {
    let Platform { target } = args;

    let resolved_target = if let Some(target) = target {
        let repo = ctx.repo.get()?;
        target.resolve_in_workspace(&repo, id_map, Purpose::Target, None)?
    } else {
        ResolvedCliIdArg::Uncommitted
    };

    match resolved_target {
        ResolvedCliIdArg::Uncommitted => Ok(DiffOperation::Uncommitted),
        ResolvedCliIdArg::Commit(commit) => Ok(DiffOperation::Commit { commit }),
        ResolvedCliIdArg::Branch(branch) => {
            let branch = branch.resolve_local_branch_name()?;
            Ok(DiffOperation::Branch { branch })
        }
        ResolvedCliIdArg::UncommittedHunkOrFile(hunk) => {
            Ok(DiffOperation::UncommittedHunkOrFile { hunk })
        }
        ResolvedCliIdArg::CommittedFile(CommittedFileId {
            commit_id,
            path,
            change_id,
        }) => Ok(DiffOperation::CommittedFile {
            commit: CommitId {
                commit_id,
                change_id,
            },
            path,
        }),
        ResolvedCliIdArg::PathPrefix { id, hunks } => Ok(DiffOperation::PathPrefix { id, hunks }),
        ResolvedCliIdArg::Stack { .. } => {
            Err(bad_input("viewing diffs for stack assignments is not supported").into())
        }
    }
}

fn run(ctx: &mut Context, op: DiffOperation) -> anyhow::Result<DiffOutcome<'_>> {
    Ok(DiffOutcome { ctx, target: op })
}

#[derive(Debug)]
enum DiffOperation {
    Uncommitted,
    Commit {
        commit: CommitId,
    },
    Branch {
        branch: FullName,
    },
    UncommittedHunkOrFile {
        hunk: Box<UncommittedHunkOrFile>,
    },
    CommittedFile {
        commit: CommitId,
        path: BString,
    },
    PathPrefix {
        id: String,
        hunks: NonEmpty<IdAndHunk>,
    },
}

use bstr::BString;
use but_ctx::Context;
use gix::{ObjectId, refs::FullName};
use serde::Serialize;
use syntect::parsing::SyntaxSet;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{Purpose, ResolvedCliIdArg},
        diff2::Platform,
    },
    bad_input,
    id::{ShortId, UncommittedHunkOrFile},
    theme::{Paint as _, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils,
        diff_rendering::{self, DetailsLine, DiffLineWriter, IdGen, WithSyntaxHighlighting},
        string_interning::Strings,
    },
};

#[derive(Debug)]
pub struct DiffOutcome<'a> {
    ctx: &'a mut Context,
    target: DiffOperation,
}

impl CliOutputHuman for DiffOutcome<'_> {
    fn on_human(self, out: &mut dyn WriteWithUtils, theme: &'static Theme) -> anyhow::Result<()> {
        let Self { ctx, target } = self;

        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax_theme = theme.load_syntax_highlighting_theme()?;

        let strings = Strings::default();
        let writer = DiffWriter { out };
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
                    commit,
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
                    ctx,
                    theme,
                    &mut id_gen,
                    options,
                    &mut writer,
                )?;
            }
            DiffOperation::CommittedFile {
                commit_id,
                path,
                id,
            } => {
                diff_rendering::render_committed_file(
                    commit_id,
                    path,
                    id,
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
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()> {
        self.on_human(out, crate::theme::get())
    }

    fn on_json(self) -> impl Serialize {
        // TODO(david)

        #[derive(Serialize)]
        struct Output {}

        Output {}
    }
}

struct DiffWriter<'a> {
    out: &'a mut dyn WriteWithUtils,
}

impl DiffLineWriter for DiffWriter<'_> {
    fn write(&mut self, line: DetailsLine) -> anyhow::Result<()> {
        match line {
            DetailsLine::Text {
                id: _,
                cli_id: _,
                line,
                skip_when_copying_hunk: _,
            } => {
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

                for span in syntax_highlighted_line {
                    let rendered = span.style.paint(&span.content);
                    write!(self.out, "{rendered}")?;
                }
                writeln!(self.out)?;
            }
            DetailsLine::Image(_) => {
                // This writer never reports image support, so image lines are not produced
                // for it; keep the placeholder in case that ever changes.
                writeln!(self.out, "Binary file - no diff available")?;
            }
            DetailsLine::SectionSeparator => {
                writeln!(self.out)?;
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
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

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
        ResolvedCliIdArg::CommittedFile {
            commit_id,
            path,
            id,
        } => Ok(DiffOperation::CommittedFile {
            commit_id,
            path,
            id,
        }),
        ResolvedCliIdArg::Stack => {
            Err(bad_input("viewing diffs for stack assignments is not supported").into())
        }
        ResolvedCliIdArg::PathPrefix => {
            // TODO(david)
            Err(anyhow::anyhow!("path prefix targets are not yet implemented").into())
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
        commit: ObjectId,
    },
    Branch {
        branch: FullName,
    },
    UncommittedHunkOrFile {
        hunk: Box<UncommittedHunkOrFile>,
    },
    CommittedFile {
        commit_id: ObjectId,
        path: BString,
        id: ShortId,
    },
}

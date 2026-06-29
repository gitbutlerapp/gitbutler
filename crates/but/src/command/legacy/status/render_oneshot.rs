use crate::{
    command::legacy::status::{
        FileLineContent, StatusOutputLine,
        output::{
            BranchLineContent, CommitLineContent, StatusOutputContent, UncommittedLineContent,
        },
    },
    theme::Paint,
    utils::WriteWithUtils,
};

/// Print one line of status output.
///
/// Works by translating the ratatui lines and spans into `colored` text and printing it.
pub(super) fn render_oneshot(
    line: StatusOutputLine,
    out: &mut dyn WriteWithUtils,
) -> anyhow::Result<()> {
    let StatusOutputLine {
        connector,
        content,
        data: _,
    } = line;

    let mut spans = Vec::new();
    if let Some(mut connector) = connector {
        spans.append(&mut connector);
    }
    match content {
        StatusOutputContent::Plain(mut content) => {
            spans.append(&mut content);
        }
        StatusOutputContent::Commit(CommitLineContent {
            mut sha,
            mut author,
            mut message,
            mut suffix,
        }) => {
            spans.append(&mut sha);
            spans.append(&mut author);
            spans.append(&mut message);
            spans.append(&mut suffix);
        }
        StatusOutputContent::Branch(BranchLineContent {
            mut id,
            mut decoration_start,
            mut branch_name,
            mut decoration_end,
            mut suffix,
        }) => {
            spans.append(&mut id);
            spans.append(&mut decoration_start);
            spans.append(&mut branch_name);
            spans.append(&mut decoration_end);
            spans.append(&mut suffix);
        }
        StatusOutputContent::File(FileLineContent {
            mut id,
            mut status,
            mut path,
        }) => {
            spans.append(&mut id);
            spans.append(&mut status);
            spans.append(&mut path);
        }
        StatusOutputContent::Uncommitted(UncommittedLineContent {
            mut id,
            mut decoration_start,
            mut label,
            mut decoration_end,
            mut suffix,
        }) => {
            spans.append(&mut id);
            spans.append(&mut decoration_start);
            spans.append(&mut label);
            spans.append(&mut decoration_end);
            spans.append(&mut suffix);
        }
    }

    for span in spans {
        let rendered = span.style.paint(&span.content);
        write!(out, "{rendered}")?;
    }

    writeln!(out)?;

    Ok(())
}

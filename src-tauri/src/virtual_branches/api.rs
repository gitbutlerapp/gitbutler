use std::{ops::Range, vec};

use anyhow::{Context, Result};

use crate::{
    deltas::{self, Document},
    gb_repository, reader, sessions,
};

use super::branch;

fn read_virtual_file(
    branch_reader: &branch::Reader,
    file_path: &str,
    vbranch_id: &str,
) -> Result<String> {
    let initial_file = branch_reader
        .read_wd_file(vbranch_id, file_path)
        .context("failed to read file")?;
    let deltas = branch_reader
        .read_deltas(vbranch_id, file_path)
        .context("failed to read deltas")?;
    let doc = Document::new(Some(&initial_file), deltas).context("failed to create document")?;
    Ok(doc.to_string())
}

fn read_real_file(session_reader: &sessions::Reader, file_path: &str) -> Result<String> {
    let initial_file = match session_reader
        .file(file_path)
        .context("failed to read file")?
    {
        reader::Content::UTF8(content) => content,
        reader::Content::Binary(_) => {
            return Err(anyhow::anyhow!("file is binary"));
        }
    };
    let deltas = deltas::Reader::new(session_reader)
        .read_file(file_path)
        .context("failed to read deltas")?;
    let doc = Document::new(Some(&initial_file), deltas.unwrap_or_default())
        .context("failed to create document")?;
    Ok(doc.to_string())
}

fn move_lines(
    gb_repo: &gb_repository::Repository,
    file_path: &str,
    line_range: &Range<usize>,
    src_vbranch_id: &str,
    dst_vbranch_id: &str,
) -> Result<()> {
    let current_session = gb_repo
        .get_or_create_current_session()
        .context("failed to get current session")?;
    let current_session_reader = sessions::Reader::open(gb_repo, &current_session)
        .context("failed to open current session")?;
    let vbranch_reader = branch::Reader::new(&current_session_reader);

    let src_content = read_virtual_file(&vbranch_reader, file_path, src_vbranch_id)
        .context("failed to read source file")?;
    let wd_content =
        read_real_file(&current_session_reader, file_path).context("failed to read real file")?;
    let dst_content = read_virtual_file(&vbranch_reader, file_path, dst_vbranch_id)
        .context("failed to read destination file")?;

    let line_number = find_range(&src_content, &wd_content, &dst_content, &line_range);

    println!("line number: {:?}", line_number);

    Ok(())
}

// given a range of chars in the source file,
// and a merge result of both source and destination files,
// returns a position in the destination file to where the range of chars should be inserted.
// returns None if the source range can't be moved to the destination file.
fn find_range(src: &str, middle: &str, dst: &str, src_range: &Range<usize>) -> Option<usize> {
    let middle_range = map_range(src, src_range, middle)?;
    // TODO...
}

// given chars range of a source file, returns chars range of the same chars in a destination file.
// returns None if the source range is not found in the destination file.
fn map_range(src: &str, range: &Range<usize>, dst: &str) -> Option<Range<usize>> {
    let src_range = src.get(range.clone())?;
    let start = dst.find(src_range)?;
    let end = start + src_range.len();
    Some(start..end)
}

#[cfg(test)]
mod test {}

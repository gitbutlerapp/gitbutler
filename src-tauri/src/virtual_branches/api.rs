use std::ops::Range;

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

//
// consider the following scenario:
//
//  previous session |  head session
//                   |
//                 o-|-o-o-o-o   <- src vbranch
//                /  |
//  -o-o-o-o-o-o-o-o-|-o-o-o-o-o-o-o-o-o  <- current working directory
//                   |  \_
//                   |    \
//                   |     o-o-o-o   <- dst vbranch
//                   |
//
// we want to move lines 2-4 from src vbranch to dst vbranch
//
// we achieve it by applying a delete operation to src branch and an insert operation
// to dst branch
//
// to correctly apply an insert operation, we need to know common root for all of:
// * src vbranch
// * dst vbranch
// * current working directory.
//
// using common root files and operation history for all, we use that information to calculate
// the correct offset for the insert operation.
//
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

    let src_vbranch = vbranch_reader
        .read(src_vbranch_id)
        .context("failed to read src vbranch")?;
    let dst_vbranch = vbranch_reader
        .read(dst_vbranch_id)
        .context("failed to read dst vbranch")?;
    let earliest_vbranch_created_timestamp_ms = src_vbranch
        .created_timestamp_ms
        .min(dst_vbranch.created_timestamp_ms);

    // find first session during which the earliest vbranch was created
    // and read all deltas from that session to now
    let mut earliest_session: sessions::Session = current_session;
    let mut wd_deltas: Vec<deltas::Delta> = vec![];
    let sessions_iterator = gb_repo
        .get_sessions_iterator()
        .context("failed to get sessions")?;
    for session in sessions_iterator {
        let session = session.context("failed to read session")?;
        if session.meta.last_timestamp_ms < earliest_vbranch_created_timestamp_ms {
            break;
        }

        earliest_session = session.clone();
        deltas::Reader::new(&current_session_reader)
            .read_file(file_path)
            .context("failed to read deltas")?
            .unwrap_or_default()
            .iter()
            .for_each(|delta| {
                wd_deltas.push(delta.clone());
            });
    }

    // read vbranch deltas
    let src_vbranch_deltas = vbranch_reader
        .read_deltas(src_vbranch_id, file_path)
        .context("failed to read src vbranch deltas")?;
    let dst_vbranch_deltas = vbranch_reader
        .read_deltas(dst_vbranch_id, file_path)
        .context("failed to read dst vbranch deltas")?;

    let earliest_vbranch_delta_timestamp = src_vbranch_deltas[0]
        .timestamp_ms
        .min(dst_vbranch_deltas[0].timestamp_ms);

    // read common root file
    let earlieat_session_init_file = match sessions::Reader::open(gb_repo, &earliest_session)
        .context(format!("failed to open session {}", earliest_session.id))?
        .file(file_path)
        .context(format!(
            "failed to read file from session {}",
            earliest_session.id
        ))? {
        reader::Content::UTF8(content) => content,
        reader::Content::Binary(_) => {
            return Err(anyhow::anyhow!("file is binary"));
        }
    };

    let root_doc = Document::new(
        Some(&earlieat_session_init_file),
        // filter out deltas that are recorded into vbranches
        wd_deltas
            .iter()
            .cloned()
            .filter(|delta| delta.timestamp_ms < earliest_vbranch_delta_timestamp)
            .collect(),
    )
    .context("failed to create document")?;

    // filter out deltas that are not recorded into vbranches
    let wd_deltas = wd_deltas
        .iter()
        .filter(|delta| delta.timestamp_ms >= earliest_vbranch_delta_timestamp)
        .collect::<Vec<_>>();

    // TODO: now when all the data is read and prepared, time to calcualte the offset

    Ok(())
}

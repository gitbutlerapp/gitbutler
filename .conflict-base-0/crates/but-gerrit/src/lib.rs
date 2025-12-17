use std::fmt::Display;

use bstr::{BString, ByteSlice};
use but_core::commit::HeadersV2;
use but_ctx::Context;
use gitbutler_commit::commit_ext::CommitExt;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use uuid::Uuid;

use crate::parse::PushOutput;

pub mod parse;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum PushFlag {
    Wip,
    Ready,
    Private,
    Hashtag(String),
    Topic(String),
}

impl Display for PushFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushFlag::Wip => write!(f, "wip"),
            PushFlag::Ready => write!(f, "ready"),
            PushFlag::Private => write!(f, "private"),
            PushFlag::Hashtag(tag) => write!(f, "t={}", tag),
            PushFlag::Topic(topic) => write!(f, "topic={}", topic),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GerritChangeId(String);

impl From<Uuid> for GerritChangeId {
    fn from(value: Uuid) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(value);
        Self(format!("I{:x}", hasher.finalize()))
    }
}
impl Display for GerritChangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn set_trailers(commit: &mut gix::objs::Commit) {
    if let Some(headers) = HeadersV2::try_from_commit(commit) {
        commit.message = with_change_id_trailer(commit.message.clone(), headers.change_id.into());
    }
}

fn with_change_id_trailer(msg: BString, change_id: Uuid) -> BString {
    let change_id = GerritChangeId::from(change_id);
    let change_id_line = format!("Change-Id: {change_id}");
    let msg_bytes = msg.as_slice();

    if msg_bytes.find(b"\nChange-Id:").is_some() {
        return msg;
    }

    let lines: Vec<&[u8]> = msg_bytes.lines().collect();

    let is_trailer = |line: &[u8]| -> bool {
        if line.is_empty() {
            return false;
        }
        // A trailer has format "Token: value"
        if let Some(colon_pos) = line.find_byte(b':') {
            if colon_pos == 0 {
                return false;
            }
            let token = &line[..colon_pos];
            !token.contains(&b' ') && colon_pos + 1 < line.len()
        } else {
            false
        }
    };

    let mut last_non_empty = lines.len();
    for (i, line) in lines.iter().enumerate().rev() {
        if !line.is_empty() {
            last_non_empty = i + 1;
            break;
        }
    }

    let mut insert_pos = last_non_empty;
    let mut found_signed_off_by = false;
    let mut found_any_trailer = false;

    for i in (0..last_non_empty).rev() {
        let line = lines[i];

        if is_trailer(line) {
            found_any_trailer = true;
            if line.starts_with(b"Signed-off-by:") {
                found_signed_off_by = true;
                insert_pos = i;
            } else if !found_signed_off_by {
                // This is a non-Signed-off-by trailer, insert after it
                insert_pos = i + 1;
            }
        } else if !line.is_empty() {
            break;
        }
    }

    let mut result = BString::from(Vec::new());
    for (i, line) in lines.iter().enumerate() {
        if i == insert_pos {
            result.extend_from_slice(change_id_line.as_bytes());
            result.push(b'\n');
        }
        result.extend_from_slice(line);
        result.push(b'\n');
    }

    // If we're inserting at the end and didn't insert yet
    if insert_pos == lines.len() {
        // Only add a blank line separator if there were NO trailers found
        // (i.e., we're creating a new trailer block from scratch)
        // If there were trailers, we're appending to the existing trailer block
        if !found_any_trailer && !lines.is_empty() {
            let needs_separator = if let Some(last_line) = lines.last() {
                !last_line.is_empty()
            } else {
                false
            };
            if needs_separator {
                result.push(b'\n');
            }
        }
        result.extend_from_slice(change_id_line.as_bytes());
        result.push(b'\n');
    }

    result
}

pub fn record_push_metadata(
    ctx: &mut Context,
    repo: &gix::Repository,
    candidate_ids: Vec<gix::ObjectId>,
    push_output: PushOutput,
) -> anyhow::Result<()> {
    let mappings = mappings(repo, candidate_ids, push_output)?;
    let mut db = ctx.db.get_mut()?;
    let mut db = db.gerrit_metadata();

    for mapping in mappings {
        let existing = db.get(&mapping.change_id)?;
        let now = chrono::Utc::now().naive_utc();
        let commit_id_str = mapping.commit_id.to_string();

        match existing {
            Some(existing_meta) => {
                // Check if commit_id has changed
                if existing_meta.commit_id != commit_id_str {
                    // Update the entry with new commit_id and updated_at
                    let updated_meta = but_db::GerritMeta {
                        change_id: mapping.change_id,
                        commit_id: commit_id_str,
                        review_url: mapping.review_url,
                        created_at: existing_meta.created_at, // Keep original creation time
                        updated_at: now,
                    };
                    db.update(updated_meta)?;
                }
                // If commit_id matches, do nothing
            }
            None => {
                // Create new entry
                let new_meta = but_db::GerritMeta {
                    change_id: mapping.change_id,
                    commit_id: commit_id_str,
                    review_url: mapping.review_url,
                    created_at: now,
                    updated_at: now,
                };
                db.insert(new_meta)?;
            }
        }
    }

    Ok(())
}

struct ChangeIdMapping {
    commit_id: gix::ObjectId,
    change_id: String,
    review_url: String,
}

fn mappings(
    repo: &gix::Repository,
    candidate_ids: Vec<gix::ObjectId>,
    push_output: PushOutput,
) -> anyhow::Result<Vec<ChangeIdMapping>> {
    let mut mappings = vec![];
    for id in candidate_ids {
        let commit = repo.find_commit(id)?;
        let msg = commit.message_bstr().to_string();
        let title = msg.lines().next().unwrap_or_default();

        let change_id_review_url = push_output
            .changes
            .iter()
            .find(|c| c.commit_title == title)
            .and_then(|c| {
                commit
                    .change_id()
                    .map(|change_id| (change_id, c.url.clone()))
            });
        if let Some((change_id, review_url)) = change_id_review_url {
            mappings.push(ChangeIdMapping {
                commit_id: id,
                change_id,
                review_url,
            });
        }
    }
    Ok(mappings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_is_41_characters_long() {
        let uuid = Uuid::new_v4();
        let change_id = GerritChangeId::from(uuid);
        let output = format!("{change_id}");
        assert_eq!(output.len(), 41); // "I" + 40 hex chars
        assert!(output.starts_with('I'));
    }

    #[test]
    fn test_add_trailer_no_existing_trailers() {
        let uuid = Uuid::new_v4();
        let change_id = GerritChangeId::from(uuid);
        let change_id_line = format!("Change-Id: {change_id}\n");

        let msg = BString::from("Initial commit\n");
        let updated_msg = with_change_id_trailer(msg.clone(), uuid);
        assert!(
            updated_msg
                .as_slice()
                .windows(change_id_line.len())
                .any(|w| w == change_id_line.as_bytes())
        );
    }

    #[test]
    fn test_add_trailer_already_has_change_id() {
        let uuid = Uuid::new_v4();
        let change_id = GerritChangeId::from(uuid);
        let change_id_line = format!("Change-Id: {change_id}\n");

        let msg_with_change_id = BString::from(format!("Initial commit\n{change_id_line}"));
        let updated_msg = with_change_id_trailer(msg_with_change_id.clone(), uuid);
        assert_eq!(updated_msg, msg_with_change_id);
    }

    #[test]
    fn test_add_trailer_with_signed_off_by() {
        let uuid = Uuid::new_v4();
        let change_id = GerritChangeId::from(uuid);
        let change_id_line = format!("Change-Id: {change_id}\n");

        let msg_with_signed_off =
            BString::from("Initial commit\n\nSigned-off-by: User <alice@example.com>\n");
        let updated_msg = with_change_id_trailer(msg_with_signed_off.clone(), uuid);
        let updated_msg_str = updated_msg.as_bstr();
        let change_id_index = updated_msg_str.find(&change_id_line).unwrap();
        let signed_off_index = updated_msg_str.find("Signed-off-by:").unwrap();
        assert!(change_id_index < signed_off_index);

        // Case 4: Has Pick-to trailer (no extra blank line should be added)
        let msg_with_pick_to = BString::from(
            "macOS: Handle non-square system tray notification icons\n\
             If the provided icon is non-square the system will end up clipping it,\n\
             so let's pre-generate a square icon if needed.\n\
             \n\
             Pick-to: 6.10\n",
        );
        let updated_msg = with_change_id_trailer(msg_with_pick_to.clone(), uuid);
        let updated_msg_str = updated_msg.to_string();

        assert!(updated_msg_str.contains(&format!("Pick-to: 6.10\n{change_id_line}")));

        assert!(
            !updated_msg_str.contains("Pick-to: 6.10\n\nChange-Id:"),
            "Should not have blank line between trailers"
        );

        // Case 5: Has multiple trailers including Signed-off-by
        let msg_with_multiple = BString::from(
            "Fix bug in authentication\n\
             \n\
             Pick-to: 6.10\n\
             Acked-by: Reviewer <reviewer@example.com>\n\
             Signed-off-by: Author <author@example.com>\n",
        );
        let updated_msg = with_change_id_trailer(msg_with_multiple.clone(), uuid);
        let updated_msg_str = updated_msg.to_string();

        let acked_by_idx = updated_msg_str.find("Acked-by:").unwrap();
        let change_id_idx = updated_msg_str.find("Change-Id:").unwrap();
        let signed_off_idx = updated_msg_str.find("Signed-off-by:").unwrap();

        assert!(
            acked_by_idx < change_id_idx && change_id_idx < signed_off_idx,
            "Change-Id should be between Acked-by and Signed-off-by"
        );

        assert!(
            !updated_msg_str.contains("Acked-by: Reviewer <reviewer@example.com>\n\nChange-Id:"),
            "Should not have blank line after Acked-by"
        );
        assert!(
            !updated_msg_str.contains(&format!("{change_id_line}\nSigned-off-by:")),
            "Should not have blank line after Change-Id"
        );
    }
}

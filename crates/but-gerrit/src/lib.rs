use std::fmt::Display;

use bstr::{BString, ByteSlice};
use but_core::commit::HeadersV2;
use gitbutler_command_context::CommandContext;
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
    let change_id_line = format!("\nChange-Id: {change_id}\n");
    let msg_bytes = msg.as_slice();

    if msg_bytes.find(b"\nChange-Id:").is_some() {
        return msg;
    }

    let lines: Vec<&[u8]> = msg_bytes.lines().collect();
    let mut insert_pos = lines.len();

    for (i, line) in lines.iter().enumerate().rev() {
        if line.starts_with(b"Signed-off-by:") {
            insert_pos = i;
        }
    }

    let mut result = BString::from(Vec::new());
    for (i, line) in lines.iter().enumerate() {
        if i == insert_pos {
            result.extend_from_slice(change_id_line.as_bytes());
        }
        result.extend_from_slice(line);
        result.push(b'\n');
    }

    if insert_pos == lines.len() {
        result.extend_from_slice(change_id_line.as_bytes());
    }

    result
}

pub fn record_push_metadata(
    ctx: &mut CommandContext,
    repo: &gix::Repository,
    candidate_ids: Vec<gix::ObjectId>,
    push_output: PushOutput,
) -> anyhow::Result<()> {
    let mappings = mappings(repo, candidate_ids, push_output)?;
    let mut db = ctx.db()?.gerrit_metadata();

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
    fn test_add_trailers() {
        let uuid = Uuid::new_v4();
        let change_id = GerritChangeId::from(uuid);
        let change_id_line = format!("Change-Id: {change_id}\n");

        // Case 1: No trailers
        let msg = BString::from("Initial commit\n");
        let updated_msg = with_change_id_trailer(msg.clone(), uuid);
        assert!(
            updated_msg
                .as_slice()
                .windows(change_id_line.len())
                .any(|w| w == change_id_line.as_bytes())
        );

        // Case 2: Already has Change-Id
        let msg_with_change_id = BString::from(format!("Initial commit\n{change_id_line}"));
        let updated_msg = with_change_id_trailer(msg_with_change_id.clone(), uuid);
        assert_eq!(updated_msg, msg_with_change_id);

        // Case 3: Has Signed-off-by trailer
        let msg_with_signed_off =
            BString::from("Initial commit\nSigned-off-by: User <alice@example.com>\n");
        let updated_msg = with_change_id_trailer(msg_with_signed_off.clone(), uuid);
        let updated_msg_str = updated_msg.as_bstr();
        let change_id_index = updated_msg_str.find(&change_id_line).unwrap();
        let signed_off_index = updated_msg_str.find("Signed-off-by:").unwrap();
        assert!(change_id_index < signed_off_index);
    }
}

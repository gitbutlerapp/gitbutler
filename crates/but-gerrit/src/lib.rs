use bstr::{BString, ByteSlice};
use but_core::commit::HeadersV2;
use sha1::{Digest, Sha1};
use std::fmt::Display;
use uuid::Uuid;

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

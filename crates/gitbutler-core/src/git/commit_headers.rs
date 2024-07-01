use bstr::{BStr, BString};
use uuid::Uuid;

use super::CommitBuffer;

/// Header used to determine which version of the headers is in use. This should never be changed
const HEADERS_VERSION_HEADER: &str = "gitbutler-headers-version";

const V1_CHANGE_ID_HEADER: &str = "change-id";

/// Used to represent the old commit headers layout. This should not be used in new code
#[derive(Debug)]
struct CommitHeadersV1 {
    change_id: String,
}

/// The version number used to represent the V2 headers
const V2_HEADERS_VERSION: &str = "2";

const V2_CHANGE_ID_HEADER: &str = "gitbutler-change-id";
const V2_IS_UNAPPLIED_HEADER_COMMIT_HEADER: &str = "gitbutler-is-unapplied-header-commit";
const V2_VBRANCH_NAME_HEADER: &str = "gitbutler-vbranch-name";
#[derive(Debug)]
pub struct CommitHeadersV2 {
    pub change_id: String,
    pub is_unapplied_header_commit: bool,
    pub vbranch_name: Option<String>,
}

impl Default for CommitHeadersV2 {
    fn default() -> Self {
        CommitHeadersV2 {
            // Change ID using base16 encoding
            change_id: Uuid::new_v4().to_string(),
            is_unapplied_header_commit: false,
            vbranch_name: None,
        }
    }
}

impl From<CommitHeadersV1> for CommitHeadersV2 {
    fn from(commit_headers_v1: CommitHeadersV1) -> CommitHeadersV2 {
        CommitHeadersV2 {
            change_id: commit_headers_v1.change_id,
            is_unapplied_header_commit: false,
            vbranch_name: None,
        }
    }
}

pub trait HasCommitHeaders {
    fn gitbutler_headers(&self) -> Option<CommitHeadersV2>;
}

impl HasCommitHeaders for git2::Commit<'_> {
    fn gitbutler_headers(&self) -> Option<CommitHeadersV2> {
        if let Ok(header) = self.header_field_bytes(HEADERS_VERSION_HEADER) {
            let version_number = BString::new(header.to_owned());

            // Parse v2 headers
            if version_number == BStr::new(V2_HEADERS_VERSION) {
                let change_id = self.header_field_bytes(V2_CHANGE_ID_HEADER).ok()?;
                // We can safely assume that the change id should be UTF8
                let change_id = change_id.as_str()?.to_string();

                // We can rationalize about is unapplied header commit with a bstring
                let is_wip_commit = self
                    .header_field_bytes(V2_IS_UNAPPLIED_HEADER_COMMIT_HEADER)
                    .ok()?;
                let is_wip_commit = BString::new(is_wip_commit.to_owned());

                // We can safely assume that the vbranch name should be UTF8
                let vbranch_name = self
                    .header_field_bytes(V2_VBRANCH_NAME_HEADER)
                    .ok()
                    .and_then(|buffer| Some(buffer.as_str()?.to_string()));

                Some(CommitHeadersV2 {
                    change_id,
                    is_unapplied_header_commit: is_wip_commit == "true",
                    vbranch_name,
                })
            } else {
                // Must be for a version we don't recognise
                None
            }
        } else {
            // Parse v1 headers
            let change_id = self.header_field_bytes(V1_CHANGE_ID_HEADER).ok()?;
            // We can safely assume that the change id should be UTF8
            let change_id = change_id.as_str()?.to_string();

            let headers = CommitHeadersV1 { change_id };

            Some(headers.into())
        }
    }
}

impl CommitHeadersV2 {
    /// Used to create a CommitHeadersV2. This does not allow a change_id to be
    /// provided in order to ensure a consistent format.
    pub fn new(is_unapplied_header_commit: bool, vbranch_name: Option<String>) -> CommitHeadersV2 {
        CommitHeadersV2 {
            is_unapplied_header_commit,
            vbranch_name,
            ..Default::default()
        }
    }

    pub fn inject_default(commit_buffer: &mut CommitBuffer) {
        CommitHeadersV2::default().inject_into(commit_buffer)
    }

    pub fn inject_into(&self, commit_buffer: &mut CommitBuffer) {
        commit_buffer.set_header(HEADERS_VERSION_HEADER, V2_HEADERS_VERSION);
        commit_buffer.set_header(V2_CHANGE_ID_HEADER, &self.change_id);
        let is_unapplied_header_commit = if self.is_unapplied_header_commit {
            "true"
        } else {
            "false"
        };
        commit_buffer.set_header(
            V2_IS_UNAPPLIED_HEADER_COMMIT_HEADER,
            is_unapplied_header_commit,
        );

        if let Some(vbranch_name) = &self.vbranch_name {
            commit_buffer.set_header(V2_VBRANCH_NAME_HEADER, vbranch_name);
        };
    }
}

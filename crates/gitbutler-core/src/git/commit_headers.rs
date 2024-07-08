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
#[derive(Debug, Clone)]
pub struct CommitHeadersV2 {
    pub change_id: String,
}

impl Default for CommitHeadersV2 {
    fn default() -> Self {
        CommitHeadersV2 {
            // Change ID using base16 encoding
            change_id: Uuid::new_v4().to_string(),
        }
    }
}

impl From<CommitHeadersV1> for CommitHeadersV2 {
    fn from(commit_headers_v1: CommitHeadersV1) -> CommitHeadersV2 {
        CommitHeadersV2 {
            change_id: commit_headers_v1.change_id,
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

                Some(CommitHeadersV2 { change_id })
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
    pub fn new() -> CommitHeadersV2 {
        CommitHeadersV2 {
            ..Default::default()
        }
    }

    pub fn inject_default(commit_buffer: &mut CommitBuffer) {
        CommitHeadersV2::default().inject_into(commit_buffer)
    }

    pub fn inject_into(&self, commit_buffer: &mut CommitBuffer) {
        commit_buffer.set_header(HEADERS_VERSION_HEADER, V2_HEADERS_VERSION);
        commit_buffer.set_header(V2_CHANGE_ID_HEADER, &self.change_id);
    }
}

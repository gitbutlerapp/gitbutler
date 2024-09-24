use bstr::{BStr, BString};
use uuid::Uuid;

/// Header used to determine which version of the headers is in use. This should never be changed
const HEADERS_VERSION_HEADER: &str = "gitbutler-headers-version";

const V1_CHANGE_ID_HEADER: &str = "change-id";

/// Used to represent the old commit headers layout. This should not be used in new code
#[derive(Debug)]
struct CommitHeadersV1 {
    /// A property we can use to determine if two different commits are
    /// actually the same "patch" at different points in time. We carry it
    /// forwards when you rebase a commit in GitButler.
    change_id: String,
}

/// The version number used to represent the V2 headers
const V2_HEADERS_VERSION: &str = "2";

const V2_CHANGE_ID_HEADER: &str = "gitbutler-change-id";
const V2_CONFLICTED_HEADER: &str = "gitbutler-conflicted";
#[derive(Debug, Clone)]
pub struct CommitHeadersV2 {
    /// A property we can use to determine if two different commits are
    /// actually the same "patch" at different points in time. We carry it
    /// forwards when you rebase a commit in GitButler.
    pub change_id: String,
    /// A property used to indicate that we've written a conflicted tree to a
    /// commit. This is only written if the property is present. Conflicted
    /// commits should never make it into the main trunk.
    pub conflicted: Option<u64>,
}

impl Default for CommitHeadersV2 {
    fn default() -> Self {
        CommitHeadersV2 {
            // Change ID using base16 encoding
            change_id: Uuid::new_v4().to_string(),
            conflicted: None,
        }
    }
}

impl From<CommitHeadersV1> for CommitHeadersV2 {
    fn from(commit_headers_v1: CommitHeadersV1) -> CommitHeadersV2 {
        CommitHeadersV2 {
            change_id: commit_headers_v1.change_id,
            conflicted: None,
        }
    }
}

impl From<CommitHeadersV2> for Vec<(BString, BString)> {
    fn from(hdr: CommitHeadersV2) -> Self {
        let mut out = vec![
            (
                BString::from(HEADERS_VERSION_HEADER),
                BString::from(V2_HEADERS_VERSION),
            ),
            (V2_CHANGE_ID_HEADER.into(), hdr.change_id.clone().into()),
        ];

        if let Some(conflicted) = hdr.conflicted {
            out.push((V2_CONFLICTED_HEADER.into(), conflicted.to_string().into()));
        }
        out
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

                let conflicted = match self.header_field_bytes(V2_CONFLICTED_HEADER) {
                    Ok(value) => {
                        let value = value.as_str()?;

                        value.parse::<u64>().ok()
                    }
                    Err(_) => None,
                };

                Some(CommitHeadersV2 {
                    change_id,
                    conflicted,
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

/// Lifecycle
impl CommitHeadersV2 {
    /// Used to create a CommitHeadersV2. This does not allow a change_id to be
    /// provided in order to ensure a consistent format.
    pub fn new() -> CommitHeadersV2 {
        CommitHeadersV2 {
            ..Default::default()
        }
    }
}

//! General types for teh APIs

use but_workspace::RefInfo;

/// Represents the workspace for the frontend
///
/// Currently this is a thin wrapper around [`RefInfo`], but by having this
/// structure, we can add more data like graph information in a backwards
/// compatible way.
pub struct WorkspaceState {
    /// The workspace presented for the frontend. See [`RefInfo`] for more
    /// detail.
    pub head_info: RefInfo,
}

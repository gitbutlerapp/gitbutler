/// A way to mark errors using `[anyhow::Context::context]` for later retrieval, e.g. to know
/// that a certain even happened.
///
/// Note that the display implementation is visible to users in logs, so it's a bit 'special'
/// to signify its marker status.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Marker {
    /// Invalid state was detected, making the repository invalid for operation.
    VerificationFailure,
    /// An indicator for a conflict in the project.
    ///
    /// See usages for details on what these conflicts can be.
    ProjectConflict,
}

impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Marker::VerificationFailure => f.write_str("<verification-failed>"),
            Marker::ProjectConflict => f.write_str("<project-conflict>"),
        }
    }
}

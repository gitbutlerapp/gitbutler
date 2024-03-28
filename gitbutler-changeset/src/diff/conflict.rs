use crate::{LineSpan, RawHunk};

/// Represents a conflict between two hunks.
///
/// File information is not included in this struct,
/// and is typically attached alongside conflict objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict<'a, 'b, H: RawHunk> {
    /// If `Some`, contains the [`LineSpan`] of the removals that conflict
    /// between the two hunks.
    pub removals: Option<(LineSpan, &'a H, &'b H)>,
    /// If `Some`, contains the [`LineSpan`] of the additions that conflict
    /// between the two hunks.
    pub additions: Option<(LineSpan, &'a H, &'b H)>,
}

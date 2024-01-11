use crate::Hunk;

/// Changesets are collections of hunks whereby
/// quite a few operations can be performed on a
/// file.
#[derive(Debug, Clone, Default)]
pub struct Changeset {
    // NOTE: Must be sorted at all times; the implementation assumes this!
    // NOTE: Inserts must perform sorted inserts based on source line number.
    hunks: Vec<Hunk>,
}

/// Denotes that a hunk could not be added to a changeset
/// due to a conflict in the source range.
#[derive(Debug, thiserror::Error)]
#[error("hunk conflicts with existing hunk (new hunk: {:?}, existing hunk: {:?})", .new.source_span(), .existing.source_span())]
pub struct HunkConflict {
    /// The hunk that could not be added.
    pub new: Hunk,
    /// The existing hunk that conflicts with the new hunk.
    pub existing: Hunk,
}

impl Changeset {
    /// Attempts to add the given hunk to the changeset.
    /// Errors if there's a conflict.
    pub fn try_add(&mut self, hunk: Hunk) -> Result<(), HunkConflict> {
        // Find the index of the first hunk that starts after the given hunk.
        // We do this with a binary search based on the hunk source range's start.
        let insert_index = match self
            .hunks
            .binary_search_by_key(&hunk.source_span().start(), |h| h.source_span().start())
        {
            Ok(index) => index,
            Err(index) => index,
        };

        // Check the hunk before the insert index, if there is one.
        // If it conflicts, error.
        if insert_index > 0 {
            if let Some(existing) = self.hunks.get(insert_index - 1) {
                if existing.source_span().intersects(&hunk.source_span()) {
                    return Err(HunkConflict {
                        new: hunk,
                        existing: existing.clone(),
                    });
                }
            }
        }

        // Check the hunk after the insert index, if there is one.
        // If it conflicts, error.
        if let Some(existing) = self.hunks.get(insert_index) {
            if existing.source_span().intersects(&hunk.source_span()) {
                return Err(HunkConflict {
                    new: hunk,
                    existing: existing.clone(),
                });
            }
        }

        // No conflicts, insert the hunk.
        self.hunks.insert(insert_index, hunk);

        Ok(())
    }

    /// Returns an iterator over all hunks.
    pub fn hunks(&self) -> impl Iterator<Item = &Hunk> {
        self.hunks.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineSpan;

    #[test]
    fn test_add() {
        let mut changeset = Changeset::default();

        // Insert hunks that don't conflict.
        changeset
            .try_add(Hunk::new(LineSpan::new(0, 0), "Hello, world!".to_string()))
            .unwrap();
        changeset
            .try_add(Hunk::new(LineSpan::new(2, 2), "Hello, world!".to_string()))
            .unwrap();
        changeset
            .try_add(Hunk::new(LineSpan::new(4, 4), "Hello, world!".to_string()))
            .unwrap();
        changeset
            .try_add(Hunk::new(LineSpan::new(1, 1), "Hello, world!".to_string()))
            .unwrap();

        // Insert a hunk that conflicts with the first hunk.
        assert!(matches!(
            changeset.try_add(Hunk::new(LineSpan::new(0, 0), "Hello, world!".to_string())),
            Err(HunkConflict { .. })
        ));

        // Insert a hunk that conflicts with the second hunk.
        assert!(matches!(
            changeset.try_add(Hunk::new(LineSpan::new(2, 3), "Hello, world!".to_string())),
            Err(HunkConflict { .. })
        ));

        // Insert a hunk that conflicts with the third hunk.
        assert!(matches!(
            changeset.try_add(Hunk::new(LineSpan::new(3, 4), "Hello, world!".to_string())),
            Err(HunkConflict { .. })
        ));

        // Insert a hunk that conflicts with the first and second hunks.
        assert!(matches!(
            changeset.try_add(Hunk::new(LineSpan::new(0, 2), "Hello, world!".to_string())),
            Err(HunkConflict { .. })
        ));

        // Insert a hunk that conflicts with the second and third hunks.
        assert!(matches!(
            changeset.try_add(Hunk::new(LineSpan::new(2, 4), "Hello, world!".to_string())),
            Err(HunkConflict { .. })
        ));

        // Insert a hunk that conflicts with the first and third hunks.
        assert!(matches!(
            changeset.try_add(Hunk::new(LineSpan::new(0, 4), "Hello, world!".to_string())),
            Err(HunkConflict { .. })
        ));

        // Insert a hunk that conflicts with all hunks.
        assert!(matches!(
            changeset.try_add(Hunk::new(
                LineSpan::new(0, 4),
                "Hello, world!\nHello, world!\nHello, world!\nHello, world!".to_string()
            )),
            Err(HunkConflict { .. })
        ));

        // Insert a few more valid hunks, some of which are in the middle of the changeset.
        changeset
            .try_add(Hunk::new(LineSpan::new(7, 9), "Hello, world!".to_string()))
            .unwrap();
        changeset
            .try_add(Hunk::new(LineSpan::new(3, 3), "Hello, world!".to_string()))
            .unwrap();
        changeset
            .try_add(Hunk::new(LineSpan::new(5, 6), "Hello, world!".to_string()))
            .unwrap();

        // Assert the hunks are sorted by their start line.
        let mut hunks = changeset.hunks().peekable();
        while let Some(hunk) = hunks.next() {
            if let Some(next) = hunks.peek() {
                assert!(hunk.source_span().start() < next.source_span().start());
            }
        }
    }
}

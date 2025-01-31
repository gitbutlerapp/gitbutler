/// workspace-independent queries, with bare Git.
mod query_workspace_ranges;
/// Turn real workspaces into dependency queries.
mod workspace_dependencies;

mod ui;

fn intersect_workspace_ranges(
    repo: &gix::Repository,
    ranges: but_hunk_dependency::WorkspaceRanges,
    worktree_changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<WorkspaceDigest> {
    let mut intersections_by_path = Vec::new();
    let mut missed_hunks = Vec::new();
    for change in worktree_changes {
        let unidiff = change.unified_diff(repo, 0)?;
        let but_core::UnifiedDiff::Patch { hunks } = unidiff else {
            continue;
        };
        let mut intersections = Vec::new();
        for hunk in hunks {
            if let Some(hunk_ranges) =
                ranges.intersection(&change.path, hunk.old_start, hunk.old_lines)
            {
                let hunk_ranges: Vec<_> =
                    hunk_ranges.into_iter().copied().map(Into::into).collect();
                intersections.push(HunkIntersection {
                    hunk,
                    commit_intersections: hunk_ranges,
                });
            } else {
                missed_hunks.push((change.path.clone(), hunk));
            }
        }
        if !intersections.is_empty() {
            intersections_by_path.push((change.path, intersections));
        }
    }

    Ok(WorkspaceDigest {
        intersections_by_path,
        missed_hunks,
        ranges_by_path: ranges
            .ranges_by_path_map()
            .iter()
            .sorted_by(|a, b| a.0.cmp(b.0))
            .map(|(path, ranges)| {
                (
                    path.to_owned(),
                    ranges.iter().map(|hr| (*hr).into()).collect(),
                )
            })
            .collect(),
    })
}

mod types {
    use but_core::TreeStatusKind;
    use but_hunk_dependency::HunkRange;
    use gix::bstr::BString;

    /// A structure that has stable content so it can be asserted on, showing the hunk-ranges that intersect with each of the input ranges.
    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct WorkspaceDigest {
        /// All available ranges for a tracked path, basically all changes seen over a set of commits.
        pub ranges_by_path: Vec<(BString, Vec<StableHunkRange>)>,
        /// The ranges that intersected with an input hunk.
        pub intersections_by_path: Vec<(BString, Vec<HunkIntersection>)>,
        /// Hunks that didn't have a matching intersection, with the filepath mentioned per hunk as well.
        pub missed_hunks: Vec<(BString, but_core::unified_diff::DiffHunk)>,
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct HunkIntersection {
        /// The hunk that was used for the intersection.
        pub hunk: but_core::unified_diff::DiffHunk,
        /// The hunks that touch `hunk` in the commit-diffs.
        pub commit_intersections: Vec<StableHunkRange>,
    }

    /// A structure that has stable content so it can be asserted on, showing the hunk-ranges that intersect with each of the input ranges.
    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct WorkspaceWithoutRanges<'a> {
        /// The ranges that intersected with an input hunk.
        pub intersections_by_path: &'a Vec<(BString, Vec<HunkIntersection>)>,
        /// Hunks that didn't have a matching intersection, with the filepath mentioned per hunk as well.
        pub missed_hunks: &'a Vec<(BString, but_core::unified_diff::DiffHunk)>,
    }

    impl WorkspaceDigest {
        /// Drop some information that might be overcall in some situations.
        pub fn partial(&self) -> WorkspaceWithoutRanges<'_> {
            WorkspaceWithoutRanges {
                intersections_by_path: &self.intersections_by_path,
                missed_hunks: &self.missed_hunks,
            }
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct StableHunkRange {
        change_type: TreeStatusKind,
        commit_id: gix::ObjectId,
        start: u32,
        lines: u32,
        line_shift: i32,
    }

    impl From<HunkRange> for StableHunkRange {
        fn from(
            HunkRange {
                change_type,
                stack_id: _,
                commit_id,
                start,
                lines,
                line_shift,
            }: HunkRange,
        ) -> Self {
            StableHunkRange {
                change_type,
                commit_id,
                start,
                lines,
                line_shift,
            }
        }
    }
}

use itertools::Itertools;
pub use types::{HunkIntersection, WorkspaceDigest};

//! The oracle for the direct (segmentless) step-graph derivation: an editor created from a
//! directly-projected workspace must build a step graph that is canonically identical to the
//! one built from the segment graph — same picks with the same ordered parents, the same
//! references resolving onto the same picks, and the same head.
//!
//! Canonical form ignores node indices, and it sorts references that sit on the same commit.
//! Note that same-commit reference order does have insertion semantics — the carrier mirrors
//! production's splice order for metadata branches — but the two derivations are allowed to
//! differ for refs outside any metadata stack, where production order is incidental.

use anyhow::Result;
use but_rebase::graph_rebase::{Editor, testing::Testing as _};

use crate::utils::{fixture, standard_options};

#[test]
fn direct_step_graph_matches_segment_derived_step_graph() -> Result<()> {
    let scenarios = [
        "single-commit",
        "four-commits",
        "four-commits-one-file",
        "two-branches-shared-bottom-two",
        "three-branches-three-commits",
        "three-branches-merged",
        "merge-in-the-middle",
        "merge-with-two-children",
        "merge-first-parent-older",
        "first-parent-leg-long",
        "second-parent-leg-long",
        "octopus-merge-with-redundant-input",
        "many-references",
        "workspace-with-three-empty-stacks",
        "disjoint-orphan-branches",
        "cherry-pick",
    ];
    for scenario in scenarios {
        let (repo, mut meta) = fixture(scenario)?;
        let project_meta = but_core::ref_metadata::ProjectMeta::default();

        let mut production_ws = but_graph::Workspace::from_head(
            &repo,
            &*meta,
            project_meta.clone(),
            standard_options(),
        )?;
        let production = {
            let editor = Editor::create(&mut production_ws, &mut *meta, &repo)?;
            editor.steps_canonical()
        };

        let mut direct_ws =
            but_graph::Workspace::from_head(&repo, &*meta, project_meta, standard_options())?;
        assert!(
            direct_ws.branches().is_some(),
            "{scenario}: the direct workspace must carry branch records"
        );
        let direct = {
            let editor = Editor::create(&mut direct_ws, &mut *meta, &repo)?;
            editor.steps_canonical()
        };

        assert_eq!(
            direct, production,
            "{scenario}: direct step graph diverges from the segment-derived one"
        );
    }
    Ok(())
}

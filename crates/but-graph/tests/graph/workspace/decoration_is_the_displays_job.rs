//! The derivation decides STRUCTURE; which refs a commit shows is decided when the display
//! materializes. Both layers used to decorate, and because every caller of the derivation reduces
//! it to commit ids before anyone can observe it, the engine's copy was invisible — so the two
//! implementations drifted apart unnoticed (they disagreed on when a detached checkout's own ref
//! is reordered). The duplicate is gone; this pins the split so a second one cannot creep back
//! without a test going red.

use but_graph::Workspace;

use super::target_meta;
use crate::walk::utils::{add_workspace, read_only_in_memory_scenario, standard_options};

#[test]
fn the_derivation_leaves_ref_decoration_to_the_display() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    add_workspace(&mut meta);
    let ws = Workspace::from_head(
        &repo,
        &*meta,
        target_meta(),
        &mut but_testsupport::in_memory_db(),
        standard_options(),
    )?
    .validated()?;

    // A segment's own naming ref is STRUCTURE — the segment already says it, so the display
    // strips it off the commit rather than showing it twice.
    let shows_own_name = |stacks: &[but_graph::workspace::Stack]| {
        stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .filter_map(|seg| Some((seg.ref_name()?, seg.commits.first()?)))
            .any(|(name, tip)| tip.refs.iter().any(|ri| ri.ref_name.as_ref() == name))
    };

    assert!(
        shows_own_name(&ws.derive_stacks()),
        "the derivation must leave the graph's refs alone — if this fails, either the scenario \
         stopped putting a branch on its own segment's tip, or decoration moved back into the \
         engine, which is what created two drifting implementations before"
    );
    assert!(
        !shows_own_name(&ws.display_stacks()?),
        "the display is what strips a segment's own name from its commit"
    );
    Ok(())
}

use but_graph::Graph;

use crate::init::{
    StackState, add_stack_with_segments,
    utils::{default_project_meta, read_only_in_memory_scenario, standard_options},
};

#[test]
fn worktree_segments_are_found_alongside_stack_segments() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/worktree-ref-mid-stack")?;
    add_stack_with_segments(&mut meta, 0, "foo", StackState::InWorkspace, &[]);
    meta.worktree_meta_mut().mark_adopted()?;
    let ws = Graph::from_head(
        &repo,
        default_project_meta(&repo),
        &mut meta.connection_mut(),
        but_graph::init::Options {
            worktrees: true,
            ..standard_options()
        },
    )?
    .validated()?
    .into_workspace()?;

    let foo: gix::refs::FullName = "refs/heads/foo".try_into()?;
    let wsref: gix::refs::FullName = "refs/heads/wsref".try_into()?;
    let main: gix::refs::FullName = "refs/heads/main".try_into()?;

    assert_eq!(
        ws.find_segment_by_refname(foo.as_ref())
            .and_then(|segment| segment.ref_name()),
        Some(foo.as_ref()),
        "stack segments are found"
    );
    assert_eq!(
        ws.find_segment_by_refname(wsref.as_ref())
            .and_then(|segment| segment.ref_name()),
        Some(wsref.as_ref()),
        "the worktree's lane owns its checked-out branch"
    );
    assert!(
        ws.find_segment_and_stack_by_refname(wsref.as_ref())
            .is_none(),
        "no stack owns the worktree's branch"
    );
    assert!(
        ws.refname_is_segment(wsref.as_ref()),
        "a branch in a worktree lane is applied"
    );
    assert!(
        ws.find_segment_by_refname(main.as_ref()).is_none(),
        "the target branch is in no lane"
    );
    Ok(())
}

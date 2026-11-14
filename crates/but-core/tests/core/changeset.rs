use but_core::changeset::{Identifier, create_similarity_lut};
use gix::prelude::ObjectIdExt as _;

#[test]
fn similarity_lut_is_the_same_on_one_thread_and_many() -> anyhow::Result<()> {
    let repo =
        but_testsupport::read_only_in_memory_scenario("merge-with-two-branches-line-offset")?;
    let tip = repo.rev_parse_single("merge")?.detach();
    let commits = repo
        .rev_walk([tip])
        .all()?
        .map(|info| but_core::Commit::from_id(info?.id.attach(&repo)))
        .collect::<anyhow::Result<Vec<_>>>()?;

    let one_thread = create_similarity_lut(&repo, commits.iter(), (1, 1), true)?;
    let changesets = one_thread
        .keys()
        .filter(|id| matches!(id, Identifier::ChangesetId(_)))
        .count();
    assert_eq!(
        changesets,
        commits.len(),
        "every commit, the merge too, changes the file in its own way"
    );

    // A cost this high spreads the diffs over one thread per core.
    let many_threads = create_similarity_lut(&repo, commits.iter(), (1 << 20, 1 << 20), true)?;
    assert_eq!(
        many_threads, one_thread,
        "the threads find the same changesets as one thread does"
    );
    Ok(())
}

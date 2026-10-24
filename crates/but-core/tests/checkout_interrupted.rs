//! Checkout when gix's interrupt flag is set. The flag is global to the process, so this test
//! runs in a binary of its own, where setting it can't interrupt other tests.

use std::sync::atomic::Ordering;

use but_core::worktree::safe_checkout_from_head;
use but_testsupport::{invoke_bash_at_dir, writable_scenario};

#[test]
fn an_interrupted_checkout_keeps_head_the_index_and_the_files_it_never_wrote() -> anyhow::Result<()>
{
    let (repo, _tmp) = writable_scenario("worktree-deletion-the-target-also-makes");
    let workdir = repo.workdir().expect("non-bare").to_owned();
    invoke_bash_at_dir(
        r#"set -eu
export GIT_INDEX_FILE="$PWD/.git/next-index"
git read-tree HEAD
git update-index --cacheinfo 100644,$(printf 'changed\n' | git hash-object -w --stdin),keep
git commit-tree $(git write-tree) -p HEAD -m 'change keep' >.git/next-commit
rm "$GIT_INDEX_FILE"
"#,
        &workdir,
    );
    let new_commit = gix::ObjectId::from_hex(
        std::fs::read_to_string(workdir.join(".git/next-commit"))?
            .trim()
            .as_bytes(),
    )?;
    let head = repo.head_id()?.detach();
    let index = std::fs::read(repo.index_path())?;

    gix::interrupt::IS_INTERRUPTED.store(true, Ordering::SeqCst);
    let outcome = safe_checkout_from_head(new_commit, &repo, Default::default());
    gix::interrupt::IS_INTERRUPTED.store(false, Ordering::SeqCst);

    outcome.expect_err("an interrupted checkout fails");
    assert_eq!(repo.head_id()?.detach(), head, "HEAD stays");
    assert_eq!(std::fs::read(repo.index_path())?, index, "the index stays");
    assert_eq!(
        std::fs::read_to_string(workdir.join("keep"))?,
        "keep\n",
        "a file checkout never wrote keeps its content"
    );
    Ok(())
}

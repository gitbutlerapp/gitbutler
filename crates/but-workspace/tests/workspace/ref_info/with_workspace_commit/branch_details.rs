use but_testsupport::visualize_commit_graph_all;
use but_workspace::branch_details;

use crate::ref_info::with_workspace_commit::read_only_in_memory_scenario;

#[test]
fn disjoint() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("disjoint")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 32791d2 (HEAD -> disjoint) disjoint init
    * fafd9d0 (origin/main, main) init
    ");

    let actual = branch_details(&repo, "refs/heads/disjoint".try_into()?, &*meta)?;
    insta::assert_debug_snapshot!(actual, @r#"
    BranchDetails {
        name: "refs/heads/disjoint",
        linked_worktree_id: None,
        remote_tracking_branch: None,
        description: None,
        pr_number: None,
        review_id: None,
        tip: Sha1(32791d22e276ec0ed87d14f906321137356bc6d6),
        base_commit: Sha1(32791d22e276ec0ed87d14f906321137356bc6d6),
        push_status: CompletelyUnpushed,
        last_updated_at: None,
        authors: [
            author <author@example.com>,
            committer <committer@example.com>,
        ],
        is_conflicted: false,
        commits: [
            Commit(32791d2, "disjoint init", local/remote(identity)),
        ],
        upstream_commits: [],
        is_remote_head: false,
    }
    "#);

    let actual = branch_details(&repo, "refs/heads/main".try_into()?, &*meta)?;
    insta::assert_debug_snapshot!(actual, @r#"
    BranchDetails {
        name: "refs/heads/main",
        linked_worktree_id: None,
        remote_tracking_branch: None,
        description: None,
        pr_number: None,
        review_id: None,
        tip: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
        base_commit: Sha1(fafd9d08a839d99db60b222cd58e2e0bfaf1f7b2),
        push_status: CompletelyUnpushed,
        last_updated_at: None,
        authors: [],
        is_conflicted: false,
        commits: [],
        upstream_commits: [],
        is_remote_head: false,
    }
    "#);

    Ok(())
}

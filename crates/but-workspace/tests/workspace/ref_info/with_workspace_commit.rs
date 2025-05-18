use but_workspace::{ref_info, ref_info_at};
use pretty_assertions::assert_eq;

#[test]
fn remote_ahead_fast_forwardable() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("remote-advanced-ff")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * fb27086 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    | * 89cc2d3 (origin/A) change in A
    |/  
    * d79bba9 (A) new file in A
    * c166d42 (origin/main, origin/HEAD, main) init-integration
    ");

    // Needs a branch for workspace to 'fake-exists'.
    add_workspace(&mut meta);
    // We can look at a workspace ref directly (via HEAD)
    let info = ref_info(
        &repo,
        &*meta,
        ref_info::Options {
            stack_commit_limit: 0,
            expensive_commit_info: true,
        },
    )?;
    insta::assert_debug_snapshot!(info, @r#"
    RefInfo {
        workspace_ref_name: Some(
            FullName(
                "refs/heads/gitbutler/workspace",
            ),
        ),
        stacks: [
            Stack {
                index: 0,
                tip: Some(
                    Sha1(d79bba960b112dbd25d45921c47eeda22288022b),
                ),
                base: Some(
                    Sha1(c166d42d4ef2e5e742d33554d03805cfb0b24d11),
                ),
                segments: [
                    StackSegment {
                        ref_name: "refs/heads/A",
                        remote_tracking_ref_name: "None",
                        ref_location: "ReachableFromWorkspaceCommit",
                        commits_unique_from_tip: [
                            LocalCommit(d79bba9, "new file in A\n", local/remote(identity)),
                        ],
                        commits_unique_in_remote_tracking_branch: [
                            RemoteCommit(89cc2d3, "change in A\n",
                        ],
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: Some(
            FullName(
                "refs/remotes/origin/main",
            ),
        ),
    }
    "#);

    let expected_info = info;

    let at = repo.find_reference("refs/heads/A")?;
    let info = ref_info_at(
        at,
        &*meta,
        ref_info::Options {
            stack_commit_limit: 0,
            expensive_commit_info: true,
        },
    )?;
    assert_eq!(
        info, expected_info,
        "Information doesn't change just because the starting point is different"
    );
    Ok(())
}

mod utils {
    use crate::ref_info::utils::named_read_only_in_memory_scenario;
    use but_workspace::VirtualBranchesTomlMetadata;

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let (repo, mut meta) =
            named_read_only_in_memory_scenario("with-remotes-and-workspace", name)?;
        let vb = meta.data_mut();
        vb.default_target = Some(gitbutler_stack::Target {
            // For simplicity, we stick to the defaults.
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            // Not required
            remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
            sha: git2::Oid::zero(),
            push_remote_name: None,
        });
        Ok((repo, meta))
    }

    // Add parameters as needed.
    pub fn add_workspace(meta: &mut VirtualBranchesTomlMetadata) {
        meta.data_mut().branches.insert(
            but_workspace::StackId::generate(),
            gitbutler_stack::Stack::new_with_just_heads(
                vec![gitbutler_stack::StackBranch::new_with_zero_head(
                    "name is not important, needs branch for workspace to exist".into(),
                    None,
                    None,
                    None,
                    true,
                )],
                0,
                0,
                false,
            ),
        );
    }
}

use crate::ref_info::with_workspace_commit::utils::add_workspace;
use but_testsupport::visualize_commit_graph_all;
use utils::read_only_in_memory_scenario;

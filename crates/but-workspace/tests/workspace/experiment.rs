use anyhow::Result;
use but_graph::init::Options;
use but_meta::virtual_branches_legacy_types::Target;
use but_testsupport::visualize_commit_graph_all;
use but_workspace::experiment::{RowData, render_stacks};
use gitbutler_commit::commit_ext::CommitMessageBstr as _;
use renderdag::GraphRowRenderer;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, named_writable_scenario_with_description,
};

#[test]
fn diamond_shape() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("diamond-partially-historically-integrated")?;
    let o1_id = repo.rev_parse_single("o1")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "master"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: o1_id,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "E", StackState::InWorkspace);
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        Options {
            extra_target_commit_id: Some(o1_id),
            ..Options::limited()
        },
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 61ee5f5 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 972cf74 (E) E
    *   9e74c75 (C) C
    |\  
    | * d6a7004 (D) D
    | | * 7de2393 (origin/master, master) o4
    | | *   7d62953 (o3) o3
    | | |\  
    | |_|/  
    |/| |   
    * | | ffb801b (B) B
    |/ /  
    * | 448b195 (A) A
    | * d1b2089 o2
    |/  
    * 85aa44b (o1) o1
    ");

    let stacks = render_stacks(
        &graph.into_workspace()?,
        || GraphRowRenderer::new().output().build_box_drawing(),
        |row_data| {
            Ok(match &row_data {
                RowData::Commit(commit) => repo.find_commit(commit.id)?.message_bstr().to_string(),
                RowData::Reference(ref_name) => ref_name.to_string(),
            })
        },
    )?;

    insta::assert_snapshot!(stacks[0].join(""), @"
    ◎  refs/heads/E
    │
    ●  E
    │
    ◎  refs/heads/C
    │
    ●    C
    ├─╮
    ◎ │  refs/heads/B
    │ │
    ● │  B
    │ │
    │ ◎  refs/heads/D
    │ │
    │ ●  D
    ├─╯
    ◎  refs/heads/A
    │
    ●  A
    │
    ~
    ");

    Ok(())
}

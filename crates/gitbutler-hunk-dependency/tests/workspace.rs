use std::{path::PathBuf, str::FromStr};

use gitbutler_hunk_dependency::{
    diff::InputDiff,
    workspace::{InputCommit, InputFile, InputStack, WorkspaceHunkRanges},
};
use gitbutler_stack::StackId;

#[test]
fn builder_simple() -> anyhow::Result<()> {
    let commit1_id = git2::Oid::from_str("a")?;
    let stack1_id = StackId::generate();
    let path = PathBuf::from_str("/test.txt")?;
    let commit2_id = git2::Oid::from_str("b")?;
    let stack2_id = StackId::generate();

    let deps = WorkspaceHunkRanges::new(vec![
        InputStack {
            stack_id: stack1_id,
            commits: vec![InputCommit {
                commit_id: commit1_id,
                files: vec![InputFile {
                    path: path.to_owned(),
                    diffs: vec![InputDiff::try_from(
                        "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
                    )?],
                }],
            }],
        },
        InputStack {
            stack_id: stack2_id,
            commits: vec![InputCommit {
                commit_id: commit2_id,
                files: vec![InputFile {
                    path: path.to_owned(),
                    diffs: vec![InputDiff::try_from(
                        "@@ -1,5 +1,3 @@
-1
-2
3
5
6
",
                    )?],
                }],
            }],
        },
    ]);

    let lookup = deps.intersection(&path, 2, 1);
    assert_eq!(lookup.len(), 1);
    assert_eq!(lookup[0].commit_id, commit1_id);
    Ok(())
}

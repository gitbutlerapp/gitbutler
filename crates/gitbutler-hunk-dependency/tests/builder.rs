use std::{path::PathBuf, str::FromStr};

use gitbutler_hunk_dependency::{builder::HunkDependencyBuilder, diff::Diff};
use gitbutler_stack::StackId;

#[test]
fn builder_simple() -> anyhow::Result<()> {
    let path = PathBuf::from_str("/test.txt")?;
    let mut builder = HunkDependencyBuilder::default();

    let commit1_id = git2::Oid::from_str("a")?;
    let stack1_id = StackId::generate();
    builder.add(
        stack1_id,
        commit1_id,
        &path,
        vec![Diff::try_from(
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
    )?;

    let commit2_id = git2::Oid::from_str("b")?;
    let stack2_id = StackId::generate();

    builder.add(
        stack2_id,
        commit2_id,
        &path,
        vec![Diff::try_from(
            "@@ -1,5 +1,3 @@
-1
-2
3
5
6
",
        )?],
    )?;

    let path_deps = builder.get_path(&path).unwrap();
    let lookup = path_deps.find(2, 1);
    assert_eq!(lookup.len(), 1);
    assert_eq!(lookup[0].commit_id, commit1_id);
    Ok(())
}

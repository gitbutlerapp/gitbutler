use gitbutler_hunk_dependency::diff::InputDiff;

#[test]
fn diff_simple() -> anyhow::Result<()> {
    let header = InputDiff::try_from(
        "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
    )?;
    assert_eq!(header.old_start, 4);
    assert_eq!(header.old_lines, 0);
    assert_eq!(header.new_start, 4);
    assert_eq!(header.new_lines, 1);
    Ok(())
}

#[test]
fn diff_complex() -> anyhow::Result<()> {
    let header = InputDiff::try_from(
        "@@ -5,7 +5,6 @@
5
6
7
-8
-9
+a
10
11
",
    )?;
    assert_eq!(header.old_start, 8);
    assert_eq!(header.old_lines, 2);
    assert_eq!(header.new_start, 8);
    assert_eq!(header.new_lines, 1);
    Ok(())
}

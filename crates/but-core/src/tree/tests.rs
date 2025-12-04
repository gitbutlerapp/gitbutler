mod to_additive_hunks {
    use crate::{tree::to_additive_hunks, utils::hunk_header};

    #[test]
    fn rejected() {
        let with = vec![hunk_header("-1,10", "+1,10")];
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                // rejected as old is out of bounds
                hunk_header("-20,1", "+1,10"),
                // rejected as new is out of bounds
                hunk_header("+1,10", "+20,1"),
                // rejected as it doesn't match any anchor point, nor does it match hunks without context
                hunk_header("-0,0", "+2,10")
            ],
            &with,
            &with,
        ).unwrap(), @r#"
        (
            [],
            [
                HunkHeader("-20,1", "+1,10"),
                HunkHeader("-1,10", "+20,1"),
                HunkHeader("-0,0", "+2,10"),
            ],
        )
        "#);
    }

    #[test]
    fn only_selections() {
        let with = vec![hunk_header("-1,10", "+1,10")];
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                hunk_header("-1,1", "+0,0"),
                hunk_header("-5,2", "+0,0"),
                hunk_header("-10,1", "+0,0")
            ],
            &with,
            &with,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-1,1", "+1,0"),
                HunkHeader("-5,2", "+1,0"),
                HunkHeader("-10,1", "+1,0"),
            ],
            [],
        )
        "#);
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                hunk_header("-0,0", "+1,1"),
                hunk_header("-0,0", "+5,2"),
                hunk_header("-0,0", "+10,1")
            ],
            &with,
            &with,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-1,0", "+1,1"),
                HunkHeader("-1,0", "+5,2"),
                HunkHeader("-1,0", "+10,1"),
            ],
            [],
        )
        "#);
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                hunk_header("-0,0", "+1,1"),
                hunk_header("-5,2", "+0,0"),
                hunk_header("-0,0", "+10,1")
            ],
            &with,
            &with,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-1,0", "+1,1"),
                HunkHeader("-5,2", "+2,0"),
                HunkHeader("-7,0", "+10,1"),
            ],
            [],
        )
        "#);
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                hunk_header("-1,1", "+0,0"),
                hunk_header("-0,0", "+5,2"),
                hunk_header("-10,1", "+0,0")
            ],
            &with,
            &with,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-1,1", "+1,0"),
                HunkHeader("-2,0", "+5,2"),
                HunkHeader("-10,1", "+7,0"),
            ],
            [],
        )
        "#);
    }

    #[test]
    fn selections_and_full_hunks() {
        let with = vec![
            hunk_header("-1,10", "+1,10"),
            hunk_header("-15,5", "+20,5"),
            hunk_header("-25,5", "+40,5"),
        ];
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                // full match
                hunk_header("-1,10", "+1,10"),
                // partial match to same hunk
                hunk_header("-15,2", "+0,0"),
                hunk_header("-0,0", "+22,3"),
                // Last hunk isn't used
            ],
            &with,
            &with,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-1,10", "+1,10"),
                HunkHeader("-15,2", "+20,0"),
                HunkHeader("-17,0", "+22,3"),
            ],
            [],
        )
        "#);
    }

    #[test]
    fn only_full_hunks() {
        let with = vec![
            hunk_header("-1,10", "+1,10"),
            hunk_header("-15,5", "+20,5"),
            hunk_header("-25,5", "+40,5"),
        ];
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                // full match
                hunk_header("-1,10", "+1,10"),
                hunk_header("-15,5", "+20,5"),
                // Last hunk isn't used
            ],
            &with,
            &with,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-1,10", "+1,10"),
                HunkHeader("-15,5", "+20,5"),
            ],
            [],
        )
        "#);
    }

    #[test]
    fn worktree_hunks_without_context_lines() {
        // diff --git a/file b/file
        // index 190423f..b513cb5 100644
        // --- a/file
        // +++ b/file
        // @@ -93,8 +93,10 @@
        //  93
        //  94
        //  95
        // -96
        // +110
        // +111
        //  97
        // +95
        //  98
        //  99
        // -100
        // +119
        let with = vec![hunk_header("-93,8", "+93,10")];

        // diff --git a/file b/file
        // index 190423f..b513cb5 100644
        // --- a/file
        // +++ b/file
        // @@ -96 +96,2 @@
        // -96
        // +110
        // +111
        // @@ -97,0 +99 @@
        // +95
        // @@ -100 +102 @@
        // -100
        // +119
        let wth0 = vec![
            hunk_header("-96,1", "+96,2"),
            hunk_header("-98,0", "+99,1"),
            hunk_header("-100,1", "+102,1"),
        ];

        insta::assert_debug_snapshot!(to_additive_hunks(
            [hunk_header("-96,1", "+0,0")],
            &with,
            &wth0,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-96,1", "+96,0"),
            ],
            [],
        )
        "#);
        insta::assert_debug_snapshot!(to_additive_hunks(
            [hunk_header("-96,1", "+0,0"), hunk_header("-0,0", "+96,2")],
            &with,
            &wth0,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-96,1", "+96,0"),
                HunkHeader("-97,0", "+96,2"),
            ],
            [],
        )
        "#);
        insta::assert_debug_snapshot!(to_additive_hunks(
            [hunk_header("-0,0", "+96,2")],
            &with,
            &wth0,
        ).unwrap(), @r#"
        (
            [
                HunkHeader("-96,0", "+96,2"),
            ],
            [],
        )
        "#);
    }

    #[test]
    fn real_world_issue() {
        let with = vec![hunk_header("-1,214", "+1,55")];
        let wth0 = vec![
            hunk_header("-4,13", "+4,0"),
            hunk_header("-18,19", "+5,1"),
            hunk_header("-38,79", "+7,3"),
            hunk_header("-118,64", "+11,0"),
            hunk_header("-183,1", "+12,1"),
            hunk_header("-185,15", "+14,2"),
            hunk_header("-201,5", "+17,5"),
            hunk_header("-207,1", "+23,26"),
            hunk_header("-209,3", "+50,3"),
        ];

        let actual = to_additive_hunks(
            [
                hunk_header("-0,0", "+23,26"),
                hunk_header("-0,0", "+50,3"),
                hunk_header("-207,1", "+0,0"),
                hunk_header("-209,3", "+0,0"),
            ],
            &with,
            &wth0,
        )
        .unwrap();
        insta::assert_debug_snapshot!(actual, @r#"
        (
            [
                HunkHeader("-207,1", "+23,26"),
                HunkHeader("-209,3", "+50,3"),
            ],
            [],
        )
        "#);

        let actual = to_additive_hunks(
            [
                hunk_header("-0,0", "+23,1"),
                hunk_header("-0,0", "+25,1"),
                hunk_header("-0,0", "+27,2"),
                hunk_header("-0,0", "+30,2"),
                hunk_header("-0,0", "+50,3"),
                hunk_header("-207,1", "+0,0"),
                hunk_header("-209,1", "+0,0"),
                hunk_header("-211,1", "+0,0"),
            ],
            &with,
            &wth0,
        )
        .unwrap();
        insta::assert_debug_snapshot!(actual, @r#"
        (
            [
                HunkHeader("-207,1", "+23,1"),
                HunkHeader("-208,0", "+25,1"),
                HunkHeader("-208,0", "+27,2"),
                HunkHeader("-208,0", "+30,2"),
                HunkHeader("-209,1", "+50,3"),
                HunkHeader("-211,1", "+53,0"),
            ],
            [],
        )
        "#);

        let actual = to_additive_hunks(
            [
                hunk_header("-207,1", "+0,0"),
                hunk_header("-209,1", "+0,0"),
                hunk_header("-211,1", "+0,0"),
                hunk_header("-0,0", "+23,1"),
                hunk_header("-0,0", "+25,1"),
                hunk_header("-0,0", "+27,2"),
                hunk_header("-0,0", "+30,2"),
                hunk_header("-0,0", "+50,3"),
            ],
            &with,
            &wth0,
        )
        .unwrap();
        insta::assert_debug_snapshot!(actual, @r#"
        (
            [
                HunkHeader("-207,1", "+23,1"),
                HunkHeader("-208,0", "+25,1"),
                HunkHeader("-208,0", "+27,2"),
                HunkHeader("-208,0", "+30,2"),
                HunkHeader("-209,1", "+50,3"),
                HunkHeader("-211,1", "+53,0"),
            ],
            [],
        )
        "#);
    }

    #[test]
    fn only_selections_workspace_example() {
        let with = vec![hunk_header("-1,10", "+1,10")];
        let actual = to_additive_hunks(
            [
                // commit NOT '2,3' of the old
                hunk_header("-2,2", "+0,0"),
                // commit NOT '6,7' of the old
                hunk_header("-6,2", "+0,0"),
                // commit NOT '9' of the old
                hunk_header("-9,1", "+0,0"),
                // commit NOT '10' of the old
                hunk_header("-10,1", "+0,0"),
                // commit '11' of the new
                hunk_header("-0,0", "+1,1"),
                // commit '15,16' of the new
                hunk_header("-0,0", "+5,2"),
                // commit '19,20' of the new
                hunk_header("-0,0", "+9,2"),
            ],
            &with,
            &with,
        )
        .unwrap();
        insta::assert_debug_snapshot!(actual, @r#"
        (
            [
                HunkHeader("-2,2", "+1,0"),
                HunkHeader("-6,2", "+1,0"),
                HunkHeader("-9,1", "+1,0"),
                HunkHeader("-10,1", "+1,0"),
                HunkHeader("-11,0", "+1,1"),
                HunkHeader("-11,0", "+5,2"),
                HunkHeader("-11,0", "+9,2"),
            ],
            [],
        )
        "#);
    }
}

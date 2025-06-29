mod to_additive_hunks {
    use super::super::to_additive_hunks;
    use crate::utils::hunk_header;

    #[test]
    fn rejected() {
        let wth = vec![hunk_header("-1,10", "+1,10")];
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                // rejected as old is out of bounds
                hunk_header("-20,1", "+1,10"),
                // rejected as new is out of bounds
                hunk_header("+1,10", "+20,1"),
                // rejected as it doesn't match any anchor point, nor does it match hunks without context
                hunk_header("-0,0", "+2,10")
            ],
            &wth,
            &wth,
        ), @r#"
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
        let wth = vec![hunk_header("-1,10", "+1,10")];
        insta::assert_debug_snapshot!(to_additive_hunks(
            [
                hunk_header("-1,1", "+0,0"),
                hunk_header("-5,2", "+0,0"),
                hunk_header("-10,1", "+0,0")
            ],
            &wth,
            &wth,
        ), @r#"
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
            &wth,
            &wth,
        ), @r#"
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
            &wth,
            &wth,
        ), @r#"
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
            &wth,
            &wth,
        ), @r#"
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
        let wth = vec![
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
            &wth,
            &wth,
        ), @r#"
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
        let wth = vec![
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
            &wth,
            &wth,
        ), @r#"
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
        let wth = vec![hunk_header("-93,8", "+93,10")];

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
            &wth,
            &wth0,
        ), @r#"
        (
            [
                HunkHeader("-96,1", "+96,0"),
            ],
            [],
        )
        "#);
        insta::assert_debug_snapshot!(to_additive_hunks(
            [hunk_header("-96,1", "+0,0"), hunk_header("-0,0", "+96,2")],
            &wth,
            &wth0,
        ), @r#"
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
            &wth,
            &wth0,
        ), @r#"
        (
            [
                HunkHeader("-96,0", "+96,2"),
            ],
            [],
        )
        "#);
    }
}

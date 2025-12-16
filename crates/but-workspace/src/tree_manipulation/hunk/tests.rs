mod subtract_hunks {
    use super::super::{HunkSubstraction::*, subtract_hunks};
    use crate::utils::range;
    use but_testsupport::hunk_header;

    #[test]
    fn removing_all_in_old_leaves_all_new_multi() {
        for new_range in ["+1,5", "+1,1", "+1,0"] {
            assert_eq!(
                subtract_hunks(
                    hunk_header("-1,5", new_range),
                    (1..=5).map(|start| Old(range(start, 1)))
                )
                .unwrap(),
                [hunk_header("-6,0", new_range)],
                "{new_range}: Subtracting all lines from old adds them, starting from the front"
            );
        }
        for old_range in ["-1,5", "-1,1", "-1,0"] {
            assert_eq!(
                subtract_hunks(
                    hunk_header(old_range, "+1,5"),
                    (1..=5).map(|start| New(range(start, 1)))
                )
                .unwrap(),
                [hunk_header(old_range, "+6,0")],
                "{old_range}: Subtracting all lines from new leaves nothing, startinf from the front"
            );
        }
    }

    #[test]
    fn removing_all_in_old_leaves_all_new_single() {
        assert_eq!(
            subtract_hunks(hunk_header("-1,5", "+1,10"), Some(Old(range(1, 5)))).unwrap(),
            [hunk_header("-6,0", "+1,10")],
            "this re-adds old as nothing was consumed"
        );

        assert_eq!(
            subtract_hunks(hunk_header("-1,10", "+1,5"), Some(New(range(1, 5)))).unwrap(),
            [hunk_header("-1,10", "+6,0")],
            "this removes all old while adding nothing new (remember that unconsumed new lines are skipped between hunks)"
        );
    }

    #[test]
    fn remove_at_beginning() {
        assert_eq!(
            subtract_hunks(hunk_header("-1,5", "+1,5"), Some(Old(range(1, 1)))).unwrap(),
            [hunk_header("-2,4", "+1,5")],
            "starting old one line later re-adds the line"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-1,5", "+1,5"), Some(New(range(1, 1)))).unwrap(),
            [hunk_header("-1,5", "+2,4")],
            "starting new one line later removes the added line"
        );
    }

    #[test]
    fn remove_from_end() {
        assert_eq!(
            subtract_hunks(hunk_header("-1,5", "+1,5"), Some(Old(range(5, 1)))).unwrap(),
            [hunk_header("-1,4", "+1,5")],
            "consuming one less old line re-adds it"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-1,5", "+1,5"), Some(New(range(5, 1)))).unwrap(),
            [hunk_header("-1,5", "+1,4")],
            "consuming one less new line removes it"
        );
    }

    #[test]
    fn single_split() {
        assert_eq!(
            subtract_hunks(hunk_header("-1,3", "+1,3"), Some(Old(range(2, 1)))).unwrap(),
            [hunk_header("-1,1", "+1,1"), hunk_header("-3,1", "+2,2")],
            "one line subtracted in old, equivalent lines split from new, which keeps everything"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-1,3", "+3,3"), Some(Old(range(2, 1)))).unwrap(),
            [hunk_header("-1,1", "+3,1"), hunk_header("-3,1", "+4,2")],
            "like before, but with line offset that doesn't matter"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-1,3", "+1,3"), Some(New(range(2, 1)))).unwrap(),
            [hunk_header("-1,1", "+1,1"), hunk_header("-2,2", "+3,1")],
            "one line subtracted in new, equivalent lines split from old, which keeps everything"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-3,3", "+1,3"), Some(New(range(2, 1)))).unwrap(),
            [hunk_header("-3,1", "+1,1"), hunk_header("-4,2", "+3,1")],
            "like before, but with line offset that doesn't matter"
        );
    }

    #[test]
    fn single_split_exhausted() {
        assert_eq!(
            subtract_hunks(hunk_header("-1,3", "+1,1"), Some(Old(range(2, 1)))).unwrap(),
            [hunk_header("-1,1", "+1,1"), hunk_header("-3,1", "+2,0")],
            "new is exhausted, signalled by 'noop' hunk"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-1,3", "+1,0"), Some(Old(range(2, 1)))).unwrap(),
            [hunk_header("-1,1", "+1,0"), hunk_header("-3,1", "+1,0")],
            "multiple noops are fine as well"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-1,1", "+1,3"), Some(New(range(2, 1)))).unwrap(),
            [hunk_header("-1,1", "+1,1"), hunk_header("-2,0", "+3,1")],
            "old is exhausted, signalled by 'noop' hunk"
        );
        assert_eq!(
            subtract_hunks(hunk_header("-1,0", "+1,3"), Some(New(range(2, 1)))).unwrap(),
            [hunk_header("-1,0", "+1,1"), hunk_header("-1,0", "+3,1")],
            "multiple noops are fine as well"
        );
    }

    #[test]
    fn multi_split() {
        // 1 2 3 4 5
        // 1   3   5
        assert_eq!(
            subtract_hunks(
                hunk_header("-1,5", "+1,5"),
                [Old(range(2, 1)), Old(range(4, 1))]
            )
            .unwrap(),
            [
                hunk_header("-1,1", "+1,1"),
                hunk_header("-3,1", "+2,1"),
                hunk_header("-5,1", "+3,3")
            ],
            "new doesn't loose any content while exhausting its lines from the start"
        );
        assert_eq!(
            subtract_hunks(
                hunk_header("-1,5", "+1,5"),
                [New(range(2, 1)), New(range(4, 1))]
            )
            .unwrap(),
            [
                hunk_header("-1,1", "+1,1"),
                hunk_header("-2,1", "+3,1"),
                hunk_header("-3,3", "+5,1")
            ],
            "old doesn't loose any content while exhausting its lines from the start"
        );
    }

    #[test]
    fn multi_split_mixed() {
        assert_eq!(
            subtract_hunks(
                hunk_header("-1,5", "+1,5"),
                [
                    Old(range(2, 1)),
                    New(range(2, 1)),
                    Old(range(4, 1)),
                    New(range(4, 1))
                ]
            )
            .unwrap(),
            [
                hunk_header("-1,1", "+1,1"),
                hunk_header("-3,1", "+3,1"),
                hunk_header("-5,1", "+5,1")
            ],
            "mixed splits work just like one would expect, rules repeat"
        );
    }

    #[test]
    fn multi_split_mixed_sort_dependent() {
        assert_eq!(
            subtract_hunks(
                hunk_header("-1,5", "+1,5"),
                [Old(range(2, 1)), New(range(1, 5)),]
            )
            .unwrap(),
            [hunk_header("-1,1", "+6,0"), hunk_header("-3,3", "+6,0")],
            "Splits are handled in order, and it's possible for these to not match up anymore"
        );
        assert_eq!(
            subtract_hunks(
                hunk_header("-1,5", "+1,5"),
                [New(range(2, 1)), Old(range(1, 5)),]
            )
            .unwrap(),
            [hunk_header("-6,0", "+1,1"), hunk_header("-6,0", "+3,3")]
        );
    }
}

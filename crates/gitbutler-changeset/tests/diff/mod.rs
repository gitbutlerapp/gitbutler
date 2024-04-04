mod hunk {
    use gitbutler_changeset::{Change, FormatHunk, RawHunk};
    use std::fmt;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestHunk {
        removal_start: usize,
        addition_start: usize,
        changes: Vec<Change>,
    }

    impl RawHunk for TestHunk {
        type ChangeIterator = std::vec::IntoIter<Change>;

        fn get_removal_start(&self) -> usize {
            self.removal_start
        }

        fn get_addition_start(&self) -> usize {
            self.addition_start
        }

        fn changes(&self) -> Self::ChangeIterator {
            self.changes.clone().into_iter()
        }
    }

    impl fmt::Display for TestHunk {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.fmt_unified(f)
        }
    }

    #[test]
    fn empty_hunk() {
        let hunk = TestHunk {
            removal_start: 1,
            addition_start: 1,
            changes: vec![],
        };

        assert_eq!(format!("{hunk}"), "");
    }

    #[test]
    fn single_removal() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![Change::Removal("Hello, world!".to_string())],
        };

        assert_eq!(
            format!("{hunk}"),
            "@@ -30 +38,0 @@\n-Hello, world!\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn single_removal_trailing_nl() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![Change::Removal("Hello, world!\n".to_string())],
        };

        assert_eq!(format!("{hunk}"), "@@ -30 +38,0 @@\n-Hello, world!\n");
    }

    #[test]
    fn single_addition() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![Change::Addition("Hello, world!".to_string())],
        };

        assert_eq!(
            format!("{hunk}"),
            "@@ -30,0 +38 @@\n+Hello, world!\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn single_addition_trailing_nl() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![Change::Addition("Hello, world!\n".to_string())],
        };

        assert_eq!(format!("{hunk}"), "@@ -30,0 +38 @@\n+Hello, world!\n");
    }

    #[test]
    fn single_modified_line() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![
                Change::Removal("Hello, world!".to_string()),
                Change::Addition("Hello, GitButler!\n".to_string()),
            ],
        };

        assert_eq!(
            format!("{hunk}"),
            "@@ -30 +38 @@\n-Hello, world!\n\\ No newline at end of file\n+Hello, GitButler!\n"
        );
    }

    #[test]
    fn preserve_change_order() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 20,
            changes: vec![
                Change::Addition("Hello, GitButler!\n".to_string()),
                Change::Removal("Hello, world!\n".to_string()),
                Change::Removal("Hello, world 2!\n".to_string()),
                Change::Addition("Hello, GitButler 2!\n".to_string()),
                Change::Removal("Hello, world 3!".to_string()),
                Change::Addition("Hello, GitButler 3!\n".to_string()),
                Change::Addition("Hello, GitButler 4!".to_string()),
            ],
        };

        assert_eq!(
                format!("{hunk}"),
                "@@ -30,3 +20,4 @@\n+Hello, GitButler!\n-Hello, world!\n-Hello, world 2!\n+Hello, GitButler 2!\n-Hello, world 3!\n\\ No newline at end of file\n+Hello, GitButler 3!\n+Hello, GitButler 4!\n\\ No newline at end of file\n"
            );
    }
}

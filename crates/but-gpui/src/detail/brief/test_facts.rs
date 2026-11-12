//! What the branch's diff says about its tests, whatever the brief says. An agent that wants a
//! pass can turn a test off and write that everything passes, so this reads the lines.
//!
//! Only a line that turns a test off is called out. In the diffs of real briefs none of these
//! patterns came up by accident.

use crate::backend::{ChangeKind, FileDiff, LineKind};

/// What in an added line of a test file turns a test off, or leaves the others out.
const TURNS_OFF: [&str; 16] = [
    "it.skip(",
    "test.skip(",
    "describe.skip(",
    "it.only(",
    "test.only(",
    "describe.only(",
    "it.todo(",
    "test.todo(",
    "xit(",
    "xtest(",
    "xdescribe(",
    "@pytest.mark.skip",
    "@unittest.skip",
    "t.Skip(",
    "@Disabled",
    "@Ignore",
];

/// Whether `line` of a file turns a test off. Rust's attribute goes anywhere, as its tests do. A
/// mark has to start a word, as `exit(` isn't `xit(`.
fn turns_off(line: &str, in_tests: bool) -> bool {
    if line.trim_start().starts_with("#[ignore") {
        return true;
    }
    in_tests
        && TURNS_OFF.iter().any(|mark| {
            (line.match_indices(mark)).any(|(at, _)| {
                !(line.get(..at).unwrap_or_default().chars().next_back())
                    .is_some_and(|before| before.is_alphanumeric() || before == '_')
            })
        })
}

/// How many of the branch's changes are to tests.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TestFacts {
    /// The files of tests it changes, as places in the diff.
    pub files: Vec<usize>,
    pub added: u32,
    pub removed: u32,
    pub deleted: Vec<String>,
    pub turned_off: Vec<TurnedOff>,
    /// It changes something other than tests and words.
    pub changes_code: bool,
}

/// A line that turns a test off, as a place in the diff.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnedOff {
    pub path: String,
    /// The line, as it's what says which test.
    pub text: String,
    pub hunk: usize,
    pub line: usize,
}

/// Whether `path` is where tests go, by the names projects give them.
fn is_test(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.split('/')
        .any(|part| matches!(part, "test" | "tests" | "__tests__" | "e2e" | "spec"))
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.contains("_test.")
        || name.starts_with("test_")
}

/// Whether `path` holds only words or settings, which tests couldn't cover.
fn is_words(path: &str) -> bool {
    (path.rsplit('.').next()).is_some_and(|extension| {
        matches!(
            extension,
            "md" | "mdx" | "txt" | "json" | "lock" | "toml" | "yml" | "yaml" | "svg" | "png"
        )
    })
}

impl TestFacts {
    pub fn of(files: &[FileDiff]) -> Self {
        let mut facts = TestFacts::default();
        for (file_ix, file) in files.iter().enumerate() {
            let path = file.file.path.as_str();
            let tests = is_test(path);
            // Tests inside a source file, as Rust has them.
            let inline = (file.hunks().iter().flat_map(|hunk| &hunk.lines)).any(|line| {
                line.kind == LineKind::Added && line.text.trim_start().starts_with("#[test]")
            });
            if tests || inline {
                let (added, removed) = file.stats();
                facts.files.push(file_ix);
                facts.added += added;
                facts.removed += removed;
                if file.file.kind == ChangeKind::Deleted {
                    facts.deleted.push(path.to_owned());
                }
            }
            if !tests && !is_words(path) {
                facts.changes_code = true;
            }
            for (hunk_ix, hunk) in file.hunks().iter().enumerate() {
                for (line_ix, line) in hunk.lines.iter().enumerate() {
                    if line.kind == LineKind::Added && turns_off(&line.text, tests || inline) {
                        facts.turned_off.push(TurnedOff {
                            path: path.to_owned(),
                            text: line.text.trim().to_owned(),
                            hunk: hunk_ix,
                            line: line_ix,
                        });
                    }
                }
            }
        }
        facts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::DiffLine;

    /// A file with one hunk of `lines`, each a kind of line and its text.
    fn file_of(path: &str, lines: &[(LineKind, &str)]) -> FileDiff {
        use crate::backend::{DiffBody, FileChange, Hunk, HunkKey};
        let hunk = Hunk {
            key: HunkKey(but_core::HunkHeader {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            }),
            header: String::new(),
            patch: "".into(),
            lines: (lines.iter())
                .map(|(kind, text)| DiffLine::new(*kind, Some(1), Some(1), text))
                .collect(),
            scope: Vec::new(),
            lead: Vec::new(),
            tail: Vec::new(),
        };
        FileDiff {
            file: FileChange::bare(path),
            body: DiffBody::Hunks {
                hunks: vec![hunk],
                added: 0,
                removed: 0,
                widest: [1, 1],
                conflicts: Vec::new(),
            },
            fingerprint: 0,
            annotated: true,
            scope_lines: None,
            drawing: None,
            made_by: Vec::new(),
        }
    }

    #[test]
    fn only_an_added_line_turns_a_test_off() {
        let facts = TestFacts::of(&[file_of(
            "src/card.test.tsx",
            &[
                (LineKind::Removed, "it.skip(\"was off\", () => {"),
                (LineKind::Added, "it(\"was off\", () => {"),
                (LineKind::Added, "it.skip(\"now off\", () => {"),
            ],
        )]);
        assert_eq!(
            facts.turned_off.len(),
            1,
            "taking one off isn't turning one off"
        );
        assert_eq!(
            facts.turned_off[0].text, "it.skip(\"now off\", () => {",
            "the line says which test"
        );
        assert_eq!(facts.files, [0], "and it's a file of tests");
    }

    #[test]
    fn what_only_looks_like_turning_a_test_off_is_not() {
        for line in [
            "    process::exit(1);",
            "let rest = lines.iter().skip(1);",
            "const SKIP: &str = \"it.skipper\";",
        ] {
            assert!(!turns_off(line, true), "{line} turns nothing off");
        }
        assert!(
            !turns_off("    it.skip(\"a\", () => {", false),
            "outside tests it's something else's words"
        );
        assert!(
            turns_off("    #[ignore = \"flaky\"]", false),
            "Rust's tests live anywhere"
        );
    }

    #[test]
    fn tests_are_told_by_where_projects_keep_them() {
        for path in [
            "src/share-card.test.tsx",
            "crates/but/tests/status.rs",
            "app/__tests__/a.js",
            "e2e/login.ts",
            "pkg/cache_test.go",
            "tests/test_cache.py",
        ] {
            assert!(is_test(path), "{path} is a test");
        }
        for path in ["src/testing.ts", "src/latest.rs", "docs/contest.md"] {
            assert!(!is_test(path), "{path} only has the word in it");
        }
    }
}

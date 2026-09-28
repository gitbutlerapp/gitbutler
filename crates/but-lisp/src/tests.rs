use crate::{ChangedLine, File, FileStatus, Query, Side};

const FILE: File<'_> = File {
    path: b"src/main.rs",
    status: FileStatus::Modified,
};

const LINES: &[ChangedLine<'_>] = &[
    ChangedLine {
        side: Side::Removed,
        content: b"old value",
    },
    ChangedLine {
        side: Side::Added,
        content: b"TODO: fix this",
    },
    ChangedLine {
        side: Side::Added,
        content: b"new value",
    },
];

#[test]
fn exclude_individual_lines() {
    assert_eq!(
        Query::parse(r#"(not (line :contains "TODO"))"#)
            .unwrap()
            .select(FILE, LINES),
        [true, false, true],
        "only the TODO line is excluded"
    );
}

#[test]
fn exclude_whole_hunks() {
    assert_eq!(
        Query::parse(r#"(not (hunk-added :contains "TODO"))"#)
            .unwrap()
            .select(FILE, LINES),
        [false, false, false],
        "an added TODO excludes the entire hunk"
    );
}

#[test]
fn restrict_hunk_matching_to_one_side() {
    assert_eq!(
        Query::parse(r#"(hunk-removed :contains "TODO")"#)
            .unwrap()
            .select(FILE, LINES),
        [false, false, false],
        "an added TODO does not match removed lines"
    );
}

#[test]
fn compose_selections() {
    let query = Query::parse(
        r#"(difference
        (union (line-added :contains "value") (line-removed :contains "value"))
        (intersection (hunk :contains "TODO") (line-removed :contains "old")))"#,
    )
    .unwrap();
    assert_eq!(
        query.select(FILE, LINES),
        [false, false, true],
        "set operators combine line and hunk selections"
    );
}

#[test]
fn non_text_changes_are_preserved_by_text_exclusions() {
    assert!(
        Query::parse(r#"(not (line :contains "TODO"))"#)
            .unwrap()
            .selects_non_text(FILE),
        "text exclusions preserve binary files"
    );
    assert!(
        !Query::parse(r#"(hunk :contains "TODO")"#)
            .unwrap()
            .selects_non_text(FILE),
        "binary files do not match text selectors"
    );
}

#[test]
fn string_escapes_and_unicode() {
    let query = Query::parse(r#"(line :contains "é\"\\")"#).unwrap();
    assert_eq!(
        query.select(
            FILE,
            &[ChangedLine {
                side: Side::Added,
                content: "é\"\\".as_bytes()
            }]
        ),
        [true],
        "quoted strings decode escapes without losing Unicode"
    );
}

#[test]
fn non_utf8_lines_are_not_lossily_decoded() {
    assert_eq!(
        Query::parse(r#"(line :contains "TODO")"#).unwrap().select(
            FILE,
            &[ChangedLine {
                side: Side::Added,
                content: b"\xffTODO"
            }]
        ),
        [true],
        "matching preserves arbitrary file bytes"
    );
}

#[test]
fn unknown_selector_has_a_source_span() {
    use miette::Diagnostic as _;
    let error = Query::parse(r#"(unknown :contains "TODO")"#).unwrap_err();
    let label = error.labels().unwrap().next().unwrap();
    assert_eq!(
        (label.offset(), label.len()),
        (1, 7),
        "the unknown selector is highlighted"
    );
    snapbox::assert_data_eq!(
        error.to_string(),
        "Invalid diff query: unknown selector or operator 'unknown'"
    );
}

#[test]
fn missing_closing_parenthesis() {
    snapbox::assert_data_eq!(
        Query::parse(r#"(line :contains "TODO""#)
            .unwrap_err()
            .to_string(),
        "Invalid diff query: expected ')'"
    );
}

#[test]
fn extra_operand() {
    assert!(
        Query::parse(r#"(not (line :contains "a") (line :contains "b"))"#).is_err(),
        "not takes one operand"
    );
}

#[test]
fn missing_operand() {
    assert!(
        Query::parse(r#"(difference (line :contains "a"))"#).is_err(),
        "difference takes two operands"
    );
}

#[test]
fn empty_union() {
    assert!(
        Query::parse("(union)").is_err(),
        "union requires at least two operands"
    );
}

#[test]
fn unsupported_predicate() {
    assert!(
        Query::parse(r#"(line :regex "TODO")"#).is_err(),
        "unimplemented predicates are rejected"
    );
}

#[test]
fn trailing_input() {
    assert!(
        Query::parse(r#"(line :contains "TODO") garbage"#).is_err(),
        "the parser consumes the entire input"
    );
}

#[test]
fn unterminated_string() {
    assert!(
        Query::parse(r#"(line :contains "TODO)"#).is_err(),
        "unterminated strings are rejected"
    );
}

#[test]
fn file_path_selects_all_changed_lines() {
    assert_eq!(
        Query::parse(r#"(file :path "src/main.rs")"#)
            .unwrap()
            .select(FILE, LINES),
        [true, true, true],
        "matching files select every candidate line"
    );
    assert!(
        !Query::parse(r#"(file :path "main.rs")"#)
            .unwrap()
            .selects_non_text(FILE),
        "paths match exactly, relative to the repository"
    );
}

#[test]
fn file_glob_matches_nested_and_direct_children() {
    let query = Query::parse(r#"(file :glob "src/**/*.rs")"#).unwrap();
    assert!(
        query.selects_non_text(FILE),
        "double-star also matches zero directories"
    );
    assert!(
        query.selects_non_text(File {
            path: b"src/nested/lib.rs",
            ..FILE
        }),
        "double-star matches nested directories"
    );
    assert!(
        !query.selects_non_text(File {
            path: b"tests/lib.rs",
            ..FILE
        }),
        "globs are anchored to the repository-relative path"
    );
}

#[test]
fn file_glob_single_star_does_not_cross_directories() {
    let query = Query::parse(r#"(file :glob "src/*.rs")"#).unwrap();
    assert!(
        !query.selects_non_text(File {
            path: b"src/nested/lib.rs",
            ..FILE
        }),
        "single-star stays inside one directory"
    );
    assert!(
        !query.selects_non_text(File {
            path: b"src/main.RS",
            ..FILE
        }),
        "globs are case-sensitive"
    );
}

#[test]
fn file_glob_preserves_non_utf8_paths() {
    assert!(
        Query::parse(r#"(file :glob "src/*.rs")"#)
            .unwrap()
            .selects_non_text(File {
                path: b"src/\xff.rs",
                ..FILE
            }),
        "glob matching does not require lossy path conversion"
    );
}

#[test]
fn file_extension_matches_the_last_filename_suffix() {
    let query = Query::parse(r#"(file :extension "lock")"#).unwrap();
    assert!(
        query.selects_non_text(File {
            path: b"nested/Cargo.lock",
            ..FILE
        }),
        "extensions omit the dot"
    );
    assert!(
        !query.selects_non_text(File {
            path: b"dir.lock/file",
            ..FILE
        }),
        "directory suffixes are not file extensions"
    );
    assert!(
        !query.selects_non_text(File {
            path: b"Cargo.lock.bak",
            ..FILE
        }),
        "only the final suffix matches"
    );
    assert!(
        !query.selects_non_text(File {
            path: b".lock",
            ..FILE
        }),
        "a leading dot does not give a dotfile an extension"
    );
}

#[test]
fn file_status_added() {
    let query = Query::parse("(file :status :added)").unwrap();
    assert!(
        query.selects_non_text(File {
            status: FileStatus::Added,
            ..FILE
        }),
        "added files match"
    );
    assert!(
        !query.selects_non_text(FILE),
        "modified files do not match added"
    );
}

#[test]
fn file_status_deleted() {
    assert!(
        Query::parse("(file :status :deleted)")
            .unwrap()
            .selects_non_text(File {
                status: FileStatus::Deleted,
                ..FILE
            }),
        "deleted files match"
    );
}

#[test]
fn file_status_renamed() {
    assert!(
        Query::parse("(file :status :renamed)")
            .unwrap()
            .selects_non_text(File {
                status: FileStatus::Renamed,
                ..FILE
            }),
        "renamed files match"
    );
}

#[test]
fn file_status_modified() {
    assert!(
        Query::parse("(file :status :modified)")
            .unwrap()
            .selects_non_text(FILE),
        "modified files match"
    );
}

#[test]
fn file_and_line_selectors_compose() {
    let query =
        Query::parse(r#"(difference (file :glob "src/**/*.rs") (line-added :contains "TODO"))"#)
            .unwrap();
    assert_eq!(
        query.select(FILE, LINES),
        [true, false, true],
        "line exclusions narrow matching files"
    );
    assert_eq!(
        query.select(
            File {
                path: b"tests/main.rs",
                ..FILE
            },
            LINES
        ),
        [false, false, false],
        "line predicates cannot expand the file selection"
    );
}

#[test]
fn invalid_file_glob_has_a_source_span() {
    use miette::Diagnostic as _;
    let error = Query::parse(r#"(file :glob "[")"#).unwrap_err();
    let label = error.labels().unwrap().next().unwrap();
    assert_eq!(
        (label.offset(), label.len()),
        (12, 3),
        "the invalid glob string is highlighted"
    );
}

#[test]
fn invalid_file_status() {
    assert!(
        Query::parse("(file :status :unknown)").is_err(),
        "unsupported statuses are rejected at parse time"
    );
}

#[test]
fn binary_file_predicate_is_still_unsupported() {
    assert!(
        Query::parse("(file :binary true)").is_err(),
        "binary classification is not implemented yet"
    );
}

#[test]
fn excessive_nesting() {
    let query = format!(
        "{}(line :contains \"TODO\"){}",
        "(not ".repeat(100),
        ")".repeat(100)
    );
    assert!(
        Query::parse(&query).is_err(),
        "user-controlled nesting is bounded"
    );
}

use crate::{ChangedLine, Query, Side};

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
            .select(LINES),
        [true, false, true],
        "only the TODO line is excluded"
    );
}

#[test]
fn exclude_whole_hunks() {
    assert_eq!(
        Query::parse(r#"(not (hunk-added :contains "TODO"))"#)
            .unwrap()
            .select(LINES),
        [false, false, false],
        "an added TODO excludes the entire hunk"
    );
}

#[test]
fn restrict_hunk_matching_to_one_side() {
    assert_eq!(
        Query::parse(r#"(hunk-removed :contains "TODO")"#)
            .unwrap()
            .select(LINES),
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
        query.select(LINES),
        [false, false, true],
        "set operators combine line and hunk selections"
    );
}

#[test]
fn non_text_changes_are_preserved_by_text_exclusions() {
    assert!(
        Query::parse(r#"(not (line :contains "TODO"))"#)
            .unwrap()
            .selects_non_text(),
        "text exclusions preserve binary files"
    );
    assert!(
        !Query::parse(r#"(hunk :contains "TODO")"#)
            .unwrap()
            .selects_non_text(),
        "binary files do not match text selectors"
    );
}

#[test]
fn string_escapes_and_unicode() {
    let query = Query::parse(r#"(line :contains "é\"\\")"#).unwrap();
    assert_eq!(
        query.select(&[ChangedLine {
            side: Side::Added,
            content: "é\"\\".as_bytes()
        }]),
        [true],
        "quoted strings decode escapes without losing Unicode"
    );
}

#[test]
fn non_utf8_lines_are_not_lossily_decoded() {
    assert_eq!(
        Query::parse(r#"(line :contains "TODO")"#)
            .unwrap()
            .select(&[ChangedLine {
                side: Side::Added,
                content: b"\xffTODO"
            }]),
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

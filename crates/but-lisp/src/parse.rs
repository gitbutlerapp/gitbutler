use miette::{Diagnostic, NamedSource, SourceSpan};

use crate::{
    FileStatus, Query, Side,
    query::{Expression, FilePredicate, LinePredicate},
};

/// A query error with the original input and the offending source span.
#[derive(Debug, thiserror::Error, Diagnostic)]
#[error("Invalid diff query: {message}")]
pub struct QueryError {
    message: String,
    #[source_code]
    input: NamedSource<String>,
    #[label("{message}")]
    span: SourceSpan,
}

impl Query {
    /// Parse a single query expression. Syntax and selector validation happen here,
    /// before any repository changes are made.
    pub fn parse(source: &str) -> Result<Query, QueryError> {
        let mut parser = Parser { source, offset: 0 };
        let expression = parser.expression(0)?;
        parser.whitespace();
        if parser.offset != source.len() {
            return Err(parser.error(
                parser.offset,
                source.len() - parser.offset,
                "expected end of query",
            ));
        }
        Ok(Query(expression))
    }
}

struct Parser<'a> {
    source: &'a str,
    offset: usize,
}

impl Parser<'_> {
    fn error(&self, start: usize, len: usize, message: impl Into<String>) -> QueryError {
        QueryError {
            message: message.into(),
            input: NamedSource::new("query", self.source.to_owned()),
            span: (start, len).into(),
        }
    }

    fn whitespace(&mut self) {
        while let Some(character) = self.source[self.offset..].chars().next() {
            if !character.is_whitespace() {
                break;
            }
            self.offset += character.len_utf8();
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), QueryError> {
        self.whitespace();
        if self.source.as_bytes().get(self.offset) != Some(&expected) {
            return Err(self.error(
                self.offset,
                0,
                format!("expected '{}'", char::from(expected)),
            ));
        }
        self.offset += 1;
        Ok(())
    }

    fn atom(&mut self) -> Result<(&str, usize), QueryError> {
        self.whitespace();
        let start = self.offset;
        while let Some(character) = self.source[self.offset..].chars().next() {
            if character.is_whitespace() || matches!(character, '(' | ')' | '"') {
                break;
            }
            self.offset += character.len_utf8();
        }
        if self.offset == start {
            return Err(self.error(start, 0, "expected a selector or operator"));
        }
        Ok((&self.source[start..self.offset], start))
    }

    fn string(&mut self) -> Result<String, QueryError> {
        self.expect(b'"')?;
        let start = self.offset - 1;
        let mut result = String::new();
        while let Some(character) = self.source[self.offset..].chars().next() {
            self.offset += character.len_utf8();
            match character {
                '"' => return Ok(result),
                '\\' => {
                    let Some(escaped) = self.source[self.offset..].chars().next() else {
                        break;
                    };
                    let escape_offset = self.offset - 1;
                    self.offset += escaped.len_utf8();
                    result.push(match escaped {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => {
                            return Err(self.error(
                                escape_offset,
                                1 + escaped.len_utf8(),
                                "unsupported string escape",
                            ));
                        }
                    });
                }
                character => result.push(character),
            }
        }
        Err(self.error(start, self.offset - start, "unterminated string"))
    }

    fn has_predicate(&mut self) -> bool {
        self.whitespace();
        self.offset < self.source.len() && self.source.as_bytes()[self.offset] != b')'
    }

    fn line_number(&mut self) -> Result<u32, QueryError> {
        let (number, start) = self.atom()?;
        let len = number.len();
        let parsed = number
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| number.parse::<u32>().ok())
            .flatten()
            .filter(|number| *number > 0);
        parsed.ok_or_else(|| {
            self.error(
                start,
                len,
                "expected a line number between 1 and 4294967295",
            )
        })
    }

    fn expression(&mut self, depth: usize) -> Result<Expression, QueryError> {
        // Bound user-controlled recursion in both parsing and evaluation.
        if depth >= 64 {
            return Err(self.error(self.offset, 0, "query nesting exceeds 64 levels"));
        }
        self.expect(b'(')?;
        let expression_start = self.offset - 1;
        let (operator, start) = self.atom()?;
        let operator = operator.to_owned();
        let expression = match operator.as_str() {
            "line" | "line-added" | "line-removed" | "hunk" | "hunk-added" | "hunk-removed" => {
                let mut predicates = Vec::new();
                while self.has_predicate() {
                    let (predicate, predicate_start) = self.atom()?;
                    let predicate = predicate.to_owned();
                    predicates.push(match predicate.as_str() {
                        ":contains" => LinePredicate::Contains(self.string()?),
                        ":regex" => {
                            self.whitespace();
                            let start = self.offset;
                            let pattern = self.string()?;
                            let regex = regex::bytes::Regex::new(&pattern).map_err(|error| {
                                self.error(
                                    start,
                                    self.offset - start,
                                    format!("invalid regex: {error}"),
                                )
                            })?;
                            LinePredicate::Regex(regex)
                        }
                        ":range" => {
                            self.whitespace();
                            let start = self.offset;
                            if self.source.as_bytes().get(self.offset) == Some(&b'\'') {
                                self.offset += 1;
                            }
                            self.expect(b'(')?;
                            let lower = self.line_number()?;
                            let upper = self.line_number()?;
                            self.expect(b')')?;
                            if lower > upper {
                                return Err(self.error(
                                    start,
                                    self.offset - start,
                                    "range start must not exceed range end",
                                ));
                            }
                            LinePredicate::Range(lower..=upper)
                        }
                        _ => {
                            return Err(self.error(
                                predicate_start,
                                predicate.len(),
                                "expected :contains, :regex, or :range",
                            ));
                        }
                    });
                }
                if predicates.is_empty() {
                    return Err(self.error(
                        start,
                        operator.len(),
                        "expected at least one predicate",
                    ));
                }
                let side = if operator.ends_with("-added") {
                    Some(Side::Added)
                } else if operator.ends_with("-removed") {
                    Some(Side::Removed)
                } else {
                    None
                };
                Expression::Lines {
                    hunk: operator.starts_with("hunk"),
                    side,
                    predicates,
                }
            }
            "file" => {
                let mut predicates = Vec::new();
                while self.has_predicate() {
                    let (predicate, predicate_start) = self.atom()?;
                    let predicate = predicate.to_owned();
                    let predicate = match predicate.as_str() {
                        ":path" => FilePredicate::Path(self.string()?),
                        ":extension" => FilePredicate::Extension(self.string()?),
                        ":glob" => {
                            self.whitespace();
                            let start = self.offset;
                            let pattern = self.string()?;
                            let glob = globset::GlobBuilder::new(&pattern)
                                .literal_separator(true)
                                .backslash_escape(true)
                                .build()
                                .map_err(|error| {
                                    self.error(
                                        start,
                                        self.offset - start,
                                        format!("invalid glob: {error}"),
                                    )
                                })?;
                            FilePredicate::Glob(glob.compile_matcher())
                        }
                        ":status" => {
                            let (status, start) = self.atom()?;
                            let status = match status {
                                ":added" => FileStatus::Added,
                                ":deleted" => FileStatus::Deleted,
                                ":modified" => FileStatus::Modified,
                                ":renamed" => FileStatus::Renamed,
                                status => {
                                    let len = status.len();
                                    return Err(self.error(
                                        start,
                                        len,
                                        "expected :added, :deleted, :modified, or :renamed",
                                    ));
                                }
                            };
                            FilePredicate::Status(status)
                        }
                        _ => {
                            return Err(self.error(
                                predicate_start,
                                predicate.len(),
                                "expected :path, :glob, :extension, or :status",
                            ));
                        }
                    };
                    predicates.push(predicate);
                }
                if predicates.is_empty() {
                    return Err(self.error(
                        start,
                        operator.len(),
                        "expected at least one predicate",
                    ));
                }
                Expression::File(predicates)
            }
            "not" => Expression::Not(Box::new(self.expression(depth + 1)?)),
            "difference" => Expression::Difference(
                Box::new(self.expression(depth + 1)?),
                Box::new(self.expression(depth + 1)?),
            ),
            "union" | "intersection" => {
                let mut operands = Vec::new();
                loop {
                    self.whitespace();
                    if self.source.as_bytes().get(self.offset) == Some(&b')') {
                        break;
                    }
                    operands.push(self.expression(depth + 1)?);
                }
                if operands.len() < 2 {
                    return Err(self.error(
                        start,
                        operator.len(),
                        "expected at least two operands",
                    ));
                }
                if operator == "union" {
                    Expression::Union(operands)
                } else {
                    Expression::Intersection(operands)
                }
            }
            _ => {
                return Err(self.error(
                    start,
                    operator.len(),
                    format!("unknown selector or operator '{operator}'"),
                ));
            }
        };
        self.whitespace();
        if self.offset == self.source.len() {
            return Err(self.error(expression_start, 1, "expected ')'"));
        }
        self.expect(b')')?;
        Ok(expression)
    }
}

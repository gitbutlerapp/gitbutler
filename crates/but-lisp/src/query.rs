use bstr::ByteSlice as _;

/// A parsed selection query, independent of repositories and diff formatting.
#[derive(Debug, Clone)]
pub struct Query(pub(crate) Expression);

#[derive(Debug, Clone)]
pub(crate) enum Expression {
    Contains {
        hunk: bool,
        side: Option<Side>,
        text: String,
    },
    File(FilePredicate),
    Not(Box<Expression>),
    Union(Vec<Expression>),
    Intersection(Vec<Expression>),
    Difference(Box<Expression>, Box<Expression>),
}

#[derive(Debug, Clone)]
pub(crate) enum FilePredicate {
    Path(String),
    Glob(globset::GlobMatcher),
    Extension(String),
    Status(FileStatus),
}

/// Repository-relative file identity. Renames use their destination path;
/// deletions use the path that was removed.
#[derive(Debug, Clone, Copy)]
pub struct File<'a> {
    pub path: &'a [u8],
    pub status: FileStatus,
}

/// The kind of change made to a file. Untracked files count as added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

/// Which side of a diff a changed line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Added,
    Removed,
}

/// A changed line's content, without its diff prefix or line terminator.
/// Context lines must not be supplied to the evaluator.
#[derive(Debug, Clone, Copy)]
pub struct ChangedLine<'a> {
    pub side: Side,
    pub content: &'a [u8],
}

impl Query {
    /// Select changed lines in one hunk. The returned mask follows the input order.
    /// A matching hunk selector selects every changed line in this hunk.
    pub fn select(&self, file: File<'_>, lines: &[ChangedLine<'_>]) -> Vec<bool> {
        self.0.evaluate(file, Some(lines))
    }

    /// Select an indivisible change with no text diff, such as a binary file.
    /// Text selectors do not match it, but their complements do.
    pub fn selects_non_text(&self, file: File<'_>) -> bool {
        self.0.evaluate(file, None)[0]
    }
}

impl FilePredicate {
    fn matches(&self, file: File<'_>) -> bool {
        match self {
            FilePredicate::Path(path) => file.path == path.as_bytes(),
            FilePredicate::Glob(glob) => {
                glob.is_match_candidate(&globset::Candidate::from_bytes(file.path))
            }
            FilePredicate::Extension(extension) => {
                let filename = file
                    .path
                    .rsplit(|&byte| byte == b'/')
                    .next()
                    .unwrap_or_default();
                // A leading dot alone does not give a dotfile an extension.
                filename
                    .iter()
                    .rposition(|&byte| byte == b'.')
                    .filter(|&dot| dot != 0)
                    .is_some_and(|dot| &filename[dot + 1..] == extension.as_bytes())
            }
            FilePredicate::Status(status) => file.status == *status,
        }
    }
}

impl Expression {
    fn evaluate(&self, file: File<'_>, lines: Option<&[ChangedLine<'_>]>) -> Vec<bool> {
        let count = lines.map_or(1, |lines| lines.len());
        match self {
            Expression::Contains { hunk, side, text } => {
                let Some(lines) = lines else {
                    return Vec::from([false]);
                };
                let mut selected: Vec<bool> = lines
                    .iter()
                    .map(|line| {
                        side.is_none_or(|side| side == line.side)
                            && line.content.contains_str(text.as_bytes())
                    })
                    .collect();
                if *hunk {
                    let matches = selected.iter().any(|selected| *selected);
                    selected.fill(matches);
                }
                selected
            }
            Expression::File(predicate) => vec![predicate.matches(file); count],
            Expression::Not(expression) => expression
                .evaluate(file, lines)
                .into_iter()
                .map(|selected| !selected)
                .collect(),
            Expression::Union(expressions) | Expression::Intersection(expressions) => {
                let intersection = matches!(self, Expression::Intersection(_));
                let mut selected = vec![intersection; count];
                for expression in expressions {
                    for (selected, operand) in
                        selected.iter_mut().zip(expression.evaluate(file, lines))
                    {
                        *selected = if intersection {
                            *selected && operand
                        } else {
                            *selected || operand
                        };
                    }
                }
                selected
            }
            Expression::Difference(selection, exclusions) => selection
                .evaluate(file, lines)
                .into_iter()
                .zip(exclusions.evaluate(file, lines))
                .map(|(selected, excluded)| selected && !excluded)
                .collect(),
        }
    }
}

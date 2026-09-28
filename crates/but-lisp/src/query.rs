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
    Not(Box<Expression>),
    Union(Vec<Expression>),
    Intersection(Vec<Expression>),
    Difference(Box<Expression>, Box<Expression>),
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
    pub fn select(&self, lines: &[ChangedLine<'_>]) -> Vec<bool> {
        self.0.evaluate(Some(lines))
    }

    /// Select an indivisible change with no text diff, such as a binary file.
    /// Text selectors do not match it, but their complements do.
    pub fn selects_non_text(&self) -> bool {
        self.0.evaluate(None)[0]
    }
}

impl Expression {
    fn evaluate(&self, lines: Option<&[ChangedLine<'_>]>) -> Vec<bool> {
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
            Expression::Not(expression) => expression
                .evaluate(lines)
                .into_iter()
                .map(|selected| !selected)
                .collect(),
            Expression::Union(expressions) | Expression::Intersection(expressions) => {
                let intersection = matches!(self, Expression::Intersection(_));
                let mut selected = vec![intersection; count];
                for expression in expressions {
                    for (selected, operand) in selected.iter_mut().zip(expression.evaluate(lines)) {
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
                .evaluate(lines)
                .into_iter()
                .zip(exclusions.evaluate(lines))
                .map(|(selected, excluded)| selected && !excluded)
                .collect(),
        }
    }
}

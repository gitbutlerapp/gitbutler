use ratatui::text::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionDirection {
    /// Build the connector for a synthetic line rendered above the source line.
    Above,
    /// Build the connector for a synthetic line rendered below the source line.
    Below,
}

/// The returned connector preserves the input connector width (character count),
/// and applies graph-continuity rules per connector glyph.
///
/// If the input connector is empty, this returns two spaces (`"  "`) so
/// extension-only lines still align with regular graph rows.
pub fn extend_connector_spans<E>(connector: &[Span<'_>], direction: ExtensionDirection, out: &mut E)
where
    E: Extend<Span<'static>>,
{
    out.extend([Span::raw(extend_connector_text(
        &connector_text(connector),
        direction,
    ))]);
}

/// The connector for a synthetic line where a stop would create a new branch: the line's lane
/// gets a branch head, `├┄`, or `╭┄` when nothing continues above it, as a stack's top branch
/// has. The lanes around it continue as [`extend_connector_spans`] has them.
pub fn new_branch_connector_spans<E>(
    connector: &[Span<'_>],
    direction: ExtensionDirection,
    out: &mut E,
) where
    E: Extend<Span<'static>>,
{
    out.extend([Span::raw(new_branch_connector_text(
        &connector_text(connector),
        direction,
    ))]);
}

/// The connector for the spacer row between a new branch's head and the line it's next to,
/// as the graph draws one between a commit and the head of the branch below it.
pub fn new_branch_spacer_spans<E>(
    connector: &[Span<'_>],
    direction: ExtensionDirection,
    out: &mut E,
) where
    E: Extend<Span<'static>>,
{
    out.extend([Span::raw(new_branch_spacer_text(
        &connector_text(connector),
        direction,
    ))]);
}

fn new_branch_spacer_text(connector: &str, direction: ExtensionDirection) -> String {
    let lane = lane_column(connector);
    extend_connector_text(connector, direction)
        .chars()
        .take(lane)
        .chain(['│'])
        .collect()
}

/// `connector` of a stack's top branch line as it is drawn below a new branch above it: the
/// branch joins the stack instead of starting it, `╭` becoming `├`.
pub fn stacked_under_new_branch(connector: &[Span<'_>]) -> Vec<Span<'static>> {
    let lane = lane_depth(connector);
    let mut column = 0;
    connector
        .iter()
        .map(|span| {
            let content = span
                .content
                .chars()
                .map(|ch| {
                    let ch = match (column == lane, ch) {
                        (true, '╭') => '├',
                        (_, ch) => ch,
                    };
                    column += 1;
                    ch
                })
                .collect::<String>();
            Span::styled(content, span.style)
        })
        .collect()
}

/// How many lanes deep the line with `connector` is drawn: the graph columns before the line's
/// own glyph.
pub fn lane_depth(connector: &[Span<'_>]) -> usize {
    connector
        .iter()
        .flat_map(|span| span.content.chars())
        .take_while(|ch| is_lane(*ch))
        .count()
}

/// Whether `ch` is the column of a lane that continues past a line.
fn is_lane(ch: char) -> bool {
    matches!(ch, '┊' | '│')
}

fn connector_text(connector: &[Span<'_>]) -> String {
    connector.iter().map(|span| span.content.as_ref()).collect()
}

/// The column of the lane a connector's line is drawn in: past the columns of the lanes it is
/// nested in.
fn lane_column(connector: &str) -> usize {
    connector.chars().take_while(|ch| is_lane(*ch)).count()
}

fn new_branch_connector_text(connector: &str, direction: ExtensionDirection) -> String {
    let extended = extend_connector_text(connector, direction);
    let lane = lane_column(connector);
    let head = match extended.chars().nth(lane) {
        Some('│') => '├',
        Some(_) | None => '╭',
    };
    extended
        .chars()
        .take(lane)
        .chain([head, '┄', ' '])
        .collect()
}

/// This function preserves the number of characters in `connector`.
///
/// Graph rules:
/// - `┊` stays `┊` (column trunk marker)
/// - `│` stays `│`
/// - `●`, `◐`, `├`, `-` continue as `│` in both directions
/// - `╭` starts here: no above continuation, but below becomes `│`
/// - `╯` ends here: above becomes `│`, but no below continuation
/// - `┴` is a terminal merge-base cap: above becomes `│`, below becomes space
/// - `┄` is horizontal-only and becomes space in extension rows
/// - spaces stay spaces
///
/// Unknown characters are kept as-is.
fn extend_connector_text(connector: &str, direction: ExtensionDirection) -> String {
    if connector.is_empty() {
        return "  ".to_string();
    }

    connector
        .chars()
        .map(|ch| extension_char(ch, direction))
        .collect()
}

const fn extension_char(ch: char, direction: ExtensionDirection) -> char {
    match ch {
        ' ' => ' ',
        '┊' => '┊',
        '│' => '│',
        '●' | '◐' | '├' | '-' => '│',
        '╭' => match direction {
            ExtensionDirection::Above => ' ',
            ExtensionDirection::Below => '│',
        },
        '╯' => match direction {
            ExtensionDirection::Above => '│',
            ExtensionDirection::Below => ' ',
        },
        '┴' => match direction {
            ExtensionDirection::Above => '│',
            ExtensionDirection::Below => ' ',
        },
        '┄' => ' ',
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExtensionDirection, extend_connector_spans, extend_connector_text,
        new_branch_connector_text, new_branch_spacer_text, stacked_under_new_branch,
    };
    use ratatui::text::Span;

    #[test]
    fn a_new_branch_below_a_stacks_last_commit_joins_the_stack() {
        assert_eq!(
            new_branch_connector_text("┊●   ", ExtensionDirection::Below),
            "┊├┄ "
        );
    }

    #[test]
    fn a_new_branch_below_an_empty_branch_joins_the_stack() {
        assert_eq!(
            new_branch_connector_text("┊╭┄", ExtensionDirection::Below),
            "┊├┄ "
        );
    }

    #[test]
    fn a_new_branch_above_a_stacks_top_branch_starts_the_stack() {
        assert_eq!(
            new_branch_connector_text("┊╭┄", ExtensionDirection::Above),
            "┊╭┄ "
        );
    }

    #[test]
    fn a_new_branch_above_a_stacked_branch_joins_the_stack() {
        assert_eq!(
            new_branch_connector_text("┊├┄", ExtensionDirection::Above),
            "┊├┄ "
        );
    }

    #[test]
    fn a_new_branch_in_a_nested_lane_keeps_the_outer_lanes() {
        assert_eq!(
            new_branch_connector_text("┊┊●   ", ExtensionDirection::Below),
            "┊┊├┄ "
        );
    }

    #[test]
    fn a_new_branch_below_a_commit_is_spaced_from_it() {
        assert_eq!(
            new_branch_spacer_text("┊●   ", ExtensionDirection::Below),
            "┊│"
        );
    }

    #[test]
    fn a_new_branch_above_a_stacks_top_branch_is_spaced_from_it() {
        assert_eq!(
            new_branch_spacer_text("┊╭┄", ExtensionDirection::Above),
            "┊│"
        );
    }

    #[test]
    fn a_new_branch_above_a_nested_branch_keeps_the_outer_lanes_in_its_spacer() {
        assert_eq!(
            new_branch_spacer_text("┊┊├┄", ExtensionDirection::Above),
            "┊┊│"
        );
    }

    #[test]
    fn a_stacks_top_branch_joins_the_stack_under_a_new_branch() {
        let connector = stacked_under_new_branch(&[Span::raw("┊"), Span::raw("╭┄")]);
        let text = connector
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(text, "┊├┄");
    }

    #[test]
    fn extends_every_connector_shape_emitted_by_status_output() {
        let cases = [
            ("╭┄", "  ", "│ "),
            ("├╯", "││", "│ "),
            ("┊", "┊", "┊"),
            ("┊╭┄", "┊  ", "┊│ "),
            ("┊├┄", "┊│ ", "┊│ "),
            ("┊╭┄┄", "┊   ", "┊│  "),
            ("┊-", "┊│", "┊│"),
            ("┊┊", "┊┊", "┊┊"),
            ("┊│", "┊│", "┊│"),
            ("┊●   ", "┊│   ", "┊│   "),
            ("┊◐   ", "┊│   ", "┊│   "),
            ("┊   ", "┊   ", "┊   "),
            ("┊  ╭┄", "┊    ", "┊  │ "),
            ("┊  │", "┊  │", "┊  │"),
            ("┊  │ ", "┊  │ ", "┊  │ "),
            ("┊│     ", "┊│     ", "┊│     "),
        ];

        for (input, expected_above, expected_below) in cases {
            assert_eq!(
                extend_connector_text(input, ExtensionDirection::Above),
                expected_above,
                "above for {input:?}"
            );
            assert_eq!(
                extend_connector_text(input, ExtensionDirection::Below),
                expected_below,
                "below for {input:?}"
            );
            assert_eq!(
                input.chars().count(),
                expected_above.chars().count(),
                "above width for {input:?}"
            );
            assert_eq!(
                input.chars().count(),
                expected_below.chars().count(),
                "below width for {input:?}"
            );
        }
    }

    #[test]
    fn extends_single_character_connectors() {
        let cases = [
            ('┊', '┊', '┊'),
            ('│', '│', '│'),
            ('●', '│', '│'),
            ('◐', '│', '│'),
            ('├', '│', '│'),
            ('-', '│', '│'),
            ('╭', ' ', '│'),
            ('╯', '│', ' '),
            ('┴', '│', ' '),
            ('┄', ' ', ' '),
        ];

        for (input, expected_above, expected_below) in cases {
            assert_eq!(
                extend_connector_text(&input.to_string(), ExtensionDirection::Above),
                expected_above.to_string(),
                "above for {input:?}"
            );
            assert_eq!(
                extend_connector_text(&input.to_string(), ExtensionDirection::Below),
                expected_below.to_string(),
                "below for {input:?}"
            );
        }
    }

    #[test]
    fn empty_input_becomes_two_spaces() {
        assert_eq!(
            extend_connector_text("", ExtensionDirection::Above),
            "  ".to_string()
        );
        assert_eq!(
            extend_connector_text("", ExtensionDirection::Below),
            "  ".to_string()
        );
    }

    #[test]
    fn extends_terminal_merge_base_connector_above_and_below() {
        let connector = "┴ ";

        assert_eq!(
            extend_connector_text(connector, ExtensionDirection::Above),
            "│ ".to_string()
        );
        assert_eq!(
            extend_connector_text(connector, ExtensionDirection::Below),
            "  ".to_string()
        );
    }

    #[test]
    fn span_based_api_flattens_and_extends_connector() {
        let input = vec![Span::raw("┊"), Span::raw("●"), Span::raw("   ")];

        let mut above = Vec::<Span<'_>>::new();
        extend_connector_spans(&input, ExtensionDirection::Above, &mut above);

        let mut below = Vec::<Span<'_>>::new();
        extend_connector_spans(&input, ExtensionDirection::Below, &mut below);

        assert_eq!(above.len(), 1);
        assert_eq!(below.len(), 1);
        assert_eq!(above[0].content.as_ref(), "┊│   ");
        assert_eq!(below[0].content.as_ref(), "┊│   ");
    }
}

use ratatui::text::Span;

/// Direction in which to extend a graph connector line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExtensionDirection {
    /// Build the connector for a synthetic line rendered above the source line.
    Above,
    /// Build the connector for a synthetic line rendered below the source line.
    Below,
}

/// Build a connector extension line for the provided connector spans.
///
/// The returned connector preserves the input connector width (character count),
/// and applies graph-continuity rules per connector glyph.
///
/// If the input connector is empty, this returns two spaces (`"  "`) so
/// extension-only lines still align with regular graph rows.
pub(super) fn extend_connector_spans<E>(
    connector: &[Span<'_>],
    direction: ExtensionDirection,
    out: &mut E,
) where
    E: Extend<Span<'static>>,
{
    let connector_text = connector
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    out.extend([Span::raw(extend_connector_text(&connector_text, direction))]);
}

/// Build a connector extension string for a connector prefix.
///
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

/// Map one connector glyph to its extension glyph in the chosen direction.
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
    use super::{ExtensionDirection, extend_connector_spans, extend_connector_text};
    use ratatui::text::Span;

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

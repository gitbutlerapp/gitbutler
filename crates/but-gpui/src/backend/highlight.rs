//! Syntax highlighting for diff lines.
//!
//! Lines are tagged with a handful of token kinds rather than colours, so the UI can
//! colour them from its own light or dark palette. syntect does the parsing, with bat's
//! syntaxes from `two-face`, which add TypeScript, Svelte, TOML and more to syntect's own;
//! its theme here only maps scopes to token kinds, smuggled through the red channel.

use std::ops::Range;
use std::str::FromStr as _;
use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Comment,
    String,
    Constant,
    Keyword,
    Function,
    Type,
    Tag,
}

const TOKENS: [(Token, &str); 7] = [
    (Token::Comment, "comment"),
    (Token::String, "string, markup.raw"),
    (
        Token::Constant,
        "constant.numeric, constant.language, constant.character, constant.other",
    ),
    (
        Token::Keyword,
        "keyword, storage.modifier, storage.type.function, variable.language",
    ),
    (
        Token::Function,
        "entity.name.function, support.function, meta.function-call variable.function",
    ),
    (
        Token::Type,
        "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, support.type, support.class, storage.type",
    ),
    (
        Token::Tag,
        "entity.name.tag, entity.other.attribute-name, markup.heading",
    ),
];

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| Theme {
        settings: ThemeSettings {
            foreground: Some(Color::BLACK),
            ..Default::default()
        },
        scopes: TOKENS
            .iter()
            .enumerate()
            .filter_map(|(ix, (_, selector))| {
                Some(ThemeItem {
                    scope: ScopeSelectors::from_str(selector).ok()?,
                    style: StyleModifier {
                        foreground: Some(Color {
                            r: ix as u8 + 1,
                            g: 0,
                            b: 0,
                            a: 255,
                        }),
                        ..Default::default()
                    },
                })
            })
            .collect(),
        ..Default::default()
    })
}

/// Highlights consecutive lines of one file.
pub struct Highlighter(HighlightLines<'static>);

impl Highlighter {
    pub fn for_path(path: &str) -> Option<Self> {
        let set = syntaxes();
        let name = path.rsplit('/').next().unwrap_or(path);
        let extension = name.rsplit_once('.').map_or(name, |(_, ext)| ext);
        // Extensions no syntax claims, read as one that has a syntax.
        let extension = match extension {
            "mjs" | "cjs" => "js",
            "jsonc" | "hjson" => "json",
            "mdx" => "md",
            other => other,
        };
        let syntax = set
            .find_syntax_by_extension(extension)
            .or_else(|| set.find_syntax_by_extension(name))?;
        Some(Self(HighlightLines::new(syntax, theme())))
    }

    /// Token spans of `line` as byte ranges, continuing the state of earlier lines.
    pub fn line(&mut self, line: &str) -> Vec<(Range<usize>, Token)> {
        let with_newline = format!("{line}\n");
        let Ok(regions) = self.0.highlight_line(&with_newline, syntaxes()) else {
            return Vec::new();
        };
        let mut spans: Vec<(Range<usize>, Token)> = Vec::new();
        let mut offset = 0;
        for (style, text) in regions {
            let start = offset;
            offset += text.len();
            let end = offset.min(line.len());
            let Some(token) = style
                .foreground
                .r
                .checked_sub(1)
                .and_then(|ix| TOKENS.get(ix as usize))
                .map(|(token, _)| *token)
            else {
                continue;
            };
            if start >= end {
                continue;
            }
            match spans.last_mut() {
                Some((range, last)) if *last == token && range.end == start => range.end = end,
                _ => spans.push((start..end, token)),
            }
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::Highlighter;

    #[test]
    fn the_repositorys_own_file_types_are_highlighted() {
        for path in [
            "a.rs", "a.ts", "a.tsx", "a.svelte", "a.js", "a.mjs", "a.cjs", "a.css", "a.scss",
            "a.json", "a.jsonc", "a.toml", "a.yml", "a.md", "a.mdx", "a.sh", "a.html",
        ] {
            assert!(Highlighter::for_path(path).is_some(), "{path} has a syntax");
        }
    }
}

//! Markdown, as pull request descriptions are written, drawn as formatted text.
//!
//! Only what can't reach outside the app is kept. Links open only to the web and to mail,
//! with where they go shown on hover, as a link's words can say anything; images are never
//! fetched, so a description can't learn you've read it, and show their words instead; HTML
//! is never interpreted, only its text kept. What's past [`MAX_BYTES`] isn't parsed, and
//! nesting past [`MAX_DEPTH`] steps in no further.
//!
//! Emphasis shows in the medium weight, not slanted: the app embeds no italic face, and
//! asking for one could pick a variable font installed on the machine, which is slow to
//! draw (see the crate's AGENTS.md).

use std::ops::Range;

use gpui::{
    AnyElement, App, Font, FontWeight, Hsla, InteractiveText, IntoElement, ParentElement,
    SharedString, StrikethroughStyle, Styled, StyledText, TextRun, UnderlineStyle, div, px,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::{Tooltip, h_flex, v_flex};
use crate::theme::Theme;

/// How much of a description is read; the rest is left out, and said to be.
const MAX_BYTES: usize = 64 * 1024;

/// How far lists and quotes step in at most.
const MAX_DEPTH: usize = 6;

/// One block of a description, as drawn: its kind, how far in it steps, and its text with
/// the styles and links over it.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub kind: Kind,
    /// Lists and quotes it's in.
    pub depth: usize,
    /// Inside a quote.
    pub quoted: bool,
    /// The outermost list it's in, counted from 1, or 0 outside lists: a list's items sit
    /// together, and another list stands apart from it.
    pub list: usize,
    pub text: String,
    pub styles: Vec<(Range<usize>, Style)>,
    /// Where each link goes, by the text it covers; only addresses that are safe to open.
    pub links: Vec<(Range<usize>, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Paragraph,
    Heading(u8),
    Code,
    /// A list item, after its marker: "•", "3." or a box.
    Item(String),
    Rule,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}

/// Where a link may go when clicked: the web, or mail. Anything else, as a file, another
/// app's address or script, or a path the forge resolves against the repository, stays text.
fn safe_link(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

/// HTML's text, without its tags or comments. `in_comment` carries a comment left open into
/// the next piece, as an HTML block comes a line at a time; an unclosed tag runs to the end.
fn html_text(html: &str, in_comment: &mut bool) -> String {
    let mut text = String::new();
    let mut rest = html;
    loop {
        if *in_comment {
            let Some(end) = rest.find("-->") else {
                return text;
            };
            *in_comment = false;
            rest = &rest[end + 3..];
        }
        let Some(start) = rest.find('<') else {
            text.push_str(rest);
            return text;
        };
        text.push_str(&rest[..start]);
        let tag = &rest[start..];
        match tag.strip_prefix("<!--") {
            Some(comment) => {
                *in_comment = true;
                rest = comment;
            }
            None => rest = tag.find('>').map_or("", |end| &tag[end + 1..]),
        }
    }
}

/// `markdown` as blocks to draw.
pub fn parse(markdown: &str) -> Vec<Block> {
    let mut end = markdown.len().min(MAX_BYTES);
    while !markdown.is_char_boundary(end) {
        end -= 1;
    }
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut blocks = Vec::new();
    let mut current: Option<Block> = None;
    let mut style = Style::default();
    // Each list in, by its next number, or `None` for bullets.
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut outermost = 0;
    let mut quotes = 0;
    let mut in_comment = false;
    let mut link: Option<(usize, Option<String>)> = None;
    let mut image: Option<(String, Option<String>)> = None;
    let depth = |lists: &Vec<Option<u64>>, quotes: usize| (lists.len() + quotes).min(MAX_DEPTH);

    let open = |kind: Kind, lists: &Vec<Option<u64>>, quotes: usize, outermost: usize| Block {
        kind,
        depth: depth(lists, quotes),
        quoted: quotes > 0,
        list: if lists.is_empty() { 0 } else { outermost },
        text: String::new(),
        styles: Vec::new(),
        links: Vec::new(),
    };
    fn push(block: &mut Block, text: &str, style: Style) {
        let start = block.text.len();
        block.text.push_str(text);
        if style != Style::default() && !text.is_empty() {
            block.styles.push((start..block.text.len(), style));
        }
    }
    fn close(blocks: &mut Vec<Block>, current: &mut Option<Block>) {
        if let Some(block) = current.take()
            && (!block.text.trim().is_empty() || matches!(block.kind, Kind::Item(_)))
        {
            blocks.push(block);
        }
    }

    for event in Parser::new_ext(&markdown[..end], options) {
        // Text inside an image is its words, shown in its place.
        if let Some((alt, _)) = &mut image {
            match event {
                Event::Text(text) | Event::Code(text) => alt.push_str(&text),
                Event::End(TagEnd::Image) => {
                    let (alt, url) = image.take().expect("in an image");
                    let block = current
                        .get_or_insert_with(|| open(Kind::Paragraph, &lists, quotes, outermost));
                    let words = match alt.trim() {
                        "" => "Image".to_owned(),
                        alt => format!("Image: {alt}"),
                    };
                    let start = block.text.len();
                    push(block, &words, style);
                    if let Some(url) = url {
                        block.links.push((start..block.text.len(), url));
                    }
                }
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::Paragraph) => {
                // A loose list item's first paragraph is the item's own text.
                let in_empty_item = matches!(&current,
                    Some(Block { kind: Kind::Item(_), text, .. }) if text.is_empty());
                if !in_empty_item {
                    close(&mut blocks, &mut current);
                    current = Some(open(Kind::Paragraph, &lists, quotes, outermost));
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                close(&mut blocks, &mut current);
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    _ => 4,
                };
                current = Some(open(Kind::Heading(level), &lists, quotes, outermost));
            }
            Event::Start(Tag::CodeBlock(_)) => {
                close(&mut blocks, &mut current);
                current = Some(open(Kind::Code, &lists, quotes, outermost));
            }
            Event::Start(Tag::BlockQuote(_)) => {
                close(&mut blocks, &mut current);
                quotes += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                close(&mut blocks, &mut current);
                quotes = quotes.saturating_sub(1);
            }
            Event::Start(Tag::List(start)) => {
                close(&mut blocks, &mut current);
                if lists.is_empty() {
                    outermost += 1;
                }
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                close(&mut blocks, &mut current);
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                close(&mut blocks, &mut current);
                let marker = match lists.last_mut() {
                    Some(Some(next)) => {
                        *next += 1;
                        format!("{}.", *next - 1)
                    }
                    _ => "•".to_owned(),
                };
                // The item steps in as far as its list, not one further.
                let mut item = open(Kind::Item(marker), &lists, quotes, outermost);
                item.depth = item.depth.saturating_sub(1);
                current = Some(item);
            }
            Event::TaskListMarker(done) => {
                if let Some(Block {
                    kind: Kind::Item(marker),
                    ..
                }) = &mut current
                {
                    *marker = if done { "☑" } else { "☐" }.to_owned();
                }
            }
            Event::End(
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item,
            ) => close(&mut blocks, &mut current),
            Event::Rule => {
                close(&mut blocks, &mut current);
                blocks.push(open(Kind::Rule, &lists, quotes, outermost));
            }
            Event::Start(Tag::Strong) => style.bold = true,
            Event::End(TagEnd::Strong) => style.bold = false,
            Event::Start(Tag::Emphasis) => style.italic = true,
            Event::End(TagEnd::Emphasis) => style.italic = false,
            Event::Start(Tag::Strikethrough) => style.strike = true,
            Event::End(TagEnd::Strikethrough) => style.strike = false,
            Event::Start(Tag::Link { dest_url, .. }) => {
                let at = current.as_ref().map_or(0, |block| block.text.len());
                link = Some((at, safe_link(&dest_url).then(|| dest_url.trim().to_owned())));
            }
            Event::End(TagEnd::Link) => {
                if let (Some((start, Some(url))), Some(block)) = (link.take(), &mut current)
                    && start < block.text.len()
                {
                    block.links.push((start..block.text.len(), url));
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                image = Some((
                    String::new(),
                    safe_link(&dest_url).then(|| dest_url.trim().to_owned()),
                ));
            }
            Event::Text(text) => {
                let block =
                    current.get_or_insert_with(|| open(Kind::Paragraph, &lists, quotes, outermost));
                push(block, &text, style);
            }
            Event::Code(code) => {
                let block =
                    current.get_or_insert_with(|| open(Kind::Paragraph, &lists, quotes, outermost));
                push(
                    block,
                    &code,
                    Style {
                        code: true,
                        ..style
                    },
                );
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                let text = html_text(&html, &mut in_comment);
                if !text.trim().is_empty() {
                    let block = current
                        .get_or_insert_with(|| open(Kind::Paragraph, &lists, quotes, outermost));
                    push(block, text.trim_end_matches('\n'), style);
                }
            }
            Event::SoftBreak => {
                if let Some(block) = &mut current {
                    push(block, " ", style);
                }
            }
            Event::HardBreak => {
                if let Some(block) = &mut current {
                    push(block, "\n", style);
                }
            }
            _ => {}
        }
    }
    close(&mut blocks, &mut current);
    for block in &mut blocks {
        if block.kind == Kind::Code {
            block.text.truncate(block.text.trim_end_matches('\n').len());
        }
    }
    if end < markdown.len() {
        let mut more = open(Kind::Paragraph, &Vec::new(), 0, 0);
        push(
            &mut more,
            "The rest of the description is too long to show here.",
            Style {
                italic: true,
                ..Style::default()
            },
        );
        blocks.push(more);
    }
    blocks
}

/// `blocks` drawn as text, `id` telling apart the links of one description from another's.
pub fn render(id: &str, blocks: &[Block], theme: &Theme) -> AnyElement {
    v_flex()
        .children(blocks.iter().enumerate().map(|(ix, block)| {
            // A list's items sit together; other blocks, and other lists, stand apart.
            let item = |block: &Block| matches!(block.kind, Kind::Item(_));
            let space = match ix.checked_sub(1).map(|before| &blocks[before]) {
                None => 0.,
                Some(before) if item(before) && item(block) && before.list == block.list => 2.,
                Some(_) => 8.,
            };
            let el = match &block.kind {
                Kind::Rule => div()
                    .h(px(1.))
                    .my(px(4.))
                    .bg(theme.border_3)
                    .into_any_element(),
                Kind::Code => div()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .bg(theme.fill_gray.opacity(0.08))
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .child(text(id, ix, block, theme, Base::CODE))
                    .into_any_element(),
                Kind::Heading(level) => div()
                    .pt(px(4.))
                    .text_size(px(match level {
                        1 => 17.,
                        2 => 15.,
                        _ => 14.,
                    }))
                    .line_height(px(22.))
                    .child(text(id, ix, block, theme, Base::HEADING))
                    .into_any_element(),
                Kind::Item(marker) => h_flex()
                    .items_start()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_none()
                            .min_w(px(14.))
                            .text_color(theme.text_2)
                            .child(SharedString::from(marker.clone())),
                    )
                    .child(div().flex_1().min_w(px(0.)).child(text(
                        id,
                        ix,
                        block,
                        theme,
                        Base::quoted(block.quoted),
                    )))
                    .into_any_element(),
                Kind::Paragraph => text(id, ix, block, theme, Base::quoted(block.quoted)),
            };
            let indented = div()
                .mt(px(space))
                .pl(px(block.depth as f32 * 16.))
                .text_size(px(13.))
                .line_height(px(20.))
                .child(el);
            match block.quoted {
                true => indented
                    .text_color(theme.text_2)
                    .border_l_2()
                    .border_color(theme.border_2)
                    .ml(px(2.))
                    .pl(px(10. + block.depth as f32 * 16.))
                    .into_any_element(),
                false => indented.into_any_element(),
            }
        }))
        .into_any_element()
}

/// How a block's text is drawn before its own styles. Each run sets its font and colour,
/// so what the block's element sets wouldn't reach it.
#[derive(Clone, Copy)]
struct Base {
    mono: bool,
    heading: bool,
    quoted: bool,
}

impl Base {
    const CODE: Base = Base {
        mono: true,
        heading: false,
        quoted: false,
    };
    const HEADING: Base = Base {
        mono: false,
        heading: true,
        quoted: false,
    };

    fn quoted(quoted: bool) -> Base {
        Base {
            mono: false,
            heading: false,
            quoted,
        }
    }
}

/// A block's text in its styles, its links clickable and naming where they go on hover.
fn text(id: &str, ix: usize, block: &Block, theme: &Theme, base: Base) -> AnyElement {
    let font = |code: bool| match code || base.mono {
        true => gpui::font(theme.mono_font.clone()),
        false => gpui::font(theme.ui_font.clone()),
    };
    let color: Hsla = match base.quoted {
        true => theme.text_2,
        false => theme.text_1,
    };
    // A run for every stretch between where a style or link starts or ends.
    let mut cuts: Vec<usize> = vec![0, block.text.len()];
    for (range, _) in &block.styles {
        cuts.extend([range.start, range.end]);
    }
    for (range, _) in &block.links {
        cuts.extend([range.start, range.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let runs = cuts
        .windows(2)
        .filter(|cut| cut[0] < cut[1])
        .map(|cut| {
            let at = cut[0];
            let style = (block.styles.iter())
                .filter(|(range, _)| range.contains(&at))
                .fold(Style::default(), |all, (_, style)| Style {
                    bold: all.bold || style.bold,
                    italic: all.italic || style.italic,
                    strike: all.strike || style.strike,
                    code: all.code || style.code,
                });
            let linked = block.links.iter().any(|(range, _)| range.contains(&at));
            let weight = match (style.bold || base.heading, style.italic) {
                (true, _) => FontWeight::SEMIBOLD,
                (false, true) => FontWeight::MEDIUM,
                (false, false) => FontWeight::NORMAL,
            };
            TextRun {
                len: cut[1] - cut[0],
                font: Font {
                    weight,
                    ..font(style.code)
                },
                color: match linked {
                    true => theme.blue,
                    false => color,
                },
                background_color: style.code.then(|| theme.fill_gray.opacity(0.12)),
                underline: linked.then(|| UnderlineStyle {
                    thickness: px(1.),
                    color: Some(theme.blue.opacity(0.4)),
                    wavy: false,
                }),
                strikethrough: style.strike.then(|| StrikethroughStyle {
                    thickness: px(1.),
                    color: None,
                }),
            }
        })
        .collect();
    let styled = StyledText::new(SharedString::from(block.text.clone())).with_runs(runs);
    if block.links.is_empty() {
        return styled.into_any_element();
    }
    let ranges: Vec<_> = block.links.iter().map(|(range, _)| range.clone()).collect();
    let urls: Vec<_> = block.links.iter().map(|(_, url)| url.clone()).collect();
    let tips = block.links.clone();
    InteractiveText::new(SharedString::from(format!("{id}-md-{ix}")), styled)
        .on_click(ranges, move |link, _, cx: &mut App| {
            cx.open_url(&urls[link])
        })
        .tooltip(move |at, window, cx| {
            let (_, url) = tips.iter().find(|(range, _)| range.contains(&at))?;
            Some(Tooltip::text(url.clone())(window, cx))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(blocks: &[Block]) -> Vec<(&Kind, &str)> {
        blocks
            .iter()
            .map(|block| (&block.kind, block.text.as_str()))
            .collect()
    }

    #[test]
    fn blocks_and_inline_styles() {
        let blocks = parse("# Title\n\nSome **bold** and `code`.\n\n```\nfn x() {}\n```\n\n---\n");
        assert_eq!(
            texts(&blocks),
            [
                (&Kind::Heading(1), "Title"),
                (&Kind::Paragraph, "Some bold and code."),
                (&Kind::Code, "fn x() {}"),
                (&Kind::Rule, ""),
            ],
            "each block once, its text without the markup"
        );
        assert_eq!(
            blocks[1].styles,
            [
                (
                    5..9,
                    Style {
                        bold: true,
                        ..Style::default()
                    }
                ),
                (
                    14..18,
                    Style {
                        code: true,
                        ..Style::default()
                    }
                ),
            ],
            "the styles cover the words they mark"
        );
    }

    #[test]
    fn only_web_and_mail_links_open() {
        let blocks = parse(
            "[web](https://gitbutler.com) [mail](mailto:a@b.c) [script](javascript:alert(1)) \
             [file](file:///etc/passwd) [app](vscode://x) [path](../README.md)",
        );
        let urls: Vec<_> = blocks[0]
            .links
            .iter()
            .map(|(_, url)| url.as_str())
            .collect();
        assert_eq!(
            urls,
            ["https://gitbutler.com", "mailto:a@b.c"],
            "any other address stays plain text"
        );
        assert_eq!(
            blocks[0].text, "web mail script file app path",
            "every link keeps its words"
        );
    }

    #[test]
    fn images_show_their_words_and_are_never_loaded() {
        let blocks = parse("![a chart](https://example.com/c.png) ![](data:image/png;base64,AAAA)");
        assert_eq!(
            blocks[0].text, "Image: a chart Image",
            "an image's words stand in for it"
        );
        let urls: Vec<_> = blocks[0]
            .links
            .iter()
            .map(|(_, url)| url.as_str())
            .collect();
        assert_eq!(
            urls,
            ["https://example.com/c.png"],
            "a web image can be opened in the browser by its words; nothing else links"
        );
    }

    #[test]
    fn html_shows_as_its_text_and_comments_go() {
        let blocks = parse(
            "<!-- template -->\n\n<details><summary>Logs</summary>\n\nok\n\n</details>\n\n\
             Hi <b>there</b> <img src=\"https://t.co/p.gif\">",
        );
        let text: Vec<_> = blocks.iter().map(|block| block.text.as_str()).collect();
        assert_eq!(
            text,
            ["Logs", "ok", "Hi there "],
            "tags go, their text stays"
        );
    }

    #[test]
    fn lists_number_nest_and_tick() {
        let blocks = parse("1. one\n2. two\n   - inner\n\n- [x] done\n- [ ] to do\n");
        assert_eq!(
            (blocks.iter())
                .map(|block| (block.kind.clone(), block.depth, block.text.as_str()))
                .collect::<Vec<_>>(),
            [
                (Kind::Item("1.".into()), 0, "one"),
                (Kind::Item("2.".into()), 0, "two"),
                (Kind::Item("•".into()), 1, "inner"),
                (Kind::Item("☑".into()), 0, "done"),
                (Kind::Item("☐".into()), 0, "to do"),
            ],
            "numbers count up, nested lists step in, tasks show their box"
        );
        assert_eq!(
            blocks.iter().map(|block| block.list).collect::<Vec<_>>(),
            [1, 1, 1, 2, 2],
            "a nested list is part of its outer one; the next list is another"
        );
    }

    #[test]
    fn template_comments_and_the_blank_lines_they_leave_go() {
        let body = "<!-- Describe the change -->\r\n\r\nFixes a crash.\n\n<!--\nIf it fixes an issue,\nlink it.\n-->\n\n<!-- Screenshots -->\n\n\nMore.\n<!-- unclosed";
        let text: Vec<_> = parse(body).into_iter().map(|block| block.text).collect();
        assert_eq!(
            text,
            ["Fixes a crash.", "More."],
            "only what was written shows"
        );
    }

    #[test]
    fn deep_nesting_and_huge_input_are_bounded() {
        let deep = (0..50)
            .map(|n| format!("{}- level {n}\n", "  ".repeat(n)))
            .collect::<String>();
        assert!(
            parse(&deep).iter().all(|block| block.depth <= MAX_DEPTH),
            "lists past the limit step in no further"
        );
        let huge = "word ".repeat(MAX_BYTES);
        let blocks = parse(&huge);
        assert!(
            blocks
                .last()
                .is_some_and(|block| block.text.contains("too long")),
            "what's past the limit isn't parsed, and says so"
        );
    }
}

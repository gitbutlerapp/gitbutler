//! Markdown, as pull request descriptions and READMEs are written, drawn as formatted text.
//!
//! Only what can't reach outside the app is kept. Links open only to the web and to mail,
//! with where they go shown on hover, as a link's words can say anything. A description's
//! images are never fetched, so it can't learn you've read it, and show their words instead;
//! a README's ([`parse_showing_images`]) are listed for the caller to load. HTML is read only
//! as far as a README lays itself out with it: headings, paragraphs and centring, bold,
//! emphasis and code, links, line breaks and images; other tags leave only their text. What's
//! past [`MAX_BYTES`] isn't parsed, and nesting past [`MAX_DEPTH`] steps in no further.
//!
//! Emphasis shows in the medium weight, not slanted: the app embeds no italic face, and
//! asking for one could pick a variable font installed on the machine, which is slow to
//! draw (see the crate's AGENTS.md).

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Font, FontWeight, Hsla, ImageSource, InteractiveElement, InteractiveText,
    IntoElement, ParentElement, RenderImage, SharedString, StatefulInteractiveElement,
    StrikethroughStyle, Styled, StyledText, TextRun, UnderlineStyle, div, img,
    prelude::FluentBuilder, px, relative,
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
    /// Inside HTML that centres it, as a README's heading and badges are.
    pub centered: bool,
    /// An [`Kind::Images`] block's images, in a row.
    pub images: Vec<Image>,
}

/// An image to show, by where it's loaded from.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// A web address, or a path in the repository from its top, which the caller reads.
    pub src: String,
    pub alt: String,
    /// The link it's in, which a click opens.
    pub link: Option<String>,
    pub width: Option<Width>,
}

/// How wide HTML asks an image to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Width {
    Px(f32),
    /// Of the text's width.
    Fraction(f32),
}

/// Every image `blocks` show, each once.
pub fn image_sources(blocks: &[Block]) -> Vec<String> {
    let mut sources: Vec<String> = Vec::new();
    for image in blocks.iter().flat_map(|block| &block.images) {
        if !sources.contains(&image.src) {
            sources.push(image.src.clone());
        }
    }
    sources
}

/// Where an image is loaded from, when it can be: the web, or a path in the repository,
/// which can't step out of it.
fn image_source(url: &str) -> Option<String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return Some(url.to_owned());
    }
    let path = url
        .split(['?', '#'])
        .next()?
        .trim_start_matches("./")
        .trim_start_matches('/');
    let inside =
        !path.is_empty() && !path.contains(':') && !path.split('/').any(|part| part == "..");
    inside.then(|| path.to_owned())
}

/// The width an `<img>`'s `width` asks for: pixels, or a share of the text's width.
fn html_width(value: &str) -> Option<Width> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let percent: f32 = percent.trim().parse().ok()?;
        return Some(Width::Fraction((percent / 100.).clamp(0.01, 1.)));
    }
    let px: f32 = value.trim_end_matches("px").trim().parse().ok()?;
    (px > 0.).then_some(Width::Px(px))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Paragraph,
    Heading(u8),
    Code,
    /// A list item, after its marker: "•", "3." or a box.
    Item(String),
    Rule,
    /// Images with no words between them, as a row of badges or a screenshot.
    Images,
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

/// `html`'s text onto `text`, its common entities as the characters they stand for, as a
/// README spaces out its links with `&nbsp;`.
fn push_decoded(text: &mut String, html: &str) {
    const ENTITIES: [(&str, char); 7] = [
        ("&nbsp;", ' '),
        ("&amp;", '&'),
        ("&lt;", '<'),
        ("&gt;", '>'),
        ("&quot;", '"'),
        ("&#39;", '\''),
        ("&apos;", '\''),
    ];
    let mut rest = html;
    while let Some(at) = rest.find('&') {
        text.push_str(&rest[..at]);
        let tail = &rest[at..];
        match ENTITIES.iter().find(|(name, _)| tail.starts_with(name)) {
            Some((name, ch)) => {
                text.push(*ch);
                rest = &tail[name.len()..];
            }
            None => {
                text.push('&');
                rest = &tail[1..];
            }
        }
    }
    text.push_str(rest);
}

/// A piece of HTML.
#[derive(Debug, PartialEq)]
enum Html {
    Text(String),
    /// A tag opening, by its lowercased name, with its attributes, their values decoded.
    Open(String, Vec<(String, String)>),
    Close(String),
}

/// `html` in pieces, comments left out. `in_comment` carries a comment left open into the
/// next piece, as an HTML block comes a line at a time; an unclosed tag runs to the end.
fn html_pieces(html: &str, in_comment: &mut bool) -> Vec<Html> {
    let mut pieces = Vec::new();
    let text = |pieces: &mut Vec<Html>, raw: &str| {
        if !raw.is_empty() {
            let mut text = String::new();
            push_decoded(&mut text, raw);
            pieces.push(Html::Text(text));
        }
    };
    let mut rest = html;
    loop {
        if *in_comment {
            let Some(end) = rest.find("-->") else {
                return pieces;
            };
            *in_comment = false;
            rest = &rest[end + 3..];
        }
        let Some(start) = rest.find('<') else {
            text(&mut pieces, rest);
            return pieces;
        };
        text(&mut pieces, &rest[..start]);
        let tag = &rest[start..];
        if let Some(comment) = tag.strip_prefix("<!--") {
            *in_comment = true;
            rest = comment;
            continue;
        }
        let Some(end) = tag.find('>') else {
            return pieces;
        };
        let inner = tag[1..end].trim().trim_end_matches('/');
        rest = &tag[end + 1..];
        match inner.strip_prefix('/') {
            Some(name) => pieces.push(Html::Close(name.trim().to_ascii_lowercase())),
            None => {
                let (name, attributes) = tag_parts(inner);
                if !name.is_empty() {
                    pieces.push(Html::Open(name, attributes));
                }
            }
        }
    }
}

/// A tag's name, lowercased, and its attributes, as `img src="a.png" width=100`.
fn tag_parts(inner: &str) -> (String, Vec<(String, String)>) {
    let inner = inner.trim();
    let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
    let name = inner[..name_end].to_ascii_lowercase();
    let mut attributes = Vec::new();
    let mut rest = inner[name_end..].trim_start();
    while !rest.is_empty() {
        let key_end = (rest.find(|c: char| c == '=' || c.is_whitespace())).unwrap_or(rest.len());
        let key = rest[..key_end].to_ascii_lowercase();
        rest = rest[key_end..].trim_start();
        let mut value = String::new();
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let (raw, left) = match after.chars().next() {
                Some(quote @ ('"' | '\'')) => {
                    let body = &after[1..];
                    let close = body.find(quote).unwrap_or(body.len());
                    (&body[..close], body.get(close + 1..).unwrap_or(""))
                }
                _ => {
                    let end = after.find(char::is_whitespace).unwrap_or(after.len());
                    (&after[..end], &after[end..])
                }
            };
            push_decoded(&mut value, raw);
            rest = left.trim_start();
        }
        if !key.is_empty() {
            attributes.push((key, value));
        }
    }
    (name, attributes)
}

fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    (attributes.iter())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// HTML runs its spaces and line breaks together.
fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (ix, word) in text.split_whitespace().enumerate() {
        if ix > 0 {
            out.push(' ');
        }
        out.push_str(word);
    }
    if text.starts_with(char::is_whitespace) {
        out.insert(0, ' ');
    }
    if text.ends_with(char::is_whitespace) && !out.ends_with(' ') {
        out.push(' ');
    }
    out
}

/// `markdown` as blocks to draw, its images by their words.
pub fn parse(markdown: &str) -> Vec<Block> {
    blocks(markdown, false)
}

/// `markdown` as blocks to draw, its images as [`Kind::Images`] blocks for the caller to
/// load, as a README's are.
pub fn parse_showing_images(markdown: &str) -> Vec<Block> {
    blocks(markdown, true)
}

fn blocks(markdown: &str, show_images: bool) -> Vec<Block> {
    let mut end = markdown.len().min(MAX_BYTES);
    while !markdown.is_char_boundary(end) {
        end -= 1;
    }
    let options =
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES;
    let mut blocks = Vec::new();
    let mut current: Option<Block> = None;
    let mut style = Style::default();
    // Each list in, by its next number, or `None` for bullets.
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut outermost = 0;
    let mut quotes = 0;
    let mut in_comment = false;
    let mut link: Option<(usize, Option<String>)> = None;
    // Its words, and where it's from.
    let mut image: Option<(String, String)> = None;
    // The HTML tags that make blocks, open around here, each with whether it centres them.
    let mut html_open: Vec<(String, bool)> = Vec::new();
    // How many of those centre what's in them.
    let mut centring = 0;
    let centered = Cell::new(false);
    // A block of HTML comes a line at a time, and lines apart read as one space.
    let mut html_gap = false;
    // In a table row: whether it's the header, and how many of its cells came before.
    let (mut in_head, mut cells) = (false, 0);
    let depth = |lists: &Vec<Option<u64>>, quotes: usize| (lists.len() + quotes).min(MAX_DEPTH);

    let open = |kind: Kind, lists: &Vec<Option<u64>>, quotes: usize, outermost: usize| Block {
        kind,
        depth: depth(lists, quotes),
        quoted: quotes > 0,
        list: if lists.is_empty() { 0 } else { outermost },
        text: String::new(),
        styles: Vec::new(),
        links: Vec::new(),
        centered: centered.get(),
        images: Vec::new(),
    };
    fn push(block: &mut Block, text: &str, style: Style) {
        let start = block.text.len();
        block.text.push_str(text);
        if style != Style::default() && !text.is_empty() {
            block.styles.push((start..block.text.len(), style));
        }
    }
    /// Drop the spaces at the end of `block`'s text, which HTML shows nothing for, and the
    /// styles and links over only them, so none ends past the text.
    fn trim_spaces(block: &mut Block) {
        let end = block.text.trim_end_matches(' ').len();
        block.text.truncate(end);
        let ranges = (block.styles.iter_mut().map(|(range, _)| range))
            .chain(block.links.iter_mut().map(|(range, _)| range));
        for range in ranges {
            range.end = range.end.min(end);
        }
        block.styles.retain(|(range, _)| !range.is_empty());
        block.links.retain(|(range, _)| !range.is_empty());
    }
    fn close(blocks: &mut Vec<Block>, current: &mut Option<Block>) {
        if let Some(block) = current.as_mut().filter(|block| block.kind != Kind::Code) {
            trim_spaces(block);
        }
        if let Some(block) = current.take()
            && (!block.text.trim().is_empty()
                || matches!(block.kind, Kind::Item(_))
                || !block.images.is_empty())
        {
            blocks.push(block);
        }
    }
    /// The block text goes in: the current one, unless that's a row of images, which ends
    /// where words start.
    fn text_block<'a>(
        blocks: &mut Vec<Block>,
        current: &'a mut Option<Block>,
        fresh: impl FnOnce() -> Block,
    ) -> &'a mut Block {
        if current
            .as_ref()
            .is_some_and(|block| block.kind == Kind::Images)
        {
            close(blocks, current);
        }
        current.get_or_insert_with(fresh)
    }
    /// Put `image` with the images before it when no words came between, or else, or when
    /// images aren't shown, its words in its place.
    fn put_image(
        current: &mut Option<Block>,
        image: Image,
        fresh: impl FnOnce() -> Block,
        style: Style,
        in_link: bool,
        shown: bool,
    ) {
        let wordless = |block: &Block| {
            block.text.trim().is_empty() && matches!(block.kind, Kind::Paragraph | Kind::Images)
        };
        if shown && current.as_ref().is_none_or(wordless) {
            let block = current.get_or_insert_with(fresh);
            block.kind = Kind::Images;
            block.text.clear();
            block.styles.clear();
            block.links.clear();
            block.images.push(image);
            return;
        }
        let block = current.get_or_insert_with(fresh);
        let words = match image.alt.trim() {
            "" => "Image".to_owned(),
            alt => format!("Image: {alt}"),
        };
        let start = block.text.len();
        push(block, &words, style);
        // Inside a link, only the link opens: a click opens every link it's in.
        if !in_link && safe_link(&image.src) {
            block.links.push((start..block.text.len(), image.src));
        }
    }

    for event in Parser::new_ext(&markdown[..end], options) {
        let html_line = matches!(event, Event::Html(_));
        // Text inside an image is its words, shown in its place.
        if let Some((alt, _)) = &mut image {
            match event {
                Event::Text(text) | Event::Code(text) => alt.push_str(&text),
                Event::End(TagEnd::Image) => {
                    let (alt, url) = image.take().expect("in an image");
                    let src = match show_images {
                        true => image_source(&url),
                        false => None,
                    };
                    let shown = src.is_some();
                    let image = Image {
                        src: src.unwrap_or(url),
                        alt,
                        link: link.as_ref().and_then(|(_, url)| url.clone()),
                        width: None,
                    };
                    put_image(
                        &mut current,
                        image,
                        || open(Kind::Paragraph, &lists, quotes, outermost),
                        style,
                        link.is_some(),
                        shown,
                    );
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
            Event::End(TagEnd::Paragraph) => {
                close(&mut blocks, &mut current);
                // An inline tag left open ends with its paragraph, not the document.
                style = Style {
                    bold: in_head,
                    ..Style::default()
                };
                link = None;
            }
            Event::End(
                TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item | TagEnd::HtmlBlock,
            ) => close(&mut blocks, &mut current),
            // Each table row is one line, its cells apart and the header bold, because
            // columns can't line up in wrapped text.
            Event::Start(row @ (Tag::TableHead | Tag::TableRow)) => {
                close(&mut blocks, &mut current);
                current = Some(open(Kind::Paragraph, &lists, quotes, outermost));
                (in_head, cells) = (matches!(row, Tag::TableHead), 0);
                style.bold = in_head;
            }
            // Every cell after the first, empty ones too, so the columns still line up.
            Event::Start(Tag::TableCell) => {
                if cells > 0
                    && let Some(block) = &mut current
                {
                    push(block, " · ", style);
                }
                cells += 1;
            }
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                (in_head, style.bold) = (false, false);
                close(&mut blocks, &mut current);
            }
            Event::Rule => {
                close(&mut blocks, &mut current);
                blocks.push(open(Kind::Rule, &lists, quotes, outermost));
            }
            Event::Start(Tag::Strong) => style.bold = true,
            Event::End(TagEnd::Strong) => style.bold = in_head,
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
                image = Some((String::new(), dest_url.trim().to_owned()));
            }
            // Spaces between images keep them in their row.
            Event::Text(text)
                if text.trim().is_empty()
                    && current.as_ref().is_some_and(|b| b.kind == Kind::Images) => {}
            Event::Text(text) => {
                let block = text_block(&mut blocks, &mut current, || {
                    open(Kind::Paragraph, &lists, quotes, outermost)
                });
                push(block, &text, style);
            }
            Event::Code(code) => {
                let block = text_block(&mut blocks, &mut current, || {
                    open(Kind::Paragraph, &lists, quotes, outermost)
                });
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
                for piece in html_pieces(&html, &mut in_comment) {
                    match piece {
                        Html::Text(text) => {
                            let text = collapse_spaces(&text);
                            let gap = std::mem::take(&mut html_gap);
                            if text.trim().is_empty() {
                                if let Some(block) = &mut current
                                    && block.kind != Kind::Images
                                    && !block.text.is_empty()
                                    && !block.text.ends_with(char::is_whitespace)
                                {
                                    push(block, " ", style);
                                }
                                continue;
                            }
                            let block = text_block(&mut blocks, &mut current, || {
                                open(Kind::Paragraph, &lists, quotes, outermost)
                            });
                            let fresh =
                                block.text.is_empty() || block.text.ends_with(char::is_whitespace);
                            let text = match fresh {
                                true => text.trim_start(),
                                false => &text,
                            };
                            if gap && !fresh && !text.starts_with(' ') {
                                push(block, " ", style);
                            }
                            push(block, text, style);
                        }
                        Html::Open(name, attributes) => match name.as_str() {
                            "br" => {
                                if let Some(block) = &mut current
                                    && block.kind != Kind::Images
                                {
                                    trim_spaces(block);
                                    push(block, "\n", style);
                                }
                            }
                            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "div" | "center"
                            | "details" | "summary" => {
                                close(&mut blocks, &mut current);
                                // A link ends with its block, as GitHub ends it.
                                link = None;
                                let centres = name == "center"
                                    || attribute(&attributes, "align")
                                        .is_some_and(|a| a.eq_ignore_ascii_case("center"));
                                html_open.push((name.clone(), centres));
                                centring += usize::from(centres);
                                centered.set(centring > 0);
                                if let Some(level) =
                                    name.strip_prefix('h').and_then(|l| l.parse::<u8>().ok())
                                {
                                    current = Some(open(
                                        Kind::Heading(level.min(4)),
                                        &lists,
                                        quotes,
                                        outermost,
                                    ));
                                }
                            }
                            "b" | "strong" => style.bold = true,
                            "i" | "em" => style.italic = true,
                            "code" | "kbd" | "tt" => style.code = true,
                            "s" | "del" | "strike" => style.strike = true,
                            "a" => {
                                let at = current.as_ref().map_or(0, |block| block.text.len());
                                let href = attribute(&attributes, "href").unwrap_or("");
                                link = Some((at, safe_link(href).then(|| href.trim().to_owned())));
                            }
                            "img" => {
                                let raw = attribute(&attributes, "src").unwrap_or("");
                                let src = match show_images {
                                    true => image_source(raw),
                                    false => None,
                                };
                                let shown = src.is_some();
                                let image = Image {
                                    src: src.unwrap_or_else(|| raw.trim().to_owned()),
                                    alt: attribute(&attributes, "alt").unwrap_or("").to_owned(),
                                    link: link.as_ref().and_then(|(_, url)| url.clone()),
                                    width: attribute(&attributes, "width").and_then(html_width),
                                };
                                put_image(
                                    &mut current,
                                    image,
                                    || open(Kind::Paragraph, &lists, quotes, outermost),
                                    style,
                                    link.is_some(),
                                    shown,
                                );
                            }
                            _ => {}
                        },
                        Html::Close(name) => match name.as_str() {
                            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "div" | "center"
                            | "details" | "summary" => {
                                close(&mut blocks, &mut current);
                                link = None;
                                if let Some(at) =
                                    html_open.iter().rposition(|(open, _)| *open == name)
                                {
                                    let closed = html_open.split_off(at);
                                    centring -=
                                        closed.iter().filter(|(_, centres)| *centres).count();
                                }
                                centered.set(centring > 0);
                            }
                            "b" | "strong" => style.bold = in_head,
                            "i" | "em" => style.italic = false,
                            "code" | "kbd" | "tt" => style.code = false,
                            "s" | "del" | "strike" => style.strike = false,
                            "a" => {
                                if let (Some((start, Some(url))), Some(block)) =
                                    (link.take(), &mut current)
                                    && start < block.text.len()
                                    && block.text.is_char_boundary(start)
                                {
                                    block.links.push((start..block.text.len(), url));
                                }
                            }
                            _ => {}
                        },
                    }
                }
                html_gap = html_line;
            }
            Event::SoftBreak => {
                if let Some(block) = &mut current
                    && block.kind != Kind::Images
                {
                    push(block, " ", style);
                }
            }
            Event::HardBreak => {
                if let Some(block) = &mut current
                    && block.kind != Kind::Images
                {
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
            "The rest is too long to show here.",
            Style {
                italic: true,
                ..Style::default()
            },
        );
        blocks.push(more);
    }
    blocks
}

/// `blocks` drawn as text, `id` telling apart the links of one description from another's,
/// their images from `images` by where they're from, or by their words until they're there.
pub fn render(
    id: &str,
    blocks: &[Block],
    images: &HashMap<String, Arc<RenderImage>>,
    theme: &Theme,
) -> AnyElement {
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
                    .child(text(id, ix, block, theme))
                    .into_any_element(),
                // As GitHub draws them, the top two over a rule.
                Kind::Heading(level) => div()
                    .pt(px(8.))
                    .text_size(px(match level {
                        1 => 22.,
                        2 => 17.,
                        3 => 15.,
                        _ => 14.,
                    }))
                    .line_height(px(match level {
                        1 => 30.,
                        2 => 24.,
                        _ => 22.,
                    }))
                    .when(*level <= 2 && !block.centered, |el| {
                        el.pb(px(6.)).border_b_1().border_color(theme.border_3)
                    })
                    .child(text(id, ix, block, theme))
                    .into_any_element(),
                Kind::Images => h_flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .when(block.centered, |el| el.justify_center())
                    .children(block.images.iter().enumerate().map(|(n, image)| {
                        picture(&format!("{id}-img-{ix}-{n}"), image, images, theme)
                    }))
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
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .child(text(id, ix, block, theme)),
                    )
                    .into_any_element(),
                Kind::Paragraph => text(id, ix, block, theme),
            };
            let indented = div()
                .mt(px(space))
                .pl(px(block.depth as f32 * 16.))
                .text_size(px(13.))
                .line_height(px(20.))
                .when(block.centered, |el| el.text_center())
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

/// An image as loaded, at the width HTML asks for or its own, never wider than the text; or
/// its words until it's there. In a link, a click opens the link.
fn picture(
    id: &str,
    image: &Image,
    images: &HashMap<String, Arc<RenderImage>>,
    theme: &Theme,
) -> AnyElement {
    let el = match images.get(&image.src) {
        Some(loaded) => {
            let shown = img(ImageSource::Render(loaded.clone())).max_w_full();
            match image.width {
                Some(Width::Px(width)) => shown.w(px(width)),
                Some(Width::Fraction(share)) => shown.w(relative(share)),
                None => shown,
            }
            .into_any_element()
        }
        None => div()
            .px(px(6.))
            .rounded(px(4.))
            .bg(theme.fill_gray.opacity(0.08))
            .text_color(theme.text_2)
            .text_size(px(12.))
            .child(SharedString::from(match image.alt.trim() {
                "" => "Image".to_owned(),
                alt => alt.to_owned(),
            }))
            .into_any_element(),
    };
    let wide = matches!(image.width, Some(Width::Fraction(_)));
    match image.link.clone() {
        Some(url) => div()
            .id(SharedString::from(id.to_owned()))
            .when(wide, |el| el.w_full())
            .cursor_pointer()
            .tooltip(Tooltip::text(url.clone()))
            .on_click(move |_, _, cx| cx.open_url(&url))
            .child(el)
            .into_any_element(),
        None => div()
            .when(wide, |el| el.w_full())
            .child(el)
            .into_any_element(),
    }
}

/// A block's text in its styles, its links clickable and naming where they go on hover.
/// Each run sets its font and colour, so what the block's element sets wouldn't reach it:
/// code is monospaced, a heading bold, and a quote quieter.
fn text(id: &str, ix: usize, block: &Block, theme: &Theme) -> AnyElement {
    let heading = matches!(block.kind, Kind::Heading(_));
    let font = |code: bool| match code || block.kind == Kind::Code {
        true => gpui::font(theme.mono_font.clone()),
        false => gpui::font(theme.ui_font.clone()),
    };
    let color: Hsla = match block.quoted {
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
            let weight = match (style.bold || heading, style.italic) {
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
    fn html_keeps_its_lines_and_blocks_apart() {
        let blocks = parse("one<br>two\n\n<p>A</p>\n<p>B</p>\n\n<div>C</div>\n");
        assert_eq!(
            texts(&blocks),
            [
                (&Kind::Paragraph, "one\ntwo"),
                (&Kind::Paragraph, "A"),
                (&Kind::Paragraph, "B"),
                (&Kind::Paragraph, "C"),
            ],
            "a <br> breaks the line, and each <p> and <div> is a block of its own"
        );
    }

    #[test]
    fn styles_and_links_stay_on_whole_characters() {
        for input in [
            "<b>Note:  </b><br>Über alles",
            "See <a href=\"https://x.io\">docs  </a><br>日本語",
            "<p>abc <a href=\"https://x.io\">x</p><p>€€€€</a></p>",
        ] {
            for block in parse(input) {
                let ranges = (block.styles.iter().map(|(range, _)| range))
                    .chain(block.links.iter().map(|(range, _)| range));
                for range in ranges {
                    assert!(
                        range.end <= block.text.len()
                            && block.text.is_char_boundary(range.start)
                            && block.text.is_char_boundary(range.end),
                        "{input:?}: {range:?} over {:?} would split a character as it's drawn",
                        block.text
                    );
                }
            }
        }
    }

    #[test]
    fn an_unclosed_inline_tag_ends_with_its_paragraph() {
        let blocks = parse("<b>Note: this\n\nPlain after\n");
        assert_eq!(texts(&blocks)[1], (&Kind::Paragraph, "Plain after"));
        assert!(
            blocks[1].styles.is_empty(),
            "a <b> never closed doesn't bold the rest of the document"
        );
    }

    #[test]
    fn html_entities_read_as_their_characters() {
        let blocks = parse("<p>Website&nbsp;•&nbsp;Blog &amp; more &copy;</p>\n");
        assert_eq!(
            texts(&blocks),
            [(&Kind::Paragraph, "Website • Blog & more &copy;")],
            "the common entities decode; one it doesn't know stays as written"
        );
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
            ["Logs", "ok", "Hi there Image"],
            "tags go, their text stays, and an image shows its words as a Markdown one does"
        );
        assert!(
            (blocks[2].styles.iter())
                .any(|(range, style)| style.bold && blocks[2].text[range.clone()] == *"there"),
            "<b> is bold"
        );
    }

    #[test]
    fn a_readme_lays_out_its_html_and_shows_its_images() {
        let readme = "<div align=\"center\">\n  <img width=\"100px\" src=\"https://x.io/logo.png\" alt=\"Logo\" />\n  <h1 align=\"center\">Name</h1>\n  <p align=\"center\">\n   <b>Better</b>.\n   <a href=\"https://x.io\">Site</a>\n  </p>\n</div>\n\n[![CI][s0]][l0] [![Chat](docs/chat.svg)](https://chat.x.io)\n\n[s0]: https://ci.x.io/badge.svg\n[l0]: https://ci.x.io\n\nPlain ![inline](a.png) text.\n";
        let blocks = parse_showing_images(readme);
        assert_eq!(
            texts(&blocks),
            [
                (&Kind::Images, ""),
                (&Kind::Heading(1), "Name"),
                (&Kind::Paragraph, "Better. Site"),
                (&Kind::Images, ""),
                (&Kind::Paragraph, "Plain Image: inline text."),
            ],
            "HTML headings and paragraphs are blocks, their spaces run together, and images \
             with no words between them sit in a row; one among words shows its words"
        );
        assert!(
            blocks[..3].iter().all(|block| block.centered)
                && blocks[3..].iter().all(|block| !block.centered),
            "what's inside a centred <div> is centred, and what's after its </div> isn't"
        );
        assert_eq!(
            blocks[0].images[0].width,
            Some(Width::Px(100.)),
            "an <img>'s width is kept"
        );
        assert_eq!(
            (blocks[3].images.iter())
                .map(|image| (image.src.as_str(), image.link.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("https://ci.x.io/badge.svg", Some("https://ci.x.io")),
                ("docs/chat.svg", Some("https://chat.x.io")),
            ],
            "a badge is loaded from the web or the repository, and opens its link"
        );
        assert_eq!(
            image_sources(&blocks),
            [
                "https://x.io/logo.png",
                "https://ci.x.io/badge.svg",
                "docs/chat.svg"
            ],
            "each image to load, once"
        );
        assert_eq!(
            blocks[2].links,
            [(8..12, "https://x.io".to_owned())],
            "an <a> links its words"
        );
        assert_eq!(
            image_source("../secret.png"),
            None,
            "nothing outside the repository"
        );
        assert_eq!(
            image_source("file:///etc/passwd"),
            None,
            "nor any other scheme"
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
    fn a_table_shows_a_row_a_line() {
        let blocks = parse("| Step | Result |\n|---|---|\n| build | ok |\n| test | **failed** |\n");
        assert_eq!(
            texts(&blocks),
            [
                (&Kind::Paragraph, "Step · Result"),
                (&Kind::Paragraph, "build · ok"),
                (&Kind::Paragraph, "test · failed"),
            ],
            "each row on its own line, its cells apart, without the pipes"
        );
        assert!(
            blocks[0].styles.iter().all(|(_, style)| style.bold),
            "the header row stands out"
        );
        let blocks = parse("|  | **Before** | After |\n|---|---|---|\n| Speed | 1s | 2s |\n");
        assert_eq!(
            texts(&blocks),
            [
                (&Kind::Paragraph, " · Before · After"),
                (&Kind::Paragraph, "Speed · 1s · 2s"),
            ],
            "an empty cell keeps its place, so the columns still line up"
        );
        let after = blocks[0].text.find("After").expect("in the header");
        assert!(
            (blocks[0].styles.iter()).any(|(range, style)| range.contains(&after) && style.bold),
            "bold text in the header leaves the rest of it bold"
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

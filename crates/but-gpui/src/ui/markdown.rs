//! Markdown, as pull request descriptions are written, drawn as formatted text.
//!
//! Only what can't reach outside the app is kept. Links open only to the web and to mail,
//! with where they go shown on hover, as a link's words can say anything. Images are drawn
//! only from GitHub, and any other shows its words, as fetching it would tell its host who
//! read the description (see [`crate::backend::images`]). A table with images lays its
//! cells side by side, as a before and after needs. HTML is read only as far as a
//! description lays itself out with it: headings, paragraphs and centring, bold, emphasis
//! and code, links, images and line breaks; other tags leave only their text. What's past
//! [`MAX_BYTES`] isn't parsed, and nesting past [`MAX_DEPTH`] steps in no further.
//!
//! Emphasis shows in the medium weight, not slanted: the app embeds no italic face, and
//! asking for one could pick a variable font installed on the machine, which is slow to
//! draw (see the crate's AGENTS.md).

use std::cell::Cell;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Font, FontWeight, Hsla, InteractiveText, IntoElement, ParentElement,
    SharedString, StrikethroughStyle, Styled, StyledText, TextRun, UnderlineStyle, div,
    prelude::FluentBuilder,
};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::{Tooltip, dp, fixed, h_flex, v_flex};
use crate::backend::images;
use crate::theme::Theme;

mod figure;
mod preview;

/// How much of a description is read; the rest is left out, and said to be.
const MAX_BYTES: usize = 64 * 1024;

/// How far lists and quotes step in at most.
const MAX_DEPTH: usize = 6;

/// The most images a description draws, more than any written by hand has; the rest show
/// their words, so a description can't have the app fetch thousands at once.
const MAX_IMAGES: usize = 24;

/// The HTML tags that make a block of their own.
const HTML_BLOCKS: &[&str] = &[
    "h1", "h2", "h3", "h4", "h5", "h6", "p", "div", "center", "details", "summary",
];

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
    /// Inside HTML that centres it.
    pub centered: bool,
    /// Its place in a table with images, which lays its cells side by side.
    pub cell: Option<TableCell>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Paragraph,
    Heading(u8),
    Code,
    /// A list item, after its marker: "•", "3." or a box.
    Item(String),
    Rule,
    /// Images on GitHub, side by side as far as they fit. Its text is their words, for what
    /// reads a description as text.
    Images(Vec<Image>),
}

/// An image a description draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub src: String,
    pub alt: String,
    /// The size its HTML gives it, in pixels, as `width="480"`; without one it's drawn a
    /// point a pixel, as GitHub draws it.
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Where a click on it goes: the link it's in, else the image itself.
    pub opens: String,
}

impl Image {
    /// What stands in its place where it isn't drawn.
    fn words(&self) -> String {
        match self.alt.trim() {
            "" => "Image".to_owned(),
            alt => format!("Image: {alt}"),
        }
    }
}

/// Where a cell is in a table with images: its row, counted from 1 through the description,
/// its column, and how many columns the table has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableCell {
    pub row: usize,
    pub column: usize,
    pub columns: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// Matches a search, tinted as the diff's find tints its matches.
    pub mark: bool,
}

/// Where a link may go when clicked: the web, or mail. Anything else, as a file, another
/// app's address or script, or a path the forge resolves against the repository, stays text.
fn safe_link(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

/// `html`'s text onto `text`, its common entities as the characters they stand for, as HTML
/// spaces out links with `&nbsp;`.
fn push_decoded(text: &mut String, html: &str) {
    // Each after its `&`.
    const ENTITIES: [(&str, char); 7] = [
        ("nbsp;", ' '),
        ("amp;", '&'),
        ("lt;", '<'),
        ("gt;", '>'),
        ("quot;", '"'),
        ("#39;", '\''),
        ("apos;", '\''),
    ];
    let mut rest = html;
    while let Some((before, after)) = rest.split_once('&') {
        text.push_str(before);
        let entity = (ENTITIES.iter()).find_map(|(name, ch)| Some((after.strip_prefix(name)?, ch)));
        match entity {
            Some((left, ch)) => {
                text.push(*ch);
                rest = left;
            }
            None => {
                text.push('&');
                rest = after;
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
            let Some((_, after)) = rest.split_once("-->") else {
                return pieces;
            };
            *in_comment = false;
            rest = after;
        }
        // `tag` from just past its `<`.
        let Some((before, tag)) = rest.split_once('<') else {
            text(&mut pieces, rest);
            return pieces;
        };
        text(&mut pieces, before);
        if let Some(comment) = tag.strip_prefix("!--") {
            *in_comment = true;
            rest = comment;
            continue;
        }
        let Some((inner, after)) = tag.split_once('>') else {
            return pieces;
        };
        let inner = inner.trim().trim_end_matches('/');
        rest = after;
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
    let (name, rest) = inner.split_at_checked(name_end).unwrap_or((inner, ""));
    let name = name.to_ascii_lowercase();
    let mut attributes = Vec::new();
    let mut rest = rest.trim_start();
    while !rest.is_empty() {
        let key_end = (rest.find(|c: char| c == '=' || c.is_whitespace())).unwrap_or(rest.len());
        let (key, left) = rest.split_at_checked(key_end).unwrap_or((rest, ""));
        let key = key.to_ascii_lowercase();
        rest = left.trim_start();
        let mut value = String::new();
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let (raw, left) = match after.chars().next() {
                Some(quote @ ('"' | '\'')) => {
                    let body = after.strip_prefix(quote).unwrap_or(after);
                    body.split_once(quote).unwrap_or((body, ""))
                }
                _ => {
                    let end = after.find(char::is_whitespace).unwrap_or(after.len());
                    after.split_at_checked(end).unwrap_or((after, ""))
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

/// An HTML size in pixels, as `width="480"`; a percentage isn't one.
fn pixels(value: &str) -> Option<f32> {
    let pixels: f32 = value.trim().trim_end_matches("px").parse().ok()?;
    (pixels.is_finite() && pixels > 0.).then_some(pixels)
}

/// Whether `markdown` draws an image, as a table of screenshots does.
fn draws_images(markdown: &str, options: Options) -> bool {
    let mut in_comment = false;
    Parser::new_ext(markdown, options).any(|event| match event {
        Event::Start(Tag::Image { dest_url, .. }) => images::on_github(&dest_url),
        Event::Html(html) | Event::InlineHtml(html) => (html_pieces(&html, &mut in_comment).iter())
            .any(|piece| {
                matches!(piece, Html::Open(tag, attributes)
                    if tag == "img" && attribute(attributes, "src").is_some_and(images::on_github))
            }),
        _ => false,
    })
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

/// `markdown` as blocks to draw.
pub fn parse(markdown: &str) -> Vec<Block> {
    let end = markdown.floor_char_boundary(MAX_BYTES);
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
    // How many images are drawn so far.
    let mut drawn = 0;
    // In a table with images, how many columns it has, and the rows read so far.
    let (mut grid, mut rows): (Option<usize>, usize) = (None, 0);
    let in_cell: Cell<Option<TableCell>> = Cell::new(None);
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
        cell: in_cell.get(),
    };
    /// `text` onto `block`; space between images only keeps them side by side.
    fn push(block: &mut Block, text: &str, style: Style) {
        if matches!(block.kind, Kind::Images(_)) {
            return;
        }
        let start = block.text.len();
        block.text.push_str(text);
        if style != Style::default() && !text.is_empty() {
            block.styles.push((start..block.text.len(), style));
        }
    }
    /// Drop the `trailing` characters at the end of `block`'s text, as the spaces HTML shows
    /// nothing for, and the styles and links over only them, so none ends past the text.
    fn trim_end(block: &mut Block, trailing: char) {
        let end = block.text.trim_end_matches(trailing).len();
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
            trim_end(block, ' ');
        }
        if let Some(block) = current.take()
            && (!block.text.trim().is_empty() || matches!(block.kind, Kind::Item(_)))
        {
            blocks.push(block);
        }
    }
    fn in_images(current: &Option<Block>) -> bool {
        matches!(
            current.as_ref().map(|block| &block.kind),
            Some(Kind::Images(_))
        )
    }
    /// The block `text` goes on, the current one or else a fresh one, and `text` as it goes
    /// there. Images end before text, which starts its block without the space before it,
    /// and space between images, which goes nowhere, keeps them side by side.
    fn writing<'a, 't>(
        blocks: &mut Vec<Block>,
        current: &'a mut Option<Block>,
        text: &'t str,
        fresh: impl FnOnce() -> Block,
    ) -> Option<(&'a mut Block, &'t str)> {
        let mut text = text;
        if in_images(current) {
            if text.trim().is_empty() {
                return None;
            }
            close(blocks, current);
            text = text.trim_start();
        }
        Some((current.get_or_insert_with(fresh), text))
    }
    /// End the images `current` is, as a line break does: the next start a line of their own.
    fn break_images(blocks: &mut Vec<Block>, current: &mut Option<Block>) {
        if in_images(current) {
            close(blocks, current);
        }
    }
    /// A link from where `current` has got to, to `url` if that's safe to open.
    fn open_link(current: &Option<Block>, url: &str) -> (usize, Option<String>) {
        // After images, its words start a block of their own.
        let words = current
            .as_ref()
            .filter(|block| !matches!(block.kind, Kind::Images(_)));
        let at = words.map_or(0, |block| block.text.len());
        (at, safe_link(url).then(|| url.trim().to_owned()))
    }
    /// Mark the text `link` covers, up to where `current` has got to.
    fn close_link(link: Option<(usize, Option<String>)>, current: &mut Option<Block>) {
        // One opened in a block that's since closed, as a list item, can start anywhere.
        // Images go where the link does on their own.
        if let (Some((start, Some(url))), Some(block)) = (link, current)
            && !matches!(block.kind, Kind::Images(_))
            && start < block.text.len()
            && block.text.is_char_boundary(start)
        {
            block.links.push((start..block.text.len(), url));
        }
    }
    /// `image` in its place: drawn beside any just before it when it's on GitHub, else its
    /// words, opening where it's from on a click.
    fn put_image(
        blocks: &mut Vec<Block>,
        current: &mut Option<Block>,
        image: Image,
        fresh: impl FnOnce() -> Block,
        style: Style,
        link: &mut Option<(usize, Option<String>)>,
        drawn: &mut usize,
    ) {
        let words = image.words();
        if images::on_github(&image.src) && *drawn < MAX_IMAGES {
            *drawn += 1;
            if !in_images(current) {
                // A link around words and images covers the words before them here, and
                // those after from the start of the block they go on.
                if let Some((start, url)) = link {
                    close_link(Some((*start, url.clone())), current);
                    *start = 0;
                }
                let mut row = Block {
                    kind: Kind::Images(Vec::new()),
                    ..fresh()
                };
                // An item of images draws them where its words would go, with no marker
                // beside nothing.
                match current.take() {
                    Some(Block {
                        kind: Kind::Item(_),
                        text,
                        depth,
                        ..
                    }) if text.is_empty() => row.depth = depth,
                    other => {
                        *current = other;
                        close(blocks, current);
                    }
                }
                *current = Some(row);
            }
            if let Some(block) = current {
                if !block.text.is_empty() {
                    block.text.push(' ');
                }
                block.text.push_str(&words);
                if let Kind::Images(images) = &mut block.kind {
                    images.push(image);
                }
            }
            return;
        }
        let Some((block, _)) = writing(blocks, current, &words, fresh) else {
            return;
        };
        let start = block.text.len();
        push(block, &words, style);
        // Inside a link, only the link opens: a click opens every link it's in.
        if link.is_none() && safe_link(&image.src) {
            block.links.push((start..block.text.len(), image.src));
        }
    }
    /// Where a click on an image with `src` goes: the `link` it's in, else the image.
    fn opens(link: &Option<(usize, Option<String>)>, src: &str) -> String {
        (link.as_ref().and_then(|(_, url)| url.clone())).unwrap_or_else(|| src.to_owned())
    }

    let read = markdown.get(..end).unwrap_or(markdown);
    for (event, range) in Parser::new_ext(read, options).into_offset_iter() {
        let html_line = matches!(event, Event::Html(_));
        // Text inside an image is its words, shown in its place.
        if let Some((alt, _)) = &mut image {
            match event {
                Event::Text(text) | Event::Code(text) => alt.push_str(&text),
                Event::End(TagEnd::Image) => {
                    let Some((alt, src)) = image.take() else {
                        continue;
                    };
                    let image = Image {
                        opens: opens(&link, &src),
                        src,
                        alt,
                        width: None,
                        height: None,
                    };
                    put_image(
                        &mut blocks,
                        &mut current,
                        image,
                        || open(Kind::Paragraph, &lists, quotes, outermost),
                        style,
                        &mut link,
                        &mut drawn,
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
            ) => {
                close(&mut blocks, &mut current);
                link = None;
            }
            Event::Start(Tag::Table(alignments)) => {
                close(&mut blocks, &mut current);
                let table = read.get(range).unwrap_or_default();
                grid = draws_images(table, options).then_some(alignments.len());
            }
            Event::End(TagEnd::Table) => grid = None,
            // Each table row is one line, its cells apart and the header bold, because
            // columns can't line up in wrapped text. A table with images lays out its
            // cells instead, each block of them in its place.
            Event::Start(row @ (Tag::TableHead | Tag::TableRow)) => {
                close(&mut blocks, &mut current);
                link = None;
                rows += 1;
                if grid.is_none() {
                    current = Some(open(Kind::Paragraph, &lists, quotes, outermost));
                }
                (in_head, cells) = (matches!(row, Tag::TableHead), 0);
                style.bold = in_head;
            }
            // Every cell after the first, empty ones too, so the columns still line up.
            Event::Start(Tag::TableCell) => {
                match grid {
                    Some(columns) => in_cell.set(Some(TableCell {
                        row: rows,
                        column: cells,
                        columns,
                    })),
                    None if cells > 0 => {
                        if let Some(block) = &mut current {
                            push(block, " · ", style);
                        }
                    }
                    None => {}
                }
                cells += 1;
            }
            Event::End(TagEnd::TableCell) if grid.is_some() => {
                close(&mut blocks, &mut current);
                link = None;
                in_cell.set(None);
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
                link = Some(open_link(&current, &dest_url));
            }
            Event::End(TagEnd::Link) => close_link(link.take(), &mut current),
            Event::Start(Tag::Image { dest_url, .. }) => {
                image = Some((String::new(), dest_url.trim().to_owned()));
            }
            Event::Text(text) => {
                let fresh = || open(Kind::Paragraph, &lists, quotes, outermost);
                if let Some((block, text)) = writing(&mut blocks, &mut current, &text, fresh) {
                    push(block, text, style);
                }
            }
            Event::Code(code) => {
                let fresh = || open(Kind::Paragraph, &lists, quotes, outermost);
                let Some((block, code)) = writing(&mut blocks, &mut current, &code, fresh) else {
                    continue;
                };
                push(
                    block,
                    code,
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
                                    && !block.text.is_empty()
                                    && !block.text.ends_with(char::is_whitespace)
                                {
                                    push(block, " ", style);
                                }
                                continue;
                            }
                            let Some((block, _)) =
                                writing(&mut blocks, &mut current, &text, || {
                                    open(Kind::Paragraph, &lists, quotes, outermost)
                                })
                            else {
                                continue;
                            };
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
                                break_images(&mut blocks, &mut current);
                                if let Some(block) = &mut current {
                                    trim_end(block, ' ');
                                    // A link opened on the spaces starts where they were.
                                    if let Some((start, _)) = &mut link {
                                        *start = (*start).min(block.text.len());
                                    }
                                    push(block, "\n", style);
                                }
                            }
                            tag if HTML_BLOCKS.contains(&tag) => {
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
                                let href = attribute(&attributes, "href").unwrap_or("");
                                link = Some(open_link(&current, href));
                            }
                            "img" => {
                                let src = attribute(&attributes, "src").unwrap_or("").trim();
                                let size = |name| attribute(&attributes, name).and_then(pixels);
                                let image = Image {
                                    opens: opens(&link, src),
                                    src: src.to_owned(),
                                    alt: attribute(&attributes, "alt").unwrap_or("").to_owned(),
                                    width: size("width"),
                                    height: size("height"),
                                };
                                put_image(
                                    &mut blocks,
                                    &mut current,
                                    image,
                                    || open(Kind::Paragraph, &lists, quotes, outermost),
                                    style,
                                    &mut link,
                                    &mut drawn,
                                );
                            }
                            _ => {}
                        },
                        Html::Close(name) => match name.as_str() {
                            tag if HTML_BLOCKS.contains(&tag) => {
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
                            "a" => close_link(link.take(), &mut current),
                            _ => {}
                        },
                    }
                }
                html_gap = html_line;
            }
            Event::SoftBreak => {
                if let Some(block) = &mut current {
                    push(block, " ", style);
                }
            }
            Event::HardBreak => {
                break_images(&mut blocks, &mut current);
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
            trim_end(block, '\n');
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

/// The images a description draws, in order, and where each block's first is among them, so a
/// preview pages through all of them.
struct Gallery {
    images: Arc<[Image]>,
    firsts: Vec<usize>,
}

impl Gallery {
    fn of(blocks: &[Block]) -> Self {
        let (mut images, mut firsts) = (Vec::new(), Vec::with_capacity(blocks.len()));
        for block in blocks {
            firsts.push(images.len());
            if let Kind::Images(drawn) = &block.kind {
                images.extend(drawn.iter().cloned());
            }
        }
        Self {
            images: images.into(),
            firsts,
        }
    }
}

/// `blocks` drawn as text, `id` telling apart the links of one description from another's.
pub fn render(id: &str, blocks: &[Block], theme: &Theme) -> AnyElement {
    let gallery = Gallery::of(blocks);
    let mut column = v_flex();
    let mut ix = 0;
    while let Some(block) = blocks.get(ix) {
        // A list's items sit together; other blocks, and other lists, stand apart, and
        // images, which have an edge where text has its leading, a little further.
        let item = |block: &Block| matches!(block.kind, Kind::Item(_));
        let image = |block: &Block| matches!(block.kind, Kind::Images(_)) && block.cell.is_none();
        let space = match ix.checked_sub(1).and_then(|before| blocks.get(before)) {
            None => 0.,
            Some(before) if item(before) && item(block) && before.list == block.list => 2.,
            Some(before) if image(before) || image(block) => 12.,
            Some(_) => 8.,
        };
        // A table row with images is its cells, which come one after another, side by side.
        let row = |cell: TableCell| {
            let rest = blocks.get(ix..).unwrap_or_default();
            let same = |other: &&Block| other.cell.is_some_and(|other| other.row == cell.row);
            rest.get(..rest.iter().take_while(same).count())
                .unwrap_or_default()
        };
        let (el, drawn) = match block.cell {
            Some(cell) => {
                let row = row(cell);
                (
                    table_row(id, ix, row, cell.columns, &gallery, theme),
                    row.len(),
                )
            }
            None => (element(id, ix, block, &gallery, theme), 1),
        };
        column = column.child(placed(block, space, el, theme));
        ix += drawn.max(1);
    }
    column.into_any_element()
}

/// `block` drawn, without where it sits.
fn element(id: &str, ix: usize, block: &Block, gallery: &Gallery, theme: &Theme) -> AnyElement {
    match &block.kind {
        Kind::Rule => div()
            .h(fixed(1.))
            .my(dp(4.))
            .bg(theme.border_3)
            .into_any_element(),
        Kind::Code => div()
            .px(dp(10.))
            .py(dp(8.))
            .rounded(dp(6.))
            .bg(theme.fill_gray.opacity(0.08))
            .text_size(dp(12.))
            .line_height(dp(18.))
            .child(text(id, ix, block, theme))
            .into_any_element(),
        // As GitHub draws them, the top two over a rule.
        Kind::Heading(level) => div()
            .pt(dp(8.))
            .text_size(dp(match level {
                1 => 22.,
                2 => 17.,
                3 => 15.,
                _ => 14.,
            }))
            .line_height(dp(match level {
                1 => 30.,
                2 => 24.,
                _ => 22.,
            }))
            .when(*level <= 2 && !block.centered, |el| {
                el.pb(dp(6.)).border_b_1().border_color(theme.border_3)
            })
            .child(text(id, ix, block, theme))
            .into_any_element(),
        Kind::Item(marker) => h_flex()
            .items_start()
            .gap(dp(6.))
            .child(
                div()
                    .flex_none()
                    .min_w(dp(14.))
                    .text_color(theme.text_2)
                    .child(SharedString::from(marker.clone())),
            )
            .child(div().flex_1().min_w_0().child(text(id, ix, block, theme)))
            .into_any_element(),
        Kind::Paragraph => text(id, ix, block, theme),
        Kind::Images(images) => {
            let Some(&first) = gallery.firsts.get(ix) else {
                return div().into_any_element();
            };
            h_flex()
                .flex_wrap()
                .items_start()
                .gap(dp(8.))
                .when(block.centered, |el| el.justify_center())
                .children((0..images.len()).map(|n| {
                    let id = format!("{id}-img-{ix}-{n}");
                    figure::Figure::new(id, gallery.images.clone(), first + n)
                }))
                .into_any_element()
        }
    }
}

/// `el`, drawn for `block`, `space` under what's before it, stepped in as far as `block` is.
fn placed(block: &Block, space: f32, el: AnyElement, theme: &Theme) -> AnyElement {
    let indented = div()
        .mt(dp(space))
        .pl(dp(block.depth as f32 * 16.))
        .text_size(dp(13.))
        .line_height(dp(20.))
        .when(block.centered, |el| el.text_center())
        .child(el);
    match block.quoted {
        true => indented
            .text_color(theme.text_2)
            .border_l_2()
            .border_color(theme.border_2)
            .ml(dp(2.))
            .pl(dp(10. + block.depth as f32 * 16.))
            .into_any_element(),
        false => indented.into_any_element(),
    }
}

/// A row of a table with images, `first` the index of its first block: its cells side by
/// side, each a column's share of the width, so a before and after sit as GitHub shows them.
fn table_row(
    id: &str,
    first: usize,
    row: &[Block],
    columns: usize,
    gallery: &Gallery,
    theme: &Theme,
) -> AnyElement {
    let cell = |column: usize| {
        let blocks = (row.iter().enumerate())
            .filter(move |(_, block)| block.cell.is_some_and(|cell| cell.column == column));
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(dp(8.))
            .children(blocks.map(|(n, block)| element(id, first + n, block, gallery, theme)))
    };
    h_flex()
        .items_start()
        .gap(dp(12.))
        .children((0..columns.max(1)).map(cell))
        .into_any_element()
}

/// `image`'s words, opening where a click on it would go, where it can't be drawn.
fn words(id: &str, image: &Image, theme: &Theme) -> AnyElement {
    let words = image.words();
    let block = Block {
        kind: Kind::Paragraph,
        depth: 0,
        quoted: false,
        list: 0,
        styles: Vec::new(),
        links: vec![(0..words.len(), image.opens.clone())],
        text: words,
        centered: false,
        cell: None,
    };
    text(id, 0, &block, theme)
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
        .filter_map(|cut| match *cut {
            [at, end] if at < end => Some((at, end)),
            _ => None,
        })
        .map(|(at, end)| {
            let style = (block.styles.iter())
                .filter(|(range, _)| range.contains(&at))
                .fold(Style::default(), |all, (_, style)| Style {
                    bold: all.bold || style.bold,
                    italic: all.italic || style.italic,
                    strike: all.strike || style.strike,
                    code: all.code || style.code,
                    mark: all.mark || style.mark,
                });
            let linked = block.links.iter().any(|(range, _)| range.contains(&at));
            let weight = match (style.bold || heading, style.italic) {
                (true, _) => FontWeight::SEMIBOLD,
                (false, true) => FontWeight::MEDIUM,
                (false, false) => FontWeight::NORMAL,
            };
            TextRun {
                len: end - at,
                font: Font {
                    weight,
                    ..font(style.code)
                },
                color: match linked {
                    true => theme.blue,
                    false => color,
                },
                background_color: match (style.mark, style.code) {
                    (true, _) => Some(theme.warn.opacity(0.28)),
                    (false, true) => Some(theme.fill_gray.opacity(0.12)),
                    (false, false) => None,
                },
                underline: linked.then(|| UnderlineStyle {
                    thickness: fixed(1.),
                    color: Some(theme.blue.opacity(0.4)),
                    wavy: false,
                }),
                strikethrough: style.strike.then(|| StrikethroughStyle {
                    thickness: fixed(1.),
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
            if let Some(url) = urls.get(link) {
                cx.open_url(url)
            }
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

    /// Where `block`'s links go.
    fn urls(block: &Block) -> Vec<&str> {
        block.links.iter().map(|(_, url)| url.as_str()).collect()
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
            // An inline tag left open going into a code block, whose last line ending goes.
            "<details>\n<summary><b>Logs</summary>\n\n```\nerr\n```\n</details>",
            "<p>Run <code>cargo test</p>\n\n```\nok\n```",
            // A link opened in one list item and closed in the next.
            "- ab <a href=\"https://x.io\">\n- ééééé</a>\n",
            // A link around an image drawn, which ends the text it opened in.
            "é [![x](https://github.com/a.png) ü](https://x.io)",
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
    fn a_link_opened_on_spaces_a_line_break_trims_covers_its_words() {
        let block = &parse("é  [<br>Āb](https://x.io)")[0];
        let (range, _) = &block.links[0];
        assert_eq!(&block.text[range.clone()], "\nĀb", "{:?}", block.text);
    }

    #[test]
    fn a_link_ends_with_its_block() {
        let blocks = parse("- ab <a href=\"https://x.io\">\n- hello</a>\n");
        assert!(
            blocks.iter().all(|block| block.links.is_empty()),
            "opened in one list item, it doesn't run on into the next: {blocks:?}"
        );
    }

    #[test]
    fn only_web_and_mail_links_open() {
        let blocks = parse(
            "[web](https://gitbutler.com) [mail](mailto:a@b.c) [script](javascript:alert(1)) \
             [file](file:///etc/passwd) [app](vscode://x) [path](../README.md)",
        );
        assert_eq!(
            urls(&blocks[0]),
            ["https://gitbutler.com", "mailto:a@b.c"],
            "any other address stays plain text"
        );
        assert_eq!(
            blocks[0].text, "web mail script file app path",
            "every link keeps its words"
        );
    }

    #[test]
    fn images_off_github_show_their_words() {
        let blocks = parse("![a chart](https://example.com/c.png) ![](data:image/png;base64,AAAA)");
        assert_eq!(
            blocks[0].text, "Image: a chart Image",
            "an image's words stand in for it"
        );
        assert_eq!(
            urls(&blocks[0]),
            ["https://example.com/c.png"],
            "a web image can be opened in the browser by its words; nothing else links"
        );
    }

    #[test]
    fn images_on_github_draw_side_by_side() {
        let blocks = parse(
            "See:\n<img width=\"480\" alt=\"Before\" src=\"https://github.com/user-attachments/assets/a\"> \
             <img src=\"https://raw.githubusercontent.com/o/r/s/b.png\" height=\"50%\">\nthen \
             [![after](https://github.com/c.png)](https://github.com/o/r/pull/1)<br><img src=\"https://github.com/d.png\">",
        );
        fn drawn(block: &Block) -> Vec<(&str, Option<f32>, &str)> {
            match &block.kind {
                Kind::Images(images) => (images.iter())
                    .map(|image| (image.src.as_str(), image.width, image.opens.as_str()))
                    .collect(),
                _ => Vec::new(),
            }
        }
        assert_eq!(
            texts(&blocks)
                .iter()
                .map(|(_, text)| *text)
                .collect::<Vec<_>>(),
            [
                "See:",
                "Image: Before Image",
                "then",
                "Image: after",
                "Image"
            ],
            "text before and after images stands apart, and they keep their words as text"
        );
        assert_eq!(
            drawn(&blocks[1]),
            [
                (
                    "https://github.com/user-attachments/assets/a",
                    Some(480.),
                    "https://github.com/user-attachments/assets/a"
                ),
                (
                    "https://raw.githubusercontent.com/o/r/s/b.png",
                    None,
                    "https://raw.githubusercontent.com/o/r/s/b.png"
                ),
            ],
            "images a space apart sit side by side, as wide as their HTML says in pixels"
        );
        assert_eq!(
            drawn(&blocks[3]),
            [(
                "https://github.com/c.png",
                None,
                "https://github.com/o/r/pull/1"
            )],
            "one in a link opens the link, and a line break starts a new row"
        );
    }

    #[test]
    fn a_link_split_by_images_keeps_its_words_linked() {
        let blocks = parse(
            "zz [View ![p](https://github.com/a.png) more](https://x.io) \
             ![q](https://github.com/b.png) [after](https://y.io)",
        );
        let linked: Vec<(&str, Vec<(&str, &str)>)> = (blocks.iter())
            .map(|block| {
                let links = (block.links.iter())
                    .map(|(range, url)| (&block.text[range.clone()], url.as_str()))
                    .collect();
                (block.text.as_str(), links)
            })
            .collect();
        assert_eq!(
            linked,
            [
                ("zz View", vec![("View", "https://x.io")]),
                ("Image: p", vec![]),
                ("more", vec![("more", "https://x.io")]),
                ("Image: q", vec![]),
                ("after", vec![("after", "https://y.io")]),
            ],
            "the words on either side of an image still link where the link goes, all of them, \
             and text after an image starts its line without the space before it"
        );
    }

    #[test]
    fn a_list_item_of_images_draws_them_in_its_place() {
        let blocks = parse("- ![shot](https://github.com/a.png)\n- [x] done\n");
        assert_eq!(
            (blocks.iter())
                .map(|block| (block.depth, block.text.as_str()))
                .collect::<Vec<_>>(),
            [(0, "Image: shot"), (0, "done")],
            "no marker beside nothing, and the image steps in no further than its item"
        );
    }

    #[test]
    fn a_description_draws_only_so_many_images() {
        let body = (0..=MAX_IMAGES)
            .map(|n| format!("![{n}](https://github.com/{n}.png)\n\n"))
            .collect::<String>();
        let blocks = parse(&body);
        let drawn = (blocks.iter())
            .filter(|block| matches!(block.kind, Kind::Images(_)))
            .count();
        assert_eq!(drawn, MAX_IMAGES, "past the limit, none is fetched");
        assert_eq!(
            blocks
                .last()
                .map(|block| (&block.kind, block.text.as_str())),
            Some((&Kind::Paragraph, format!("Image: {MAX_IMAGES}").as_str())),
            "the rest show their words"
        );
    }

    #[test]
    fn a_table_with_images_lays_out_its_cells() {
        let blocks = parse(
            "| | Before | After |\n|---|---|---|\n| Hover | \
             <img src=\"https://raw.githubusercontent.com/o/r/s/a.png\" width=\"420\"> | \
             ![after](https://raw.githubusercontent.com/o/r/s/b.png) |\n",
        );
        assert_eq!(
            (blocks.iter())
                .map(|block| {
                    let cell = block.cell.expect("every block is in a cell");
                    let image = matches!(block.kind, Kind::Images(_));
                    (
                        cell.row,
                        cell.column,
                        cell.columns,
                        image,
                        block.text.as_str(),
                    )
                })
                .collect::<Vec<_>>(),
            [
                (1, 1, 3, false, "Before"),
                (1, 2, 3, false, "After"),
                (2, 0, 3, false, "Hover"),
                (2, 1, 3, true, "Image"),
                (2, 2, 3, true, "Image: after"),
            ],
            "each cell in its column, an empty one leaving its place, so a before and after \
             sit side by side under their headings"
        );
        assert!(
            blocks[0].styles.iter().all(|(_, style)| style.bold),
            "the headings stand out"
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
    fn html_lays_out_its_blocks_and_centring() {
        let blocks = parse(
            "<div align=\"center\">\n  <h1 align=\"center\">Name</h1>\n  <p align=\"center\">\n   \
             <b>Better</b>.\n   <a href=\"https://x.io\">Site</a>\n  </p>\n</div>\n\nPlain text.\n",
        );
        assert_eq!(
            texts(&blocks),
            [
                (&Kind::Heading(1), "Name"),
                (&Kind::Paragraph, "Better. Site"),
                (&Kind::Paragraph, "Plain text."),
            ],
            "HTML headings and paragraphs are blocks, and their spaces run together"
        );
        assert!(
            blocks[..2].iter().all(|block| block.centered) && !blocks[2].centered,
            "what's inside a centred <div> is centred, and what's after its </div> isn't"
        );
        assert_eq!(
            blocks[1].links,
            [(8..12, "https://x.io".to_owned())],
            "an <a> links its words"
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

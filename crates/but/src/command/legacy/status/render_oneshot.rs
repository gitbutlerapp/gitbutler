use colored::{ColoredString, Colorize};
use ratatui::style::{Color, Modifier, Style};

use crate::{
    command::legacy::status::{
        StatusOutputLine,
        output::{CommitLineContent, StatusOutputContent},
    },
    utils::WriteWithUtils,
};

/// Print one line of status output.
///
/// Works by translating the ratatui lines and spans into `colored` text and printing it.
pub(super) fn render_oneshot(
    line: StatusOutputLine,
    out: &mut dyn WriteWithUtils,
) -> anyhow::Result<()> {
    let StatusOutputLine {
        connector,
        content,
        data: _,
    } = line;

    let should_colorize = colored::control::SHOULD_COLORIZE.should_colorize();

    let mut spans = Vec::new();
    if let Some(mut connector) = connector {
        spans.append(&mut connector);
    }
    match content {
        StatusOutputContent::Plain(mut content) => {
            spans.append(&mut content);
        }
        StatusOutputContent::Commit(CommitLineContent {
            mut sha,
            mut author,
            mut message,
            mut suffix,
        }) => {
            spans.append(&mut sha);
            spans.append(&mut author);
            spans.append(&mut message);
            spans.append(&mut suffix);
        }
    }

    for span in spans {
        let style = span.style;
        let rendered = render_span_with_colored(&span.content, style, should_colorize);
        write!(out, "{rendered}")?;
        if should_colorize && style_has_effect(style) {
            write!(out, "\x1b[0m")?;
        }
    }

    writeln!(out)?;

    Ok(())
}

/// Render a span's text using `colored` based on a ratatui style.
fn render_span_with_colored(content: &str, style: Style, should_colorize: bool) -> String {
    if !should_colorize || content.is_empty() {
        return content.to_string();
    }

    if !style_has_effect(style) {
        return content.to_string();
    }

    let mut styled = content.normal();

    if let Some(fg) = style.fg {
        styled = apply_foreground(styled, fg);
    }

    if let Some(bg) = style.bg {
        styled = apply_background(styled, bg);
    }

    styled = apply_modifiers(styled, style.add_modifier);

    styled.to_string()
}

/// Return true if this style has foreground/background colors or active modifiers.
fn style_has_effect(style: Style) -> bool {
    style.fg.is_some() || style.bg.is_some() || !style.add_modifier.is_empty()
}

/// Apply all style modifiers supported by `colored`.
fn apply_modifiers(mut styled: ColoredString, modifiers: Modifier) -> ColoredString {
    if modifiers.contains(Modifier::BOLD) {
        styled = styled.bold();
    }
    if modifiers.contains(Modifier::DIM) {
        styled = styled.dimmed();
    }
    if modifiers.contains(Modifier::ITALIC) {
        styled = styled.italic();
    }
    if modifiers.contains(Modifier::UNDERLINED) {
        styled = styled.underline();
    }
    if modifiers.contains(Modifier::SLOW_BLINK) || modifiers.contains(Modifier::RAPID_BLINK) {
        styled = styled.blink();
    }
    if modifiers.contains(Modifier::REVERSED) {
        styled = styled.reversed();
    }
    if modifiers.contains(Modifier::HIDDEN) {
        styled = styled.hidden();
    }
    if modifiers.contains(Modifier::CROSSED_OUT) {
        styled = styled.strikethrough();
    }

    styled
}

/// Apply foreground color using `colored`.
fn apply_foreground(styled: ColoredString, color: Color) -> ColoredString {
    match color {
        Color::Black => styled.black(),
        Color::Red => styled.red(),
        Color::Green => styled.green(),
        Color::Yellow => styled.yellow(),
        Color::Blue => styled.blue(),
        Color::Magenta => styled.magenta(),
        Color::Cyan => styled.cyan(),
        Color::Gray | Color::White => styled.white(),
        Color::DarkGray => styled.bright_black(),
        Color::LightRed => styled.bright_red(),
        Color::LightGreen => styled.bright_green(),
        Color::LightYellow => styled.bright_yellow(),
        Color::LightBlue => styled.bright_blue(),
        Color::LightMagenta => styled.bright_magenta(),
        Color::LightCyan => styled.bright_cyan(),
        Color::Rgb(r, g, b) => styled.truecolor(r, g, b),
        Color::Indexed(_) | Color::Reset => styled,
    }
}

/// Apply background color using `colored`.
fn apply_background(styled: ColoredString, color: Color) -> ColoredString {
    match color {
        Color::Black => styled.on_black(),
        Color::Red => styled.on_red(),
        Color::Green => styled.on_green(),
        Color::Yellow => styled.on_yellow(),
        Color::Blue => styled.on_blue(),
        Color::Magenta => styled.on_magenta(),
        Color::Cyan => styled.on_cyan(),
        Color::Gray | Color::White => styled.on_white(),
        Color::DarkGray => styled.on_bright_black(),
        Color::LightRed => styled.on_bright_red(),
        Color::LightGreen => styled.on_bright_green(),
        Color::LightYellow => styled.on_bright_yellow(),
        Color::LightBlue => styled.on_bright_blue(),
        Color::LightMagenta => styled.on_bright_magenta(),
        Color::LightCyan => styled.on_bright_cyan(),
        Color::Rgb(r, g, b) => styled.on_truecolor(r, g, b),
        Color::Indexed(_) | Color::Reset => styled,
    }
}

use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    prelude::*,
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToastKind {
    Error,
    Info,
    Debug,
}

#[derive(Debug, Default)]
pub(super) struct Toasts {
    toasts: Vec<Toast>,
}

#[derive(Debug)]
pub(super) struct Toast {
    kind: ToastKind,
    text: Text<'static>,
    dismiss_at: Instant,
}

impl Toasts {
    pub(super) fn insert(&mut self, kind: ToastKind, text: impl Into<Text<'static>>) {
        let text = text.into();
        if text_is_blank(&text) {
            return;
        }
        self.toasts.push(Toast {
            kind,
            text,
            dismiss_at: Instant::now() + Duration::from_secs(5),
        });
    }

    pub(super) fn update(&mut self) -> bool {
        let now = Instant::now();
        let len_before = self.toasts.len();
        self.toasts.retain(|toast| toast.dismiss_at > now);
        len_before != self.toasts.len()
    }
}

pub(super) fn render_toasts(frame: &mut Frame, area: Rect, toasts: &Toasts, theme: &'static Theme) {
    let mut bottom_margin = 1;
    for toast in &toasts.toasts {
        if bottom_margin >= area.height {
            break;
        }

        let rendered_height = render_toast(
            frame,
            area,
            ToastMargin {
                right: 1,
                bottom: bottom_margin,
            },
            toast,
            theme,
        );

        bottom_margin = bottom_margin.saturating_add(rendered_height);
    }
}

struct ToastMargin {
    right: u16,
    bottom: u16,
}

fn render_toast(
    frame: &mut Frame,
    area: Rect,
    margin: ToastMargin,
    toast: &Toast,
    theme: &'static Theme,
) -> u16 {
    let horizontal_padding: u16 = 1;
    let vertical_padding: u16 = 0;
    let border_width: u16 = 2;
    let border_height: u16 = 2;

    let ToastMargin {
        right: right_margin,
        bottom: bottom_margin,
    } = margin;

    let toast_text = toast.text.clone();

    let max_toast_width = area.width.saturating_sub(right_margin).max(1);
    let max_toast_height = area.height.saturating_sub(bottom_margin).max(1);

    let max_line_width = toast_text
        .lines
        .iter()
        .map(Line::width)
        .max()
        .unwrap_or_default() as u16;

    let desired_width = max_line_width
        .saturating_add(border_width)
        .saturating_add(horizontal_padding * 2);
    let width = desired_width.clamp(1, max_toast_width);

    let inner_width = width
        .saturating_sub(border_width)
        .saturating_sub(horizontal_padding * 2)
        .max(1) as usize;

    let wrapped_line_count: u16 = toast_text
        .lines
        .iter()
        .map(|line| {
            let line_width = line.width();
            let wrapped = line_width.div_ceil(inner_width);
            wrapped.max(1) as u16
        })
        .sum();

    let desired_height = wrapped_line_count
        .saturating_add(border_height)
        .saturating_add(vertical_padding * 2);
    let height = desired_height.clamp(1, max_toast_height);

    let x = area.x.saturating_add(
        area.width
            .saturating_sub(right_margin)
            .saturating_sub(width),
    );
    let y = area.y.saturating_add(
        area.height
            .saturating_sub(bottom_margin)
            .saturating_sub(height),
    );

    let toast_area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, toast_area);

    let border_style = match toast.kind {
        ToastKind::Error => theme.error,
        ToastKind::Info => theme.info,
        ToastKind::Debug => theme.hint,
    };

    let widget = Paragraph::new(toast_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .padding(Padding::new(
                    horizontal_padding,
                    horizontal_padding,
                    vertical_padding,
                    vertical_padding,
                )),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(widget, toast_area);

    height
}

fn text_is_blank(text: &Text<'_>) -> bool {
    text.lines
        .iter()
        .all(|line| line.spans.iter().all(|span| span.content.trim().is_empty()))
}

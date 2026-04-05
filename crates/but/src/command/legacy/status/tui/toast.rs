use std::time::{Duration, Instant};

use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    prelude::*,
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ToastKind {
    Error,
    #[expect(dead_code)]
    Info,
}

#[derive(Debug, Default)]
pub(super) struct Toasts {
    toasts: Vec<Toast>,
}

#[derive(Debug)]
pub(super) struct Toast {
    kind: ToastKind,
    text: String,
    dismiss_at: Instant,
}

impl Toasts {
    pub(super) fn insert(&mut self, kind: ToastKind, text: String) {
        if text.trim().is_empty() {
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

pub(super) fn render_toasts(frame: &mut Frame, area: Rect, toasts: &Toasts) {
    for (idx, toast) in toasts.toasts.iter().enumerate() {
        render_toast(
            frame,
            area,
            ToastMargin {
                right: 1 + idx as u16,
                bottom: idx as u16,
            },
            toast,
        );
    }
}

struct ToastMargin {
    right: u16,
    bottom: u16,
}

fn render_toast(frame: &mut Frame, area: Rect, margin: ToastMargin, toast: &Toast) {
    let horizontal_padding: u16 = 1;
    let vertical_padding: u16 = 0;
    let border_width: u16 = 2;
    let border_height: u16 = 2;

    let ToastMargin {
        right: right_margin,
        bottom: bottom_margin,
    } = margin;

    let toast_text = toast
        .text
        .into_text()
        .unwrap_or_else(|_| Text::raw(toast.text.clone()));

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
        ToastKind::Error => Style::default().red(),
        ToastKind::Info => Style::default().green(),
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
}

//! The diff column: every changed file as a card, built row by row so only what's on
//! screen is laid out. Uncommitted files carry review controls; every hunk gets a small
//! toolbar while it's under the cursor or the pointer, as in Zed's agent review.

use gpui::{AnyElement, App, Context, MouseButton, WeakEntity, div, list, prelude::*, px};

use super::{
    Check, Cursor, Detail, DiscardHunk, NextHunk, PreviousHunk, ToggleReviewed, checkbox_box,
};
use crate::app::MenuItem;
use crate::backend::{DiffBody, DiffSource, Selected};
use crate::state::Review;
use crate::theme::Theme;
use crate::ui::diff::{self, Decor, DiffRow};
use crate::ui::{ButtonStyle, Icon, button, empty_state, h_flex, icon_button, shortcut};

impl Detail {
    pub(super) fn render_diff(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let ground = |child: AnyElement| {
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .bg(theme.bg_2)
                .child(child)
                .into_any_element()
        };
        if let Some(err) = &self.diff_error {
            return ground(empty_state("Couldn't load the diff", err.clone(), theme));
        }
        let Some(diff) = &self.diff else {
            let title = if self.loading.is_some() {
                "Loading diff…"
            } else {
                ""
            };
            return ground(empty_state(title, "", theme));
        };
        if diff.files.is_empty() {
            let (title, detail) = match diff.key.source {
                DiffSource::Worktree => (
                    "No uncommitted changes",
                    "Edits show up here as soon as they're saved",
                ),
                _ => ("No changes", ""),
            };
            return ground(empty_state(title, detail, theme));
        }
        let entity = cx.entity().downgrade();
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .bg(theme.bg_2)
            .pt(px(12.))
            .child(
                list(self.list.clone(), move |ix, _window, cx| {
                    let Some(detail) = entity.upgrade() else {
                        return div().into_any_element();
                    };
                    detail.read(cx).render_diff_row(ix, detail.downgrade(), cx)
                })
                .size_full(),
            )
            .into_any_element()
    }

    fn render_diff_row(&self, ix: usize, entity: WeakEntity<Detail>, cx: &App) -> AnyElement {
        let theme = Theme::get(cx);
        let Some(diff) = self.diff.as_ref().filter(|diff| ix < diff.rows.len()) else {
            return div().into_any_element();
        };
        let row = diff.rows[ix];
        let worktree = diff.key.source == DiffSource::Worktree;
        let store = self.store.read(cx);
        let hunk_key = |file: usize, hunk: usize| match &diff.files[file].body {
            DiffBody::Hunks { hunks, .. } => {
                Some((diff.files[file].file.path.clone(), hunks[hunk].key))
            }
            _ => None,
        };
        let excluded = |file: usize, hunk: usize| {
            self.composing
                && worktree
                && hunk_key(file, hunk).is_some_and(|(path, key)| !self.hunk_checked(&path, key))
        };
        match row {
            DiffRow::File(file) => {
                let folded = matches!(diff.rows.get(ix + 1), Some(DiffRow::Gap));
                let review =
                    worktree.then(|| store.review(&diff.key.project, &diff.files[file].file));
                let toggle = entity.clone();
                let header = diff::render_row(
                    row,
                    &diff.files,
                    theme,
                    Decor {
                        trailing: review.map(|review| review_button(ix, file, review, entity, cx)),
                        current: self.cursor == Cursor { file, hunk: None },
                        folded,
                        ..Default::default()
                    },
                );
                div()
                    .id(("file-header", ix))
                    .child(header)
                    .on_click(move |_, _, cx| {
                        toggle
                            .update(cx, |detail, cx| detail.toggle_fold(file, cx))
                            .ok();
                    })
                    .into_any_element()
            }
            DiffRow::Hunk { file, hunk } => {
                let current = self.cursor
                    == Cursor {
                        file,
                        hunk: Some(hunk),
                    };
                let leading = (self.composing && worktree)
                    .then(|| hunk_key(file, hunk))
                    .flatten()
                    .map(|(path, key)| {
                        let check = if self.hunk_checked(&path, key) {
                            Check::All
                        } else {
                            Check::None
                        };
                        let entity = entity.clone();
                        checkbox_box(("hunk-check", ix), check, theme)
                            .on_click(move |_, _, cx| {
                                entity
                                    .update(cx, |detail, cx| detail.toggle_hunk(&path, key, cx))
                                    .ok();
                            })
                            .into_any_element()
                    });
                let toolbar = hunk_toolbar(ix, file, hunk, worktree, current, entity.clone(), cx);
                let element = diff::render_row(
                    row,
                    &diff.files,
                    theme,
                    Decor {
                        leading,
                        trailing: Some(toolbar),
                        excluded: excluded(file, hunk),
                        current,
                        folded: false,
                    },
                );
                // Right-click offers the same discard, with its key.
                let discard = hunk_key(file, hunk).map(|(_, key)| {
                    (
                        diff.key.project.clone(),
                        Selected {
                            file: diff.files[file].file.clone(),
                            hunks: Some(vec![key]),
                        },
                    )
                });
                div()
                    .id(("hunk", ix))
                    .group("hunk-row")
                    .child(element)
                    .when_some(discard.filter(|_| worktree), |el, (project, change)| {
                        let app = self.app.clone();
                        el.on_mouse_down(MouseButton::Right, move |event, _, cx| {
                            let (project, change) = (project.clone(), change.clone());
                            let item = MenuItem::new("Discard hunk", move |app, window, cx| {
                                app.discard(project.clone(), vec![change.clone()], window, cx)
                            })
                            .key(shortcut(&DiscardHunk, cx))
                            .danger();
                            app.update(cx, |app, cx| app.open_menu(event.position, vec![item], cx))
                                .ok();
                        })
                    })
                    .into_any_element()
            }
            DiffRow::Line { file, hunk, .. } => diff::render_row(
                row,
                &diff.files,
                theme,
                Decor {
                    excluded: excluded(file, hunk),
                    ..Default::default()
                },
            ),
            DiffRow::Note(_) | DiffRow::Gap => {
                diff::render_row(row, &diff.files, theme, Decor::default())
            }
        }
    }
}

/// Lite's three-state review toggle on a file header.
fn review_button(
    ix: usize,
    file: usize,
    review: Review,
    entity: WeakEntity<Detail>,
    cx: &App,
) -> AnyElement {
    let theme = Theme::get(cx);
    let (check, label) = match review {
        Review::Reviewed => (Check::All, "Reviewed"),
        Review::Changed => (Check::Some, "Needs review"),
        Review::NotReviewed => (Check::None, "Not reviewed"),
    };
    let key = shortcut(&ToggleReviewed, cx);
    h_flex()
        .id(("review", ix))
        .flex_none()
        .h(px(22.))
        .px(px(6.))
        .gap(px(6.))
        .rounded(px(4.))
        .text_size(px(12.))
        .text_color(if review == Review::Changed {
            theme.warn
        } else {
            theme.text_2
        })
        .hover(|el| el.bg(theme.hover()))
        .child(checkbox_box(("review-check", ix), check, theme))
        .child(label)
        .tooltip(move |_, cx| {
            crate::ui::Tooltip::with_key(
                "Mark reviewed and go to the next file".into(),
                key.clone(),
                cx,
            )
        })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            entity
                .update(cx, |detail, cx| detail.toggle_reviewed_file(file, cx))
                .ok();
        })
        .into_any_element()
}

/// Previous, next and (for uncommitted changes) discard, shown on the hunk under the
/// cursor and on the one under the pointer.
fn hunk_toolbar(
    ix: usize,
    file: usize,
    hunk: usize,
    worktree: bool,
    current: bool,
    entity: WeakEntity<Detail>,
    cx: &App,
) -> AnyElement {
    let step = |forward: bool| {
        let entity = entity.clone();
        move |_: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut App| {
            entity
                .update(cx, |detail, cx| {
                    detail.cursor = Cursor {
                        file,
                        hunk: Some(hunk),
                    };
                    detail.move_hunk(forward, cx);
                })
                .ok();
        }
    };
    let discard = entity.clone();
    h_flex()
        .flex_none()
        .gap(px(2.))
        .font_family(Theme::get(cx).ui_font.clone())
        .when(!current, |el| {
            el.invisible().group_hover("hunk-row", |el| el.visible())
        })
        .child(
            icon_button(("hunk-up", ix), Icon::ArrowUp)
                .small()
                .tooltip_with_key("Previous hunk", shortcut(&PreviousHunk, cx))
                .on_click(step(false)),
        )
        .child(
            icon_button(("hunk-down", ix), Icon::ArrowDown)
                .small()
                .tooltip_with_key("Next hunk", shortcut(&NextHunk, cx))
                .on_click(step(true)),
        )
        .when(worktree, |el| {
            el.child(
                button(("hunk-discard", ix), "Discard")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .tooltip_with_key("Throw this hunk away", shortcut(&DiscardHunk, cx))
                    .on_click(move |_, window, cx| {
                        discard
                            .update(cx, |detail, cx| {
                                detail.discard(file, Some(hunk), window, cx)
                            })
                            .ok();
                    }),
            )
        })
        .into_any_element()
}

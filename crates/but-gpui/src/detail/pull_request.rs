//! A branch's pull request on its page: the card with its state, labels, checks and
//! reviews, then what the forge says about its review, its open conversations and the
//! latest comments.

use gpui::{
    AnyElement, ClickEvent, Context, Div, MouseButton, MouseDownEvent, div, prelude::*, px,
};

use super::Detail;
use crate::app::MenuItem;
use crate::backend;
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::diff;
use crate::ui::review as ui_review;
use crate::ui::{
    ButtonStyle, Icon, button, h_flex, icon, list_row, plural, shortcut, small_icon, v_flex,
};

/// A branch's pull request in two lines, as t3code shows one: what it is, then its number,
/// state and checks, with a way to open it.
pub(super) fn render_review(
    review: &backend::Review,
    verdicts: Option<&backend::reviews::Verdicts>,
    github: bool,
    theme: &Theme,
    cx: &Context<Detail>,
) -> AnyElement {
    let (glyph, color) = ui_review::state_glyph(review.state, theme);
    let title = if review.title.is_empty() {
        format!("Pull request #{}", review.number)
    } else {
        review.title.clone()
    };
    v_flex()
        .flex_none()
        .px(px(16.))
        .py(px(12.))
        .gap(px(4.))
        .border_b_1()
        .border_color(theme.border_3)
        .child(
            h_flex()
                .gap(px(8.))
                .child(icon(glyph).text_color(color))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(title),
                )
                .when_some(review.url.clone(), |el, url| {
                    el.child(
                        button("open-review", "Open")
                            .icon(Icon::Link)
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key(
                                "Open the pull request",
                                shortcut(&crate::app::OpenPullRequest, cx),
                            )
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    )
                }),
        )
        .child(
            h_flex()
                .pl(px(24.))
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(format!("#{}", review.number))
                .child("·")
                .child(ui_review::state_label(review.state))
                .when_some(review.checks, |el, checks| {
                    let (glyph, color, text) = ui_review::checks_summary(checks, theme);
                    el.child("·")
                        .child(small_icon(glyph).text_color(color))
                        .child(text)
                }),
        )
        .when(!review.labels.is_empty(), |el| {
            el.child(h_flex().pl(px(24.)).gap(px(4.)).flex_wrap().children(
                review.labels.iter().map(|label| {
                    div()
                        .px(px(6.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(theme.border_2)
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(label.clone())
                }),
            ))
        })
        .when(review.auto_merge, |el| {
            el.child(
                div()
                    .pl(px(24.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child("Merges when checks pass"),
            )
        })
        .when(!review.failed_checks.is_empty(), |el| {
            el.child(failed_checks(&review.failed_checks, theme))
        })
        .children(review_line(review, verdicts, github, theme))
        .into_any_element()
}

impl Detail {
    /// Ask the forge where the review of `ref_name`'s open pull request stands, for its card.
    /// Once per visit to the branch, rather than on every reload.
    pub(crate) fn load_feedback(
        &mut self,
        project: &ProjectId,
        ref_name: &str,
        cx: &mut Context<Self>,
    ) {
        use backend::ReviewState as S;
        let store = self.store.read(cx);
        let number = (store.workspace(project))
            .and_then(|ws| ws.find_branch(ref_name)?.review.as_ref())
            .filter(|review| matches!(review.state, S::Open | S::Draft))
            .map(|review| review.number);
        let (Some(number), Some(path)) = (number, store.path(project)) else {
            return;
        };
        let project = project.clone();
        self.feedback_asked = Some(std::time::Instant::now());
        self._feedback_load = cx.spawn(async move |this, cx| {
            let feedback = backend::run(move || backend::reviews::feedback(&path, number)).await;
            // A forge that can't say, or no account for it, leaves the card as it was.
            let Ok(feedback) = feedback else {
                return;
            };
            this.update(cx, |this, cx| {
                this.feedback = Some((project, number, feedback));
                cx.notify();
            })
            .ok();
        });
    }

    /// Mark a conversation on the pull request resolved, then ask again where it stands.
    fn resolve_thread(
        &mut self,
        project: ProjectId,
        ref_name: String,
        id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let resolved = backend::run(move || backend::reviews::resolve_thread(&path, id)).await;
            this.update(cx, |this, cx| match resolved {
                Ok(()) => {
                    this.store
                        .update(cx, |store, cx| store.snack("Resolved", cx));
                    this.load_feedback(&project, &ref_name, cx)
                }
                Err(err) => this.store.update(cx, |store, cx| {
                    store.error("Couldn't resolve the conversation", format!("{err:#}"), cx)
                }),
            })
            .ok();
        })
        .detach();
    }

    /// The conversations still open on the branch's pull request: where each is and how it
    /// opened. A click shows its line in the diff; its menu opens it on the forge or
    /// resolves it.
    pub(super) fn render_threads(
        &self,
        project: &ProjectId,
        ref_name: &str,
        threads: &[backend::reviews::Thread],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        /// More than this many are better read on the forge.
        const SHOWN: usize = 8;
        if threads.is_empty() {
            return None;
        }
        let rows = threads.iter().take(SHOWN).enumerate().map(|(ix, thread)| {
            let (name, _) = diff::split_path(&thread.path);
            let place = match thread.line {
                Some(line) => format!("{name}:{line}"),
                None => name.to_owned(),
            };
            let (path, line, url) = (thread.path.clone(), thread.line, thread.url.clone());
            let menu_url = url.clone();
            let (project, ref_name, id) = (project.clone(), ref_name.to_owned(), thread.id.clone());
            list_row(("thread", ix), false, theme)
                .text_size(px(12.))
                .gap(px(6.))
                .child(
                    div()
                        .flex_none()
                        .font_family(theme.mono_font.clone())
                        .text_color(theme.text_3)
                        .child(place),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .child(format!("{}: {}", thread.author, thread.opening)),
                )
                .when(thread.replies > 0, |el| {
                    el.child(div().flex_none().text_color(theme.text_3).child(
                        match thread.replies {
                            1 => "1 reply".to_owned(),
                            n => format!("{n} replies"),
                        },
                    ))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !this.reveal_line(&path, line, cx) {
                        cx.open_url(&url);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let url = menu_url.clone();
                        let (project, ref_name, id) =
                            (project.clone(), ref_name.clone(), id.clone());
                        let (reply_project, reply_ref, reply_id) =
                            (project.clone(), ref_name.clone(), id.clone());
                        let items = vec![
                            MenuItem::new("Reply…", move |app, window, cx| {
                                let (project, ref_name, id) =
                                    (reply_project.clone(), reply_ref.clone(), reply_id.clone());
                                app.reply_to_thread(project, ref_name, id, window, cx)
                            }),
                            MenuItem::new("Open on the forge", move |_, _, cx| cx.open_url(&url)),
                            MenuItem::new("Mark resolved", move |app, _, cx| {
                                let (project, ref_name, id) =
                                    (project.clone(), ref_name.clone(), id.clone());
                                app.detail.update(cx, |detail, cx| {
                                    detail.resolve_thread(project, ref_name, id, cx)
                                })
                            }),
                        ];
                        this.app
                            .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                            .ok();
                    }),
                )
        });
        let more = threads.len().saturating_sub(SHOWN);
        Some(
            v_flex()
                .flex_none()
                .pb(px(8.))
                .border_b_1()
                .border_color(theme.border_3)
                .child(feedback_heading(
                    format!("{} open", plural(threads.len(), "conversation")),
                    theme,
                ))
                .children(rows)
                .when(more > 0, |el| {
                    el.child(
                        div()
                            .px(px(16.))
                            .text_size(px(12.))
                            .text_color(theme.text_3)
                            .child(format!("And {more} more on the forge")),
                    )
                })
                .into_any_element(),
        )
    }
}

/// Above a part of what the forge said about a pull request.
fn feedback_heading(text: String, theme: &Theme) -> Div {
    div()
        .px(px(16.))
        .pt(px(10.))
        .pb(px(4.))
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.text_3)
        .child(text)
}

/// The latest comments people left on a pull request, each opening on the forge.
pub(super) fn render_comments(
    comments: &[backend::reviews::Comment],
    theme: &Theme,
) -> Option<AnyElement> {
    if comments.is_empty() {
        return None;
    }
    let rows = comments.iter().enumerate().map(|(ix, comment)| {
        let url = comment.url.clone();
        list_row(("comment", ix), false, theme)
            .text_size(px(12.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(format!("{}: {}", comment.author, comment.opening)),
            )
            .on_click(move |_, _, cx| cx.open_url(&url))
    });
    Some(
        v_flex()
            .flex_none()
            .pb(px(8.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(feedback_heading("Latest comments".into(), theme))
            .children(rows)
            .into_any_element(),
    )
}

/// The checks that failed, a line each, opening its log when clicked.
fn failed_checks(failed: &[(String, String)], theme: &Theme) -> impl IntoElement {
    /// More than this many say little more than the count does.
    const SHOWN: usize = 5;
    v_flex()
        .pl(px(24.))
        .text_size(px(12.))
        .children(
            failed
                .iter()
                .take(SHOWN)
                .enumerate()
                .map(|(ix, (name, url))| {
                    let url = url.clone();
                    h_flex()
                        .id(("failed-check", ix))
                        .gap(px(6.))
                        .min_w(px(0.))
                        .text_color(theme.danger)
                        .cursor_pointer()
                        .tooltip(|_, cx| {
                            crate::ui::Tooltip::with_key("Open why it failed".into(), None, cx)
                        })
                        .child(small_icon(Icon::Cross).text_color(theme.danger))
                        .child(div().min_w(px(0.)).truncate().child(name.clone()))
                        .on_click(move |_, _, cx| cx.open_url(&url))
                }),
        )
}

/// Where a pull request's review stands: who approved, who asked for changes, and whose
/// review it's still waiting on, a line each. Nothing when there's nothing to say.
fn review_line(
    review: &backend::Review,
    verdicts: Option<&backend::reviews::Verdicts>,
    github: bool,
    theme: &Theme,
) -> Option<AnyElement> {
    // GitHub lists a reviewer who decided only once asked again, which means they're
    // awaited; the other forges keep listing everyone asked, decided or not.
    let waiting: Vec<String> = (review.reviewers.iter())
        .filter(|login| {
            github
                || verdicts.is_none_or(|v| {
                    !v.approved.contains(login) && !v.changes_requested.contains(login)
                })
        })
        .cloned()
        .collect();
    let parts = [
        (
            verdicts.map_or(&[][..], |v| &v.approved),
            "Approved by",
            theme.safe,
        ),
        (
            verdicts.map_or(&[][..], |v| &v.changes_requested),
            "Changes requested by",
            theme.danger,
        ),
        (&waiting[..], "Waiting on", theme.text_3),
    ];
    let parts: Vec<_> = (parts.into_iter())
        .filter(|(logins, ..)| !logins.is_empty())
        .map(|(logins, label, color)| {
            div()
                .text_color(color)
                .child(format!("{label} {}", logins.join(", ")))
        })
        .collect();
    // One to a line: the pane is narrow, and logins run long.
    (!parts.is_empty()).then(|| {
        v_flex()
            .pl(px(24.))
            .text_size(px(12.))
            .children(parts)
            .into_any_element()
    })
}

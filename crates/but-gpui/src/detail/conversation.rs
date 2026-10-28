//! A pull request's conversation, under its description, as Lite's activity: its comments and
//! reviews, newest first, each review with the threads it started on the diff. It's there to
//! read. Answering, reacting and resolving happen on the forge, where a note's time opens it.

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Div, ElementId, Role, div, prelude::*,
};

use super::Detail;
use super::pull_request::{card, tag};
use crate::backend::reviews::Verdict;
use crate::backend::{Forge, Note, Remark, Review, Thread};
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::diff::{excerpt, split_path};
use crate::ui::markdown;
use crate::ui::person::forge_avatar;
use crate::ui::{
    Icon, Tooltip, ago, dp, error_summary, h_flex, local_time, plural, small_icon, v_flex,
};

impl Detail {
    /// The heading over the conversation on `review` of `project`, saying while there's
    /// nothing under it why: it's loading, it couldn't be read, or nobody's said anything.
    /// Only GitHub's conversation is read, so for another forge there's none.
    pub(super) fn conversation(
        &self,
        project: &ProjectId,
        review: &Review,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let store = self.store.read(cx);
        if store.workspace(project).and_then(|ws| ws.forge) != Some(Forge::GitHub) {
            return None;
        }
        let quiet = |text: &'static str| {
            div()
                .text_size(dp(13.))
                .text_color(theme.text_3)
                .child(text)
        };
        let status = match store.conversation(project, review.number) {
            // Faded in, so a read done in a blink doesn't flash it.
            None => Some(
                quiet("Loading…")
                    .with_animation(
                        ("conversation-loading", review.number as usize),
                        Animation::new(std::time::Duration::from_millis(600)),
                        |el, delta| el.opacity(((delta - 0.5) * 2.).clamp(0., 1.)),
                    )
                    .into_any_element(),
            ),
            Some(Err(err)) => Some(
                quiet("Couldn't ask GitHub for the activity")
                    .id("conversation-error")
                    .tooltip(Tooltip::text(error_summary(err)))
                    .into_any_element(),
            ),
            Some(Ok(remarks)) if remarks.is_empty() => {
                Some(quiet("No comments").into_any_element())
            }
            Some(Ok(_)) => None,
        };
        Some(
            v_flex()
                .pt(dp(24.))
                .gap(dp(16.))
                // With the first card's own 6, 16 under the heading.
                .when(status.is_none(), |el| el.pb(dp(10.)))
                .border_t_1()
                .border_color(theme.border_3)
                .child(
                    div()
                        .text_size(dp(15.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Activity"),
                )
                .children(status),
        )
    }

    /// How many remarks the conversation on pull request `number` of `project` shows.
    pub(super) fn remarks_shown(&self, project: &ProjectId, number: u64, cx: &App) -> usize {
        let store = self.store.read(cx);
        let github = store.workspace(project).and_then(|ws| ws.forge) == Some(Forge::GitHub);
        match store.conversation(project, number) {
            Some(Ok(remarks)) if github => remarks.len(),
            _ => 0,
        }
    }

    /// The `ix`th remark of the conversation on pull request `number` of `project`.
    pub(super) fn remark_at(
        &self,
        project: &ProjectId,
        number: u64,
        ix: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let Some(Ok(remarks)) = self.store.read(cx).conversation(project, number) else {
            return None;
        };
        Some(self.remark(remarks.get(ix)?, theme, cx))
    }

    /// A comment or review as Lite's card, or a thread, set in under the review that started it.
    fn remark(&self, remark: &Remark, theme: &Theme, cx: &Context<Self>) -> Div {
        let card = card(theme).p(dp(14.)).gap(dp(8.));
        match remark {
            Remark::Comment(note) => card
                .child(header(note, None, false, theme))
                .child(markdown::render(&note.url, &note.body, theme)),
            Remark::Review { verdict, note } => card
                .child(header(note, Some(*verdict), false, theme))
                .when(!note.body.is_empty(), |el| {
                    el.child(markdown::render(&note.url, &note.body, theme))
                }),
            // Under its review, in line with the review's words.
            Remark::Thread { thread, in_review } => div()
                .when(*in_review, |el| el.pl(dp(14.)))
                .child(self.thread(thread, theme, cx)),
        }
    }

    /// A thread as Lite's: a row naming where it hangs that folds it, then the lines it's on
    /// and what was said there. It shows open unless it's resolved.
    fn thread(&self, thread: &Thread, theme: &Theme, cx: &Context<Self>) -> Div {
        let open = thread.resolved == self.threads_flipped.contains(&thread.id);
        let (name, dir) = split_path(&thread.path);
        let lines = match thread.lines {
            Some((start, end)) if start < end => format!(":{start}-{end}"),
            Some((_, end)) => format!(":{end}"),
            None => String::new(),
        };
        let state = match (thread.resolved, thread.outdated) {
            (true, _) => Some("Resolved"),
            (false, true) => Some("Outdated"),
            (false, false) => None,
        };
        let id = thread.id.clone();
        let anchor = h_flex()
            .id(ElementId::Name(format!("thread-{}", thread.id).into()))
            .role(Role::Button)
            .aria_label(match open {
                true => format!("Fold the thread on {}", thread.path),
                false => format!("Open the thread on {}", thread.path),
            })
            .px(dp(10.))
            .py(dp(7.))
            .gap(dp(6.))
            .text_size(dp(12.))
            .hover(|el| el.bg(theme.hover()))
            .child(
                small_icon(match open {
                    true => Icon::ChevronDown,
                    false => Icon::ChevronRight,
                })
                .text_color(theme.text_3),
            )
            .child(
                h_flex()
                    .flex_grow(1.)
                    .min_w_0()
                    .when(!dir.is_empty(), |el| {
                        el.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(theme.text_3)
                                .child(format!("{dir}/")),
                        )
                    })
                    .child(
                        div()
                            .flex_none()
                            .text_color(theme.text_1)
                            .child(format!("{name}{lines}")),
                    ),
            )
            .children(state.map(|label| tag(theme.text_2).child(label)))
            .when(!open, |el| {
                el.child(
                    div()
                        .flex_none()
                        .text_color(theme.text_3)
                        .child(plural(thread.comments.len(), "comment")),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.threads_flipped.remove(&id) {
                    this.threads_flipped.insert(id.clone());
                }
                cx.notify();
            }));
        v_flex()
            .rounded(dp(6.))
            .border_1()
            .border_color(theme.border_3)
            .bg(theme.bg_2)
            .overflow_hidden()
            .child(anchor)
            .when(open, |el| {
                el.child(
                    v_flex()
                        .p(dp(10.))
                        .gap(dp(10.))
                        .border_t_1()
                        .border_color(theme.border_3)
                        .bg(theme.bg_1)
                        .when(!thread.excerpt.is_empty(), |el| {
                            el.child(excerpt(&thread.excerpt, theme))
                        })
                        .children(thread.comments.iter().enumerate().map(|(ix, note)| {
                            v_flex()
                                .gap(dp(6.))
                                .when(ix > 0, |el| {
                                    el.pt(dp(10.)).border_t_1().border_color(theme.border_3)
                                })
                                .child(header(note, None, true, theme))
                                .child(markdown::render(&note.url, &note.body, theme))
                        })),
                )
            })
    }
}

/// Who wrote `note`, tagged when a bot did, with `verdict` when it's a review's, and when,
/// which opens it on GitHub. `compact` for a thread's, a step smaller.
fn header(note: &Note, verdict: Option<Verdict>, compact: bool, theme: &Theme) -> Div {
    let author = note.author.as_ref().map(|author| {
        h_flex()
            .min_w_0()
            .gap(dp(6.))
            .child(forge_avatar(
                &author.login,
                author.avatar_url.as_deref(),
                if compact { 16. } else { 18. },
                theme,
            ))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(dp(if compact { 12. } else { 13. }))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text_1)
                    .child(author.login.clone()),
            )
    });
    let verdict = verdict.map(|verdict| match verdict {
        Verdict::Approved => ("Approved", theme.safe),
        Verdict::ChangesRequested => ("Changes requested", theme.danger),
        Verdict::Commented => ("Reviewed", theme.text_2),
        Verdict::Dismissed => ("Review dismissed", theme.text_2),
    });
    h_flex()
        .min_h(dp(20.))
        .gap(dp(6.))
        .children(author)
        .when(note.bot, |el| el.child(tag(theme.text_2).child("Agent")))
        .children(verdict.map(|(label, color)| tag(color).child(label)))
        .child(div().flex_grow(1.))
        .children(note.at.map(|at| opens(note, at, compact, theme)))
}

/// When `note` was written, and whether it was edited since, as a link to it on GitHub,
/// where it can be answered. An arrow shows on hover, as on the checks' rows, in the card's
/// padding so the time lines up with the card's edge, and the tooltip has the time here.
fn opens(note: &Note, at: i64, compact: bool, theme: &Theme) -> AnyElement {
    let url = note.url.clone();
    // In a card's padding of 14 or a thread's of 10, with 2 before it.
    let (room, arrow) = if compact { (10., 8.) } else { (14., 12.) };
    let group = gpui::SharedString::from("note-time");
    h_flex()
        .id(ElementId::Name(format!("open-{}", note.url).into()))
        .group(group.clone())
        .role(Role::Link)
        .aria_label("Open on GitHub")
        .relative()
        .flex_none()
        .gap(dp(6.))
        .text_size(dp(12.))
        .text_color(theme.text_2)
        .hover(|el| el.text_color(theme.text_1))
        .child(ago(at))
        .when(note.edited, |el| {
            el.child(div().text_color(theme.text_3).child("edited"))
        })
        .child(
            h_flex()
                .absolute()
                .top_0()
                .bottom_0()
                .right(dp(-room))
                .child(
                    small_icon(Icon::ArrowUpRight)
                        .size(dp(arrow))
                        .text_color(theme.text_3)
                        .opacity(0.)
                        .group_hover(group, |el| el.opacity(1.)),
                ),
        )
        .tooltip(Tooltip::text(local_time(at)))
        .on_click(move |_, _, cx| cx.open_url(&url))
        .into_any_element()
}

//! A branch's pull request, on a tab of the branch's page beside its changes: what it is,
//! where it stands, its description and its checks, as the forge cache last saw them.
//! Nothing here asks the forge; the store's background refresh does, and the page says how
//! old what it shows is.

use gpui::{AnyElement, App, ClickEvent, Context, Div, Stateful, Window, div, prelude::*, px};

use super::Detail;
use super::panes::title_text;
use super::resolve::{banner_action, warn_banner};
use crate::backend::{CheckRun, Forge, Review, ReviewState};
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::markdown;
use crate::ui::review as ui_review;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, button, faint, h_flex, icon, local_time, muted, now, relative_time,
    shortcut, small_icon, v_flex,
};

/// How tall the tabs over the diff beside a list are.
const TABS_HEIGHT: f32 = 40.;

/// How old, in seconds, what the pull request tab shows can get before a failed refresh
/// warns about it.
const STALE_AFTER: i64 = 30 * 60;

impl Detail {
    /// The selected branch's pull request, if it has one: the branch page's, the Branches
    /// page's, or the one the project page's selected target commit landed.
    pub(super) fn selected_review<'a>(&'a self, cx: &'a App) -> Option<&'a Review> {
        let store = self.store.read(cx);
        match store.selection.as_ref()? {
            Selection::Branch(project, ref_name) => store
                .workspace(project)?
                .find_branch(ref_name)?
                .review
                .as_ref(),
            Selection::Branches(project) => self.selected_other_branch(project)?.review.as_ref(),
            Selection::Project(project) => self.selected_target_review(project),
            _ => None,
        }
    }

    /// Whether the branch page shows its pull request in place of its changes. The tab
    /// picked stays picked from branch to branch, as but.dev's does; a branch without a
    /// pull request shows its changes.
    pub fn showing_review(&self, cx: &App) -> bool {
        self.review_tab && self.selected_review(cx).is_some()
    }

    fn set_review_tab(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.review_tab == on {
            return;
        }
        self.review_tab = on;
        // The branch page's file list isn't drawn beside its pull request. The Branches
        // page's list stays, and keeps the keys.
        let focus = match (&self.store.read(cx).selection, on) {
            (Some(Selection::Branches(_)), _) => Some(self.side_focus.clone()),
            (Some(Selection::Branch(..)), true) => Some(self.focus.clone()),
            (Some(Selection::Branch(..)), false) => Some(self.files_focus.clone()),
            _ => None,
        };
        if let Some(focus) = focus {
            window.focus(&focus, cx);
        }
        cx.notify();
        // The window draws the diff beside the files, or doesn't.
        self.app.update(cx, |_, cx| cx.notify()).ok();
    }

    /// A branch or commit's tabs: its changes, with how many files, and its pull request,
    /// with a glyph while its checks fail or run.
    pub(super) fn review_tabs(
        &self,
        review: &Review,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let files = (self.shown_diff().map(|diff| diff.files.len())).filter(|n| *n > 0);
        let checks = review.checks.filter(|c| c.failed > 0 || c.running > 0);
        h_flex()
            // The first tab's label, not its hover ground, lines up with the text above.
            .ml(px(-8.))
            .child(
                tab("tab-changes", "Changes", !self.review_tab, theme)
                    .children(files.map(|n| faint(n.to_string(), theme)))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.set_review_tab(false, window, cx)),
                    ),
            )
            .child(
                tab("tab-review", "Pull request", self.review_tab, theme)
                    .children(checks.map(|checks| {
                        let (glyph, color, _) = ui_review::checks_summary(checks, theme);
                        small_icon(glyph).text_color(color)
                    }))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.set_review_tab(true, window, cx)),
                    ),
            )
            .into_any_element()
    }

    /// Beside the list of the Branches page or a project's page, the tabs of the selected
    /// branch or commit with a pull request, over the pull request or the diff.
    pub(super) fn render_list_tabs(
        &self,
        project: &ProjectId,
        review: &Review,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .child(
                h_flex()
                    .flex_none()
                    .h(px(TABS_HEIGHT))
                    .px(px(16.))
                    .items_end()
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(self.review_tabs(review, theme, cx)),
            )
            .child(match self.review_tab {
                true => self.render_review_tab(project, theme, cx),
                false => div().flex_1().into_any_element(),
            })
            .into_any_element()
    }

    /// How tall the tabs beside a list are, which the diff starts below: nothing when the
    /// page has none.
    pub(super) fn list_tabs_height(&self, cx: &App) -> f32 {
        match self.list_page(cx) && self.selected_review(cx).is_some() {
            true => TABS_HEIGHT,
            false => 0.,
        }
    }

    /// The pull request tab: its title and facts, how fresh they are, its description and
    /// its checks.
    pub(super) fn render_review_tab(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(review) = self.selected_review(cx) else {
            return div().into_any_element();
        };
        let store = self.store.read(cx);
        let forge =
            (store.workspace(project).and_then(|ws| ws.forge)).map_or("the forge", Forge::name);
        // A refresh that changes nothing reloads nothing, so the store's time can be newer.
        let synced = store.forge_synced(project).max(review.synced_at);
        let (checking, failed) = (store.forge_checking(project), store.forge_failed(project));
        let ask = {
            let (store, project) = (self.store.clone(), project.clone());
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |store, _| store.refresh_forge(&project))
            }
        };
        // A failed refresh is worth a warning only once what shows has aged; until then the
        // button to check again is enough.
        let stale = failed && synced.is_none_or(|at| now() - at > STALE_AFTER);
        // A merged or closed one is settled, and asking again is about the open ones.
        let state = store.review_state(project, review);
        let settled = matches!(state, ReviewState::Merged | ReviewState::Closed);
        let fresh = (!stale && !settled).then(|| {
            button(
                "review-check",
                match synced {
                    _ if checking => "Checking…".to_owned(),
                    Some(at) => format!("Checked {}", ago(at)),
                    None => format!("Check {forge}"),
                },
            )
            .icon(Icon::Refresh)
            .small()
            .style(ButtonStyle::Ghost)
            .tooltip(format!("Ask {forge} again"))
            .disabled_because(checking.then_some("Asking already"))
            .on_click(ask.clone())
        });
        let stale = (stale && !settled).then(|| {
            warn_banner(
                match synced {
                    Some(at) => format!("Showing what {forge} said {}", ago(at)),
                    None => format!("Showing what {forge} said last"),
                },
                format!("Couldn't update from {forge} just now."),
                Some(
                    banner_action("review-retry", "Try again", theme)
                        .disabled_because(checking.then_some("Asking already"))
                        .on_click(ask)
                        .into_any_element(),
                ),
                theme,
            )
        });
        let body = markdown::parse(&review.body);
        let doc = v_flex()
            .w_full()
            .max_w(px(760.))
            .px(px(24.))
            .py(px(20.))
            .gap(px(24.))
            .child(head(
                review,
                state,
                forge,
                fresh.map(IntoElement::into_any_element),
                theme,
                cx,
            ))
            .children(stale)
            .when(!body.is_empty(), |el| {
                el.child(section("Description", None, theme).child(markdown::render(
                    &format!("review-{}", review.number),
                    &body,
                    theme,
                )))
            })
            .when(!review.check_runs.is_empty(), |el| {
                el.child(self.render_checks(review, theme, cx))
            });
        let page = div()
            .id("review")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.review_scroll)
            .child(doc);
        scrolled(
            page,
            "review-scrollbar",
            Scroll::Div(self.review_scroll.clone()),
            theme,
        )
        .w_full()
        .into_any_element()
    }

    /// The checks, those that failed or still run first; the ones that passed or were
    /// skipped fold into a line, as they're seldom the news.
    fn render_checks(&self, review: &Review, theme: &Theme, cx: &Context<Self>) -> Div {
        let (live, rest): (Vec<_>, Vec<_>) =
            (review.check_runs.iter().enumerate()).partition(|(_, run)| run.failed != Some(false));
        let hint = review
            .checks
            .map(|checks| ui_review::checks_summary(checks, theme).2);
        let open = self.passed_checks_open;
        let skipped = rest
            .iter()
            .filter(|(_, run)| run.status == "skipped")
            .count();
        let folded = match skipped {
            0 => format!("{} passed", rest.len()),
            skipped => format!("{} passed, {skipped} skipped", rest.len() - skipped),
        };
        section("Checks", hint, theme)
            .children(live.iter().map(|(ix, run)| check_row(*ix, run, theme)))
            .when(!rest.is_empty(), |el| {
                let chevron = match open {
                    true => Icon::ChevronDown,
                    false => Icon::ChevronRight,
                };
                el.child(
                    row("passed-checks", theme)
                        .text_color(theme.text_2)
                        .child(small_icon(chevron).text_color(theme.text_3))
                        .child(folded)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.passed_checks_open = !this.passed_checks_open;
                            cx.notify();
                        })),
                )
                .when(open, |el| {
                    el.children(rest.iter().map(|(ix, run)| check_row(*ix, run, theme)))
                })
            })
    }
}

/// The pull request's state glyph and title, with a way to open it, over its number, state,
/// author, target, age and labels, and who's been asked to review it.
fn head(
    review: &Review,
    state: ReviewState,
    forge: &str,
    fresh: Option<AnyElement>,
    theme: &Theme,
    cx: &Context<Detail>,
) -> Div {
    let (glyph, color) = ui_review::state_glyph(state, theme);
    let title = match review.title.as_str() {
        "" => format!("Pull request #{}", review.number),
        title => title.to_owned(),
    };
    let mut facts: Vec<AnyElement> = vec![
        div()
            .child(format!("#{}", review.number))
            .into_any_element(),
        div()
            .text_color(color)
            .child(ui_review::state_label(state))
            .into_any_element(),
    ];
    if let Some(author) = &review.author {
        facts.push(div().child(format!("by {author}")).into_any_element());
    }
    if !review.target_branch.is_empty() {
        let target = div()
            .font_family(theme.mono_font.clone())
            .text_color(theme.blue)
            .child(review.target_branch.clone());
        facts.push(
            h_flex()
                .gap(px(4.))
                .child("into")
                .child(target)
                .into_any_element(),
        );
    }
    if let Some(at) = review.updated_at {
        facts.push(
            div()
                .id("review-updated")
                .child(format!("updated {}", ago(at)))
                .tooltip(Tooltip::text(local_time(at)))
                .into_any_element(),
        );
    }
    let mut row = h_flex()
        .pl(px(24.))
        .flex_wrap()
        .gap(px(6.))
        .text_size(px(12.))
        .text_color(theme.text_2);
    for (ix, fact) in facts.into_iter().enumerate() {
        if ix > 0 {
            row = row.child(div().text_color(theme.text_3).child("·"));
        }
        row = row.child(fact);
    }
    let row = row
        .children(review.labels.iter().map(|label| {
            div()
                .px(px(6.))
                .rounded(px(4.))
                .border_1()
                .border_color(theme.border_2)
                .text_size(px(11.))
                .child(label.clone())
        }))
        .children(fresh);
    let open = review.url.clone().map(|url| {
        button("open-review", format!("Open on {forge}"))
            .icon(Icon::Link)
            .small()
            .style(ButtonStyle::Ghost)
            .tooltip_with_key(
                "Open the pull request",
                shortcut(&crate::app::OpenPullRequest, cx),
            )
            .on_click(move |_, _, cx| cx.open_url(&url))
    });
    v_flex()
        .gap(px(8.))
        .child(
            h_flex()
                .items_start()
                .gap(px(8.))
                .child(
                    div()
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .child(icon(glyph).text_color(color)),
                )
                .child(title_text(title).flex_1())
                .children(open),
        )
        .child(row)
        .when(!review.reviewers.is_empty(), |el| {
            let reviewers = review.reviewers.join(", ");
            el.child(muted(format!("Review requested from {reviewers}"), theme).pl(px(24.)))
        })
}

/// A section of the pull request tab: its title, and what it comes to beside it.
fn section(title: &'static str, hint: Option<String>, theme: &Theme) -> Div {
    v_flex().gap(px(6.)).child(
        h_flex()
            .gap(px(8.))
            .pb(px(2.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .children(hint.map(|hint| faint(hint, theme))),
    )
}

/// A line of a section that does something when clicked.
fn row(id: impl Into<gpui::ElementId>, theme: &Theme) -> Stateful<Div> {
    let hover = theme.hover();
    h_flex()
        .id(id)
        .h(px(28.))
        .gap(px(8.))
        .px(px(8.))
        .mx(px(-8.))
        .rounded(px(4.))
        .text_size(px(13.))
        .cursor_pointer()
        .hover(move |el| el.bg(hover))
}

/// A check: how it stands, its name, how it finished and how long it took, and when; a
/// click opens its log.
fn check_row(ix: usize, run: &CheckRun, theme: &Theme) -> Stateful<Div> {
    let (glyph, color) = match run.failed {
        Some(true) => (Icon::Cross, theme.danger),
        None => (Icon::Clock, theme.warn),
        Some(false) if run.status == "skipped" => (Icon::Tick, theme.text_3),
        Some(false) => (Icon::Tick, theme.safe),
    };
    let status = match duration(run) {
        Some(took) => format!("{} · {took}", run.status),
        None => run.status.clone(),
    };
    let when = (run.completed_at.map(ago))
        .or_else(|| run.started_at.map(|at| format!("started {}", ago(at))));
    let url = run.url.clone();
    row(("check", ix), theme)
        .when(!url.is_empty(), |el| {
            el.tooltip(Tooltip::text("Open its log"))
                .on_click(move |_, _, cx| cx.open_url(&url))
        })
        .child(small_icon(glyph).text_color(color))
        .child(div().min_w(px(0.)).truncate().child(run.name.clone()))
        .child(faint(status, theme).flex_none())
        .child(div().flex_1())
        .children(when.map(|when| faint(when, theme).flex_none()))
}

/// How long a finished check ran: "42s", "3m 5s", "1h 12m".
fn duration(run: &CheckRun) -> Option<String> {
    let seconds = (run.completed_at? - run.started_at?).max(0);
    Some(match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    })
}

/// `seconds` (since the epoch) as how long ago: "just now", "5m ago".
fn ago(seconds: i64) -> String {
    match relative_time(seconds).as_str() {
        "now" => "just now".into(),
        ago => format!("{ago} ago"),
    }
}

/// A tab of a page's header, underlined along the header's edge while `selected`.
fn tab(id: &'static str, label: &'static str, selected: bool, theme: &Theme) -> Stateful<Div> {
    let hover = theme.text_1;
    h_flex()
        .id(id)
        .relative()
        .gap(px(6.))
        .px(px(8.))
        .pt(px(4.))
        .pb(px(10.))
        .text_size(px(13.))
        .text_color(if selected { theme.text_1 } else { theme.text_2 })
        .cursor_pointer()
        .hover(move |el| el.text_color(hover))
        .child(label)
        .when(selected, |el| {
            el.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left(px(8.))
                    .right(px(8.))
                    .h(px(2.))
                    .bg(theme.text_1),
            )
        })
}

//! A branch's pull request, on a tab of the branch's page beside its changes: what it is,
//! where it stands, its description and its checks, as the forge cache last saw them.
//! The store's background refresh asks the forge, and the page says how old what it shows
//! is. Only whether an open one can merge is asked as the tab shows it, as the cache doesn't
//! keep that.

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, MouseButton, SharedString, Stateful, Window, div,
    prelude::*,
};

use super::Detail;
use super::panes::title_text;
use super::resolve::{banner_action, warn_banner};
use crate::backend::reviews::MergeMethod;
use crate::backend::{CheckRun, Forge, Review, ReviewState};
use crate::overlays::MenuItem;
use crate::pr::{Mergeability, Offer};
use crate::state::{ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::markdown;
use crate::ui::review as ui_review;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, ago, button, count_badge, dp, dropdown_button, duration, faint,
    h_flex, icon, keep_focus_on_press, list_row, local_time, muted, now, small_icon, v_flex,
};

/// How tall the tabs over the diff beside a list are.
pub(super) const TABS_HEIGHT: f32 = 40.;

/// How old, in seconds, what the pull request tab shows can get before a failed refresh
/// warns about it.
const STALE_AFTER: i64 = 30 * 60;

impl Detail {
    /// The selected branch's pull request, if it has one: the branch page's, the Branches
    /// page's, or the one the target commit selected, on the project page or in the
    /// navigator, landed.
    pub(super) fn selected_review<'a>(&'a self, cx: &'a App) -> Option<&'a Review> {
        let store = self.store.read(cx);
        match store.selection.as_ref()? {
            Selection::Branch(project, ref_name) => store
                .workspace(project)?
                .find_branch(ref_name)?
                .review
                .as_ref(),
            Selection::Branches(project) => self.selected_other_branch(project)?.review.as_ref(),
            Selection::Project(project) if self.showing_target_commit(project, cx) => {
                self.selected_target_commit(project)?.review.as_ref()
            }
            Selection::Upstream(project, ref_name, id) => (store.workspace(project)?)
                .upstream(ref_name.as_deref())
                .iter()
                .find(|commit| commit.id == *id)?
                .review
                .as_ref(),
            _ => None,
        }
    }

    /// Whether the branch page shows its pull request in place of its changes. The tab
    /// picked stays picked from branch to branch, as but.dev's does; a branch without a
    /// pull request shows its changes.
    pub fn showing_review(&self, cx: &App) -> bool {
        self.review_tab && self.selected_review(cx).is_some()
    }

    /// A list page's list keeps the keys, and the branch page's file list, which isn't drawn
    /// beside its pull request, takes them back from the diff.
    fn set_review_tab(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.review_tab = on;
        self.focus_files(window, cx);
        self.read_merge_status(cx);
    }

    /// The selected branch, in the main checkout, and its pull request, while the tab shows
    /// it open: what the merge box is for, as the pull request actions are.
    fn merge_box_review<'a>(&'a self, cx: &'a App) -> Option<(&'a ProjectId, &'a str, &'a Review)> {
        let store = self.store.read(cx);
        let selection = store.selection.as_ref()?;
        let Selection::Branch(project, ref_name) = selection else {
            return None;
        };
        let review = self.selected_review(cx)?;
        let applies = self.review_tab
            && crate::pr::on_forge(store, selection)
            && store.worktree_of(selection).is_none()
            && store.review_state(project, review) == ReviewState::Open;
        applies.then_some((project, ref_name.as_str(), review))
    }

    /// Have the store ask the forge whether the pull request the tab shows can merge, for its
    /// merge box, when it hasn't lately.
    pub(super) fn read_merge_status(&self, cx: &mut Context<Self>) {
        let Some((project, _, review)) = self.merge_box_review(cx) else {
            return;
        };
        let (project, review) = (project.clone(), review.clone());
        (self.store).update(cx, |store, cx| {
            store.read_merge_status(&project, &review, cx)
        });
    }

    /// Where the open pull request stands on merging, and the one thing that offers, as
    /// GitHub's merge box has it.
    fn merge_box(&self, theme: &Theme, cx: &Context<Self>) -> Option<Div> {
        let (project, branch, review) = self.merge_box_review(cx)?;
        let store = self.store.read(cx);
        let merge = crate::pr::mergeability(store, project, review);
        let (glyph, color, title, note): (_, _, SharedString, Option<SharedString>) = match &merge {
            Mergeability::AutoMerging => (
                Icon::Clock,
                theme.safe,
                "Auto-merge is on".into(),
                Some("It merges once its required checks and reviews pass".into()),
            ),
            Mergeability::Ready(note) => (
                Icon::Tick,
                if note.is_some() {
                    theme.warn
                } else {
                    theme.safe
                },
                "Ready to merge".into(),
                note.map(Into::into),
            ),
            Mergeability::Waiting(why) => (Icon::Clock, theme.warn, (*why).into(), None),
            Mergeability::Blocked(why) => (Icon::Cross, theme.danger, why.clone().into(), None),
            Mergeability::Unknown(None) => (
                Icon::Clock,
                theme.text_3,
                "Checking whether it can merge…".into(),
                None,
            ),
            Mergeability::Unknown(Some(err)) => (
                Icon::Cross,
                theme.warn,
                "Couldn't ask whether it can merge".into(),
                Some(err.clone().into()),
            ),
        };
        // Turning auto-merge on or off shows at once, until the forge has it.
        let waiting = store.auto_merge_on(project, review) != review.auto_merge;
        let method = crate::pr::merge_method(store, project);
        let action = merge.offer().map(|offer| {
            let action = button("merge-offer", offer.label(method))
                .small()
                .style(match offer {
                    Offer::Merge => ButtonStyle::Pop,
                    Offer::EnableAutoMerge | Offer::DisableAutoMerge => ButtonStyle::Outline,
                })
                .disabled_because(match offer {
                    _ if waiting => Some("Waiting for the forge"),
                    Offer::Merge | Offer::EnableAutoMerge => {
                        crate::pr::push_first(store, project, branch)
                    }
                    Offer::DisableAutoMerge => None,
                })
                .dispatch(offer.action())
                .tooltip(match offer {
                    Offer::Merge => "Merge it now on the forge",
                    Offer::EnableAutoMerge => {
                        "Merge it by itself once its required checks and reviews pass"
                    }
                    Offer::DisableAutoMerge => "Leave it to merge by hand",
                });
            match (offer, method) {
                (Offer::Merge, Some(method)) => {
                    let project = project.clone();
                    let open = cx.listener(move |this, event: &ClickEvent, _, cx| {
                        this.open_merge_methods(&project, method, event, cx)
                    });
                    dropdown_button(action, "Merge method", open).into_any_element()
                }
                _ => action.into_any_element(),
            }
        });
        Some(
            h_flex()
                .gap(dp(10.))
                .px(dp(12.))
                .py(dp(10.))
                .rounded(dp(6.))
                .border_1()
                .border_color(theme.border_2)
                .child(icon(glyph).text_color(color))
                .child(
                    v_flex()
                        .flex_grow(1.)
                        .min_w_0()
                        .child(
                            div()
                                .text_size(dp(13.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(title),
                        )
                        .children(note.map(|note| muted(note, theme).truncate())),
                )
                .children(action),
        )
    }

    /// The ways to merge, as GitHub's merge button offers them, ticking the one `project`
    /// merges by. Picking one keeps it for the project's next merge.
    fn open_merge_methods(
        &mut self,
        project: &ProjectId,
        current: MergeMethod,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        let items = [
            (MergeMethod::Merge, "Create a merge commit"),
            (MergeMethod::Squash, "Squash and merge"),
            (MergeMethod::Rebase, "Rebase and merge"),
        ]
        .map(|(method, label)| {
            let project = project.to_string();
            MenuItem::new(label, move |app, _, cx| {
                let project = project.clone();
                (app.store).update(cx, |store, cx| {
                    store.set_prefs(|p| _ = p.merge_methods.insert(project, method), cx)
                })
            })
            .ticked(method == current)
        });
        self.open_menu(event.position(), items.into(), cx);
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
            .ml(dp(-8.))
            .child(
                tab(
                    "tab-changes",
                    Icon::Diff,
                    "Changes",
                    !self.review_tab,
                    theme,
                )
                .children(files.map(|n| count_badge(n, theme)))
                .on_click(
                    cx.listener(|this, _, window, cx| this.set_review_tab(false, window, cx)),
                ),
            )
            .child(
                tab(
                    "tab-review",
                    Icon::Pr,
                    "Pull request",
                    self.review_tab,
                    theme,
                )
                .children(checks.map(|checks| {
                    let (glyph, color, _) = ui_review::checks_summary(checks, theme);
                    small_icon(glyph).text_color(color)
                }))
                .on_click(cx.listener(|this, _, window, cx| this.set_review_tab(true, window, cx))),
            )
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
            .max_w(dp(760.))
            .px(dp(24.))
            .py(dp(20.))
            .gap(dp(24.))
            .child(head(
                review,
                state,
                forge,
                fresh.map(IntoElement::into_any_element),
                theme,
            ))
            .children(stale)
            .children(self.merge_box(theme, cx))
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
                    list_row("passed-checks", false, theme)
                        .mx(dp(-8.))
                        .text_size(dp(13.))
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
                .gap(dp(4.))
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
        .pl(dp(24.))
        .flex_wrap()
        .gap(dp(6.))
        .text_size(dp(12.))
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
                .px(dp(6.))
                .rounded(dp(4.))
                .border_1()
                .border_color(theme.border_2)
                .text_size(dp(11.))
                .child(label.clone())
        }))
        .children(fresh);
    let open = review.url.clone().map(|url| {
        button("open-review", format!("Open on {forge}"))
            .icon(Icon::Link)
            .small()
            .style(ButtonStyle::Ghost)
            .tooltip_for("Open the pull request", &crate::app::OpenPullRequest)
            .on_click(move |_, _, cx| cx.open_url(&url))
    });
    v_flex()
        .gap(dp(8.))
        .child(
            h_flex()
                .items_start()
                .gap(dp(8.))
                .child(
                    div()
                        .h(dp(22.))
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
            el.child(muted(format!("Review requested from {reviewers}"), theme).pl(dp(24.)))
        })
}

/// A section of the pull request tab: its title, and what it comes to beside it.
fn section(title: &'static str, hint: Option<String>, theme: &Theme) -> Div {
    v_flex().gap(dp(6.)).child(
        h_flex()
            .gap(dp(8.))
            .pb(dp(2.))
            .child(
                div()
                    .text_size(dp(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .children(hint.map(|hint| faint(hint, theme))),
    )
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
    // How long a finished check ran.
    let status = match run.completed_at.zip(run.started_at) {
        Some((end, start)) => format!("{} · {}", run.status, duration((end - start).max(0))),
        None => run.status.clone(),
    };
    let when = (run.completed_at.map(ago))
        .or_else(|| run.started_at.map(|at| format!("started {}", ago(at))));
    let url = run.url.clone();
    list_row(("check", ix), false, theme)
        .mx(dp(-8.))
        .text_size(dp(13.))
        .when(!url.is_empty(), |el| {
            el.tooltip(Tooltip::text("Open its log"))
                .on_click(move |_, _, cx| cx.open_url(&url))
        })
        .child(small_icon(glyph).text_color(color))
        .child(div().min_w_0().truncate().child(run.name.clone()))
        .child(faint(status, theme).flex_none())
        .child(div().flex_1())
        .children(when.map(|when| faint(when, theme).flex_none()))
}

/// A tab of a page's header, as ui-react's and but.dev's: its glyph, its label faded unless
/// it's selected or under the pointer, and under the selected one a bar along the header's edge.
fn tab(
    id: &'static str,
    glyph: Icon,
    label: &'static str,
    selected: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = theme.hover();
    h_flex()
        .id(id)
        .group(id)
        .relative()
        .h(dp(28.))
        // Room for the bar, 6 below, down to the header's edge.
        .mb(dp(10.))
        .gap(dp(6.))
        .px(dp(8.))
        .rounded(dp(4.))
        .text_size(dp(13.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.text_1)
        .when(!selected, |el| el.hover(move |el| el.bg(hover)))
        .on_mouse_down(MouseButton::Left, keep_focus_on_press)
        .child(icon(glyph).text_color(theme.text_1.opacity(0.7)))
        .child(
            div()
                .opacity(if selected { 0.9 } else { 0.63 })
                .group_hover(id, |el| el.opacity(0.9))
                .child(label),
        )
        .when(selected, |el| {
            el.child(
                div()
                    .absolute()
                    .top(dp(34.))
                    .left(dp(8.))
                    .right(dp(8.))
                    .h(dp(4.))
                    .bg(theme.fill_gray),
            )
        })
}

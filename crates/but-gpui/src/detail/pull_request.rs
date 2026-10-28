//! A branch's pull request, on a tab of the branch's page beside its changes: what it is,
//! where it stands, its description and its checks, as the forge cache last saw them.
//! The store's background refresh asks the forge, and the page says how old what it shows
//! is. Only whether an open one can merge is asked as the tab shows it, as the cache doesn't
//! keep that.

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Div, Hsla, MouseButton, Role,
    ScrollHandle, SharedString, Stateful, Toggled, Window, div, list, prelude::*,
};

use super::Detail;
use super::panes::measured;
use super::resolve::{banner_action, warn_banner};
use crate::backend::reviews::MergeMethod;
use crate::backend::{CheckRun, Forge, FullName, Review, ReviewState};
use crate::overlays::MenuItem;
use crate::pr::{Mergeability, Offer};
use crate::state::{ProjectId, Selection, snack};
use crate::theme::Theme;
use crate::ui::markdown;
use crate::ui::person::forge_avatar;
use crate::ui::review as ui_review;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, ago, button, count_badge, dot, dp, dropdown_button, duration,
    h_flex, icon, keep_focus_on_press, local_time, muted, now, plural, relative_time, shortcut,
    small_icon, switch_grounds, switch_spring, tab_stop, v_flex,
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
        match &self.selection(cx)? {
            Selection::Branch(project, ref_name) => store
                .workspace(project)?
                .find_branch(ref_name.as_ref())?
                .review
                .as_ref(),
            Selection::Branches(project) => self.selected_other_branch(project)?.review.as_ref(),
            Selection::Project(project) if self.showing_target_commit(project, cx) => {
                self.selected_target_commit(project)?.review.as_ref()
            }
            Selection::Upstream(project, ref_name, id) => (store.workspace(project)?)
                .upstream(ref_name.as_ref())
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
    /// ⌃Tab: the other of a branch's Changes and Pull request tabs, when it has both.
    pub(crate) fn switch_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_review(cx).is_some() {
            self.set_review_tab(!self.review_tab, window, cx)
        }
    }

    fn set_review_tab(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.review_tab = on;
        self.focus_files(window, cx);
        self.ask_forge(cx);
    }

    /// The selected branch and its pull request, while the tab shows it open: what the merge
    /// box is for, as the pull request actions are.
    fn merge_box_review<'a>(
        &'a self,
        cx: &'a App,
    ) -> Option<(&'a ProjectId, &'a FullName, &'a Review)> {
        let store = self.store.read(cx);
        let selection = store.selection.as_ref()?;
        let Selection::Branch(project, Some(ref_name)) = selection else {
            return None;
        };
        let review = self.selected_review(cx)?;
        let applies = self.review_tab
            && crate::pr::on_forge(store, selection)
            && store.review_state(project, review) == ReviewState::Open;
        applies.then_some((project, ref_name, review))
    }

    /// Have the store ask the forge, when it hasn't lately, what the tab shows that the forge
    /// cache doesn't keep: whether the pull request can merge, and what's been said on it.
    pub(super) fn ask_forge(&self, cx: &mut Context<Self>) {
        self.read_merge_status(cx);
        self.read_conversation(cx);
    }

    /// Have the store ask GitHub what's been said on the pull request the tab shows. Other
    /// forges' conversations aren't read.
    fn read_conversation(&self, cx: &mut Context<Self>) {
        if !self.review_tab {
            return;
        }
        let review = self.selected_review(cx);
        let (Some(project), Some((number, updated_at))) = (
            self.selected_project(cx),
            review.map(|review| (review.number, review.updated_at)),
        ) else {
            return;
        };
        let store = self.store.read(cx);
        if store.workspace(&project).and_then(|ws| ws.forge) == Some(Forge::GitHub) {
            (self.store).update(cx, |store, cx| {
                store.read_conversation(&project, number, updated_at, cx)
            });
        }
    }

    /// Have the store ask the forge whether the pull request the tab shows can merge, for its
    /// merge box, when it hasn't lately.
    fn read_merge_status(&self, cx: &mut Context<Self>) {
        let Some((project, _, review)) = self.merge_box_review(cx) else {
            return;
        };
        let (project, review) = (project.clone(), review.clone());
        (self.store).update(cx, |store, cx| {
            store.read_merge_status(&project, &review, cx)
        });
    }

    /// Merging, as Lite's Merge section: auto-merge as a switch and Merge with its method, each
    /// saying in its tooltip why it can't be pressed when GitHub's merge box wouldn't offer it.
    fn merge_section(&self, theme: &Theme, cx: &Context<Self>) -> Option<Div> {
        let (project, branch, review) = self.merge_box_review(cx)?;
        let store = self.store.read(cx);
        let merge = crate::pr::mergeability(store, project, review);
        let (title, ..) = headline(&merge, theme);
        let offer = merge.offer();
        // Turning auto-merge on or off shows at once, until the forge has it.
        let waiting = (store.auto_merge_on(project, review) != review.auto_merge)
            .then_some("Waiting for the forge");
        let push_first = crate::pr::push_first(store, project, branch);
        let auto_why: Option<SharedString> = match offer {
            _ if waiting.is_some() => waiting.map(Into::into),
            Some(Offer::EnableAutoMerge) => push_first.map(Into::into),
            Some(Offer::DisableAutoMerge) => None,
            Some(Offer::Merge) if matches!(merge, Mergeability::Ready(_)) => {
                Some("It can merge now".into())
            }
            Some(Offer::Merge) | None => Some(title.clone()),
        };
        let merge_why: Option<SharedString> = match offer {
            _ if waiting.is_some() => waiting.map(Into::into),
            Some(Offer::Merge) => push_first.map(Into::into),
            Some(Offer::EnableAutoMerge | Offer::DisableAutoMerge) | None => Some(title),
        };
        let method = crate::pr::merge_method(store, project);
        let merge_button = button("merge-offer", merge_label(method))
            .small()
            .style(ButtonStyle::Pop)
            .disabled_because(merge_why)
            .dispatch(Offer::Merge.action())
            .tooltip("Merge it now on the forge");
        let merge_button = match method {
            Some(method) => {
                let project = project.clone();
                let open = cx.listener(move |this, event: &ClickEvent, _, cx| {
                    this.open_merge_methods(&project, method, event, cx)
                });
                dropdown_button(merge_button, "Merge method", open).into_any_element()
            }
            None => merge_button.into_any_element(),
        };
        let on = matches!(merge, Mergeability::AutoMerging);
        let actions = h_flex()
            .flex_wrap()
            .justify_end()
            .gap(dp(8.))
            .child(auto_merge_switch(on, auto_why, theme))
            .child(merge_button);
        Some(side_section(
            "Merge",
            Some(actions.into_any_element()),
            theme,
        ))
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
        let project = project.to_string();
        let items = move |_: &mut Self, _: &mut Context<Self>| {
            [
                (MergeMethod::Merge, "Create a merge commit"),
                (MergeMethod::Squash, "Squash and merge"),
                (MergeMethod::Rebase, "Rebase and merge"),
            ]
            .map(|(method, label)| {
                let project = project.clone();
                MenuItem::new(label, move |app, _, cx| {
                    let project = project.clone();
                    (app.store).update(cx, |store, cx| {
                        store.set_prefs(|p| _ = p.merge_methods.insert(project, method), cx)
                    })
                })
                .ticked(method == current)
            })
            .into()
        };
        self.open_menu(event.position(), items, cx);
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
                    // Its number, as Lite's tab names it.
                    match review.state {
                        ReviewState::Draft => format!("Draft #{}", review.number),
                        _ => format!("PR #{}", review.number),
                    },
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

    /// The pull request tab, as Lite lays it out: its title, who opened it, its description
    /// and its conversation, beside a panel of where it stands, each scrolling on its own.
    /// Narrower than [`TWO_COLUMNS`], the panel goes between the title and the description.
    /// The column is a list, so a long conversation draws only what's on screen.
    pub(super) fn render_review_tab(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(review) = self.selected_review(cx) else {
            return div().into_any_element();
        };
        let two_columns = self.review_width.get() >= TWO_COLUMNS;
        let (rows, drawn) = (
            LEAD + self.remarks_shown(project, review.number, cx),
            self.review_list.item_count(),
        );
        // New remarks arrive at the top of the conversation, newest first. A splice there moves
        // the list's scroll position past them, so the reader stays on the remark they're on.
        match drawn {
            0 => self.review_list.reset(rows),
            _ if rows > drawn => self.review_list.splice(LEAD..LEAD, rows - drawn),
            _ if rows < drawn => self.review_list.splice(LEAD..LEAD + drawn - rows, 0),
            _ => {}
        }
        let column = scrolled(
            list(
                self.review_list.clone(),
                cx.processor(|this, ix, _, cx| this.review_row(ix, cx)),
            )
            .size_full(),
            "review-scrollbar",
            Scroll::List(self.review_list.clone()),
            theme,
        )
        .h_full();
        // Side by side, each scrolls on its own, so neither a long description nor a long
        // list of checks takes the other out of sight. Narrow, the panel goes between the
        // title and the description, so the page still opens on what the pull request is.
        let page = match two_columns {
            true => h_flex()
                .size_full()
                .max_w(dp(1000. + 2. * 24.))
                .child(column.min_w_0())
                .child(
                    scrolling(
                        "review-side",
                        "review-side-scrollbar",
                        self.side(project, review, theme, cx)
                            .pl(dp(12.))
                            .pr(dp(24.)),
                        &self.review_side_scroll,
                        theme,
                    )
                    .flex_none()
                    .w(dp(12. + PANEL_WIDTH + 24.)),
                ),
            false => column,
        };
        let width = measured(&self.review_width, &self.app, |bounds| {
            bounds.size.width.into()
        });
        (div().relative().flex_1().min_h_0().w_full())
            .child(page)
            .child(width)
            .into_any_element()
    }

    /// Row `ix` of the pull request tab's column: its title, the panel when it isn't beside
    /// the column, its description, the conversation's heading, then each of what's been said.
    fn review_row(&self, ix: usize, cx: &Context<Self>) -> AnyElement {
        let (Some(project), Some(review)) = (self.selected_project(cx), self.selected_review(cx))
        else {
            return div().into_any_element();
        };
        let theme = Theme::get(cx);
        let two_columns = self.review_width.get() >= TWO_COLUMNS;
        // Each row's content and the room above it. The list ignores a row's own margins, so
        // the room and the column's sides go on a row of its own around the content.
        let (above, content) = match ix {
            0 => (16., Some(self.head(&project, review, theme, cx))),
            1 => (
                20.,
                (!two_columns).then(|| self.side(&project, review, theme, cx)),
            ),
            2 => (
                if two_columns { 12. } else { 20. },
                Some(description(review, theme)),
            ),
            3 => (24., self.conversation(&project, review, theme, cx)),
            _ => (
                6.,
                self.remark_at(&project, review.number, ix - LEAD, theme, cx),
            ),
        };
        // Not the list's count: the list is borrowed while it draws its rows.
        let last = ix + 1 == LEAD + self.remarks_shown(&project, review.number, cx);
        let Some(content) = content else {
            return div().when(last, |el| el.h(dp(24.))).into_any_element();
        };
        match two_columns {
            true => div().pl(dp(24.)).pr(dp(12.)),
            false => div().px(dp(24.)),
        }
        .pt(dp(above))
        .when(last, |el| el.pb(dp(24.)))
        .child(content)
        .into_any_element()
    }

    /// The tab's title, who opened it and when it last changed, under a banner when what it
    /// shows has aged since the forge last answered.
    fn head(&self, project: &ProjectId, review: &Review, theme: &Theme, cx: &App) -> Div {
        let (_, stale) = self.freshness(project, review, theme, cx);
        let title = match review.title.as_str() {
            "" => format!("Pull request #{}", review.number),
            title => title.to_owned(),
        };
        v_flex()
            .gap(dp(12.))
            .children(stale)
            .child(
                div()
                    .text_size(dp(18.))
                    .line_height(dp(24.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .children(byline(review, theme))
    }

    /// The panel of where the pull request stands: its checklist over its card of facts.
    fn side(&self, project: &ProjectId, review: &Review, theme: &Theme, cx: &Context<Self>) -> Div {
        let store = self.store.read(cx);
        let forge =
            (store.workspace(project).and_then(|ws| ws.forge)).map_or("the forge", Forge::name);
        let state = store.review_state(project, review);
        let (fresh, _) = self.freshness(project, review, theme, cx);
        v_flex()
            .gap(dp(12.))
            .children(self.checklist(project, review, state, fresh, theme, cx))
            .child(self.side_sections(project, review, state, forge, theme, cx))
    }

    /// How fresh what the tab shows is: a button that asks the forge again, saying when it
    /// last answered, or once a failed ask has let what shows age, a banner saying so. Neither
    /// for a merged or closed one, which is settled.
    fn freshness(
        &self,
        project: &ProjectId,
        review: &Review,
        theme: &Theme,
        cx: &App,
    ) -> (Option<crate::ui::Button>, Option<Stateful<Div>>) {
        let store = self.store.read(cx);
        let forge =
            (store.workspace(project).and_then(|ws| ws.forge)).map_or("the forge", Forge::name);
        // A refresh that changes nothing reloads nothing, so the store's time can be newer.
        let synced = store.forge_synced(project).max(review.synced_at);
        let (checking, failed) = (store.forge_checking(project), store.forge_failed(project));
        if matches!(
            store.review_state(project, review),
            ReviewState::Merged | ReviewState::Closed
        ) {
            return (None, None);
        }
        let ask = {
            let (store, project) = (self.store.clone(), project.clone());
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |store, _| store.refresh_forge(&project))
            }
        };
        // A failed refresh is worth a warning only once what shows has aged; until then the
        // button to check again is enough.
        if failed && synced.is_none_or(|at| now() - at > STALE_AFTER) {
            let banner = warn_banner(
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
            );
            return (None, Some(banner));
        }
        let fresh = button(
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
        .on_click(ask);
        (Some(fresh), None)
    }

    /// What stands between the pull request and merging, as Lite's checklist: where an open
    /// one stands, over a row for each thing the forge has said about it. Its heading says
    /// how fresh that is with `fresh`, the button to ask again.
    fn checklist(
        &self,
        project: &ProjectId,
        review: &Review,
        state: ReviewState,
        fresh: Option<impl IntoElement>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let store = self.store.read(cx);
        let headline = (self.merge_box_review(cx)).map(|(project, _, review)| {
            headline(&crate::pr::mergeability(store, project, review), theme)
        });
        // Waiting on reviewers or conflicts is news only while it's open.
        let open = matches!(state, ReviewState::Open | ReviewState::Draft);
        let mut rows: Vec<(String, Option<Hsla>)> = Vec::new();
        if let Some(checks) = review.checks {
            rows.push(match checks {
                _ if checks.failed > 0 => ("Checks failed".into(), Some(theme.danger)),
                _ if checks.running > 0 => ("Checks running".into(), Some(theme.warn)),
                _ => ("Checks passed".into(), Some(theme.safe)),
            });
        }
        // GitHub lists only the reviewers who haven't answered; the others keep everyone.
        let github = store.workspace(project).and_then(|ws| ws.forge) == Some(Forge::GitHub);
        if open && github && !review.reviewers.is_empty() {
            let pending = plural(review.reviewers.len(), "review");
            rows.push((format!("{pending} pending"), None));
        }
        let status = (store.merge_status(project, review.number))
            .and_then(|status| status.as_ref().ok())
            .filter(|_| open);
        match status.and_then(|status| status.mergeable_state.as_deref()) {
            Some("dirty" | "cannot_be_merged") => {
                rows.push(("Merge conflicts".into(), Some(theme.warn)))
            }
            Some("clean" | "unstable" | "has_hooks" | "can_be_merged") => {
                rows.push(("No conflicts".into(), Some(theme.safe)))
            }
            _ => {}
        }
        if headline.is_none() && rows.is_empty() && fresh.is_none() {
            return None;
        }
        let clear = rows
            .iter()
            .filter(|(_, tone)| *tone == Some(theme.safe))
            .count();
        let mono = |text: String, color: Hsla| {
            div()
                .font_family(theme.mono_font.clone())
                .text_size(dp(10.))
                .text_color(color)
                .child(text)
        };
        Some(
            card(theme)
                .p(dp(14.))
                .gap(dp(8.))
                .child(
                    // As tall as its words, with the button overhanging, so the rows below
                    // stay put as it comes and goes.
                    h_flex()
                        .h(dp(14.))
                        .gap(dp(8.))
                        .child(mono("CHECKLIST".into(), theme.text_2))
                        .when(!rows.is_empty(), |el| {
                            el.child(mono(
                                format!("{clear} of {} checks", rows.len()),
                                theme.text_3,
                            ))
                        })
                        .children(fresh.map(|fresh| div().ml_auto().mr(dp(-6.)).child(fresh))),
                )
                .children(headline.map(|(title, color, note)| {
                    v_flex()
                        .pb(dp(2.))
                        .child(
                            div()
                                .text_size(dp(14.))
                                .line_height(dp(18.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(color)
                                .child(title),
                        )
                        // An error can run long; its whole text is on hover.
                        .children(note.map(|note| {
                            muted(note.clone(), theme)
                                .id("merge-note")
                                .line_clamp(2)
                                .text_ellipsis()
                                .tooltip(Tooltip::text(note))
                        }))
                }))
                .child(
                    v_flex()
                        .gap(dp(7.))
                        .children(rows.into_iter().map(|(label, tone)| {
                            let dot = match tone {
                                Some(color) => dot(color),
                                None => dot(gpui::transparent_black())
                                    .border_1()
                                    .border_color(theme.text_3),
                            };
                            h_flex()
                                .gap(dp(8.))
                                .text_size(dp(12.))
                                .child(dot)
                                .child(label)
                        })),
                ),
        )
    }

    /// The panel's card of facts, as Lite's: merging, the state and a link to it, the
    /// checks, who's asked to review, labels, branches and when it was opened.
    fn side_sections(
        &self,
        project: &ProjectId,
        review: &Review,
        state: ReviewState,
        forge: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let store = self.store.read(cx);
        let github = store.workspace(project).and_then(|ws| ws.forge) == Some(Forge::GitHub);
        let sections = [
            self.merge_section(theme, cx),
            Some(status_section(review, state, forge, theme)),
            (!review.check_runs.is_empty()).then(|| self.checks_section(review, theme, cx)),
            reviewers_section(review, github, theme),
            labels_section(review, theme),
            self.branches_section(review, theme),
            review.created_at.map(|at| {
                side_section("Created", None, theme).child(
                    div()
                        .text_size(dp(13.))
                        .text_color(theme.text_2)
                        .child(format!("{}, {}", ago(at), local_time(at))),
                )
            }),
        ];
        card(theme).children(
            (sections.into_iter().flatten().enumerate()).map(|(ix, section)| {
                section.when(ix > 0, |el| el.border_t_1().border_color(theme.border_3))
            }),
        )
    }

    /// The checks, as Lite's: a bar of how they stand and what that comes to, then those that
    /// failed or still run, the news, with the rest folded under the heading.
    fn checks_section(&self, review: &Review, theme: &Theme, cx: &Context<Self>) -> Div {
        let runs = &review.check_runs;
        let failed = runs.iter().filter(|run| run.failed == Some(true)).count();
        let running = runs.iter().filter(|run| run.failed.is_none()).count();
        let skipped = (runs.iter())
            .filter(|run| run.failed == Some(false) && run.status == "Skipped")
            .count();
        let passed = runs.len() - failed - running - skipped;
        // Skipped ones ran nothing, so they're counted but take no room on the bar.
        let bar = h_flex().h(dp(6.)).gap(dp(2.)).children(
            [
                (failed, theme.danger),
                (running, theme.text_3),
                (passed, theme.safe),
            ]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, color)| {
                div()
                    .h_full()
                    .flex_basis(dp(0.))
                    .flex_grow(n as f32)
                    .rounded_full()
                    .bg(color)
            }),
        );
        let counts = h_flex()
            .flex_wrap()
            .gap(dp(8.))
            .text_size(dp(12.))
            .children(
                [
                    (failed, format!("{failed} failed"), theme.danger),
                    (
                        passed,
                        match passed == runs.len() {
                            true => format!("All {passed} passed"),
                            false => format!("{passed} passed"),
                        },
                        theme.safe,
                    ),
                    (running, format!("{running} running"), theme.text_2),
                    (skipped, format!("{skipped} skipped"), theme.text_3),
                ]
                .into_iter()
                .filter(|(n, ..)| *n > 0)
                .map(|(_, text, color)| div().text_color(color).child(text)),
            );
        let (live, rest): (Vec<_>, Vec<_>) =
            (runs.iter().enumerate()).partition(|(_, run)| run.failed != Some(false));
        let open = self.passed_checks_open;
        let heading = h_flex()
            .id("passed-checks")
            .gap(dp(4.))
            .ml(dp(-4.))
            .text_size(dp(12.))
            .text_color(theme.text_2)
            .when(!rest.is_empty(), |el| {
                let tip = match open {
                    true => "Fold the checks that passed",
                    false => "List the checks that passed",
                };
                el.role(Role::Button)
                    .aria_label(tip)
                    .child(
                        small_icon(match open {
                            true => Icon::ChevronDown,
                            false => Icon::ChevronRight,
                        })
                        .text_color(theme.text_3),
                    )
                    .tooltip(Tooltip::text(tip))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.passed_checks_open = !this.passed_checks_open;
                        cx.notify();
                    }))
            })
            .child("Checks");
        v_flex()
            .gap(dp(8.))
            .pl(dp(14.))
            .pr(dp(12.))
            .py(dp(12.))
            .child(h_flex().min_h(dp(20.)).child(heading))
            .child(bar)
            .child(counts)
            .when(!live.is_empty() || open, |el| {
                el.child(
                    v_flex()
                        .children(live.iter().map(|(ix, run)| check_row(*ix, run, theme)))
                        .when(open, |el| {
                            el.children(rest.iter().map(|(ix, run)| check_row(*ix, run, theme)))
                        }),
                )
            })
    }

    /// The branch it merges from, a click copying its name, and the one it merges into.
    fn branches_section(&self, review: &Review, theme: &Theme) -> Option<Div> {
        if review.target_branch.is_empty() {
            return None;
        }
        let source = (!review.source_branch.is_empty()).then(|| {
            let (name, store) = (review.source_branch.clone(), self.store.clone());
            div()
                .id("review-source")
                .role(Role::Button)
                .aria_label("Copy branch name")
                .min_w_0()
                .truncate()
                .text_color(theme.text_1)
                .border_b_1()
                .border_dashed()
                .border_color(theme.text_3)
                .hover(|el| el.text_color(theme.text_2))
                .tooltip(Tooltip::text("Copy branch name"))
                .child(name.clone())
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
                    snack(&store, format!("Copied {name}"), cx)
                })
        });
        let target = h_flex()
            .min_w_0()
            .gap(dp(6.))
            .child(div().flex_none().text_color(theme.text_3).child("→"))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.text_2)
                    .child(review.target_branch.clone()),
            );
        Some(
            side_section("Branches", None, theme).child(
                h_flex()
                    .flex_wrap()
                    .gap(dp(6.))
                    .text_size(dp(13.))
                    .children(source)
                    .child(target),
            ),
        )
    }
}

/// How wide, in design pixels, the pull request tab must be for its panel to sit beside the
/// description, as Lite's breakpoint.
pub(super) const TWO_COLUMNS: f32 = 720.;

/// How wide the panel beside the description is, as Lite's.
const PANEL_WIDTH: f32 = 315.;

/// The rows of the tab's column before what's been said: the title, the panel, empty while it's
/// beside the column so a change of layout adds or takes no row, the description and the
/// conversation's heading.
const LEAD: usize = 4;

/// What the merge button says, merging by `method`, as Lite's.
fn merge_label(method: Option<MergeMethod>) -> &'static str {
    match method {
        Some(MergeMethod::Squash) => "Squash and merge",
        Some(MergeMethod::Rebase) => "Rebase and merge",
        Some(MergeMethod::Merge) | None => "Merge",
    }
}

/// Where an open pull request stands on merging, in a few words and a colour, with more
/// when there's more to say.
fn headline(merge: &Mergeability, theme: &Theme) -> (SharedString, Hsla, Option<SharedString>) {
    match merge {
        Mergeability::AutoMerging => (
            "Auto-merge is on".into(),
            theme.safe,
            Some("It merges once its required checks and reviews pass".into()),
        ),
        Mergeability::Ready(note) => ("Ready to merge".into(), theme.safe, note.map(Into::into)),
        Mergeability::Waiting(why) => (why.clone().into(), theme.warn, None),
        Mergeability::Blocked(why) => (why.clone().into(), theme.warn, None),
        Mergeability::Unknown(None) => {
            ("Checking whether it can merge…".into(), theme.text_3, None)
        }
        Mergeability::Unknown(Some(err)) => (
            "Couldn't ask whether it can merge".into(),
            theme.warn,
            Some(err.clone().into()),
        ),
    }
}

/// Auto-merge as Lite's outlined switch button: on while it merges by itself, pressable unless
/// `why_not` says otherwise.
fn auto_merge_switch(on: bool, why_not: Option<SharedString>, theme: &Theme) -> Stateful<Div> {
    use gpui::AnimationExt as _;
    let offer = match on {
        true => Offer::DisableAutoMerge,
        false => Offer::EnableAutoMerge,
    };
    let tip: SharedString = why_not.clone().unwrap_or(match on {
        true => "Leave it to merge by hand".into(),
        false => "Merge it by itself once its required checks and reviews pass".into(),
    });
    let enabled = why_not.is_none();
    let [off_ground, on_ground] = switch_grounds(theme);
    let el = h_flex()
        .id("auto-merge")
        .role(Role::Switch)
        .aria_label("Auto-merge")
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .flex_none()
        .h(dp(20.))
        .px(dp(6.))
        .gap(dp(6.))
        .rounded(dp(4.))
        .border_1()
        .border_color(theme.control_border())
        .text_size(dp(12.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(
            h_flex()
                .w(dp(20.))
                .h(dp(12.))
                .p(dp(2.))
                .rounded_full()
                .with_spring("track", switch_spring(on), move |el, phase| {
                    el.bg(phase.interpolate_clamped(off_ground, on_ground))
                        .child(
                            div()
                                // The track's width, less its padding and the knob.
                                .ml(phase.interpolate_clamped(dp(0.), dp(8.)))
                                .size(dp(8.))
                                .rounded_full()
                                .bg(gpui::white()),
                        )
                }),
        )
        .child("Auto-merge")
        .tooltip(move |window, cx| {
            let key = enabled.then(|| shortcut(offer.action(), cx)).flatten();
            Tooltip::keyed(tip.clone(), key)(window, cx)
        });
    match enabled {
        true => tab_stop(el, theme)
            .hover(|el| el.bg(theme.hover()))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                window.dispatch_action(offer.action().boxed_clone(), cx)
            }),
        false => el.opacity(0.5),
    }
}

/// Who opened it, and when it last changed when that's later, as Lite's line under the title.
fn byline(review: &Review, theme: &Theme) -> Option<Div> {
    // The forge stamps an update on any activity, opening included, so only a later one is
    // worth saying.
    let updated =
        (review.updated_at).filter(|at| review.created_at.is_none_or(|created| at - created > 60));
    let author = review.author.as_ref().map(|author| {
        h_flex()
            .gap(dp(8.))
            .text_color(theme.text_1)
            .child(forge_avatar(
                &author.login,
                author.avatar_url.as_deref(),
                18.,
                theme,
            ))
            .child(author.login.clone())
            .into_any_element()
    });
    let updated = updated.map(|at| {
        div()
            .id("review-updated")
            .child(format!("updated {}", ago(at)))
            .tooltip(Tooltip::text(local_time(at)))
            .into_any_element()
    });
    let facts: Vec<AnyElement> = author.into_iter().chain(updated).collect();
    if facts.is_empty() {
        return None;
    }
    let mut row = h_flex()
        .flex_wrap()
        .gap(dp(6.))
        .text_size(dp(13.))
        .text_color(theme.text_2);
    for (ix, fact) in facts.into_iter().enumerate() {
        if ix > 0 {
            row = row.child(div().text_color(theme.text_3).child("·"));
        }
        row = row.child(fact);
    }
    Some(row)
}

/// The description, on a wash just wider than its text, as Lite's card; or that there's none.
fn description(review: &Review, theme: &Theme) -> Div {
    let body = markdown::parse(&review.body);
    if body.is_empty() {
        return div()
            .text_size(dp(14.))
            .text_color(theme.text_3)
            .child("No description");
    }
    div()
        .mx(dp(-12.))
        .px(dp(12.))
        .py(dp(10.))
        .rounded(dp(6.))
        // Halfway from the page to the first grey step, as Lite's: a passage, not a panel.
        .bg(theme.bg_1.blend(theme.bg_2.opacity(0.5)))
        .child(markdown::render(
            &format!("review-{}", review.number),
            &body,
            theme,
        ))
}

/// `content` as a column of the tab that scrolls on its own, under the scrollbar `bar`.
fn scrolling(
    id: &'static str,
    bar: &'static str,
    content: Div,
    scroll: &ScrollHandle,
    theme: &Theme,
) -> Div {
    let column = div()
        .id(id)
        .size_full()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(content.pt(dp(16.)).pb(dp(24.)));
    scrolled(column, bar, Scroll::Div(scroll.clone()), theme).h_full()
}

/// A card of the side panel or the conversation, as Lite's.
pub(super) fn card(theme: &Theme) -> Div {
    v_flex()
        .rounded(dp(6.))
        .border_1()
        .border_color(theme.border_3)
        .bg(theme.bg_1)
}

/// A section of the side panel's card: its heading, with what acts on it opposite.
fn side_section(heading: &'static str, action: Option<AnyElement>, theme: &Theme) -> Div {
    v_flex()
        .gap(dp(8.))
        .pl(dp(14.))
        .pr(dp(12.))
        .py(dp(12.))
        .child(
            h_flex()
                .min_h(dp(20.))
                .gap(dp(8.))
                .justify_between()
                .child(
                    div()
                        .flex_none()
                        .text_size(dp(12.))
                        .text_color(theme.text_2)
                        .child(heading),
                )
                .children(action),
        )
}

/// A badge of a word or two on a ground tinted `color`, as Lite's: a pull request's state, a
/// review's verdict.
pub(super) fn tag(color: Hsla) -> Div {
    h_flex()
        .flex_none()
        .h(dp(20.))
        .px(dp(7.))
        .gap(dp(4.))
        .rounded(dp(4.))
        .bg(color.opacity(0.14))
        .text_color(color)
        .text_size(dp(12.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
}

/// Its state as a tinted badge, and its number, which opens it on the forge.
fn status_section(review: &Review, state: ReviewState, forge: &str, theme: &Theme) -> Div {
    let (glyph, color) = ui_review::state_glyph(state, theme);
    let badge = tag(color)
        .child(small_icon(glyph).size(dp(12.)).text_color(color))
        .child(ui_review::state_label(state));
    let link = review.url.clone().map(|url| {
        h_flex()
            .id("open-review")
            .role(Role::Link)
            .aria_label(format!("Open on {forge}"))
            .gap(dp(2.))
            .text_size(dp(12.))
            .text_color(theme.text_2)
            .hover(|el| el.text_color(theme.text_1))
            .child(div().underline().child(format!("#{}", review.number)))
            .child(
                small_icon(Icon::ArrowUpRight)
                    .size(dp(12.))
                    .text_color(theme.text_3),
            )
            .tooltip(move |window, cx| {
                let key = shortcut(&crate::app::OpenPullRequest, cx);
                Tooltip::keyed("Open the pull request", key)(window, cx)
            })
            .on_click(move |_, _, cx| cx.open_url(&url))
    });
    let actions = h_flex().gap(dp(12.)).child(badge).children(link);
    side_section("Status", Some(actions.into_any_element()), theme)
}

/// Who's been asked to review it, and on GitHub, which lists only those who haven't
/// answered, that they haven't.
fn reviewers_section(review: &Review, github: bool, theme: &Theme) -> Option<Div> {
    if review.reviewers.is_empty() {
        return None;
    }
    let rows = review.reviewers.iter().enumerate().map(|(ix, user)| {
        h_flex()
            .gap(dp(8.))
            .text_size(dp(13.))
            .child(forge_avatar(
                &user.login,
                user.avatar_url.as_deref(),
                18.,
                theme,
            ))
            .child(
                div()
                    .flex_grow(1.)
                    .min_w_0()
                    .truncate()
                    .child(user.login.clone()),
            )
            .when(github, |el| {
                el.child(
                    div()
                        .id(("awaiting", ix))
                        .child(icon(Icon::Clock).text_color(theme.text_3))
                        .tooltip(Tooltip::text("Awaiting review")),
                )
            })
    });
    Some(side_section("Reviewers", None, theme).children(rows))
}

/// Its labels as Lite's tags: an outlined pill with the label's colour as a dot.
fn labels_section(review: &Review, theme: &Theme) -> Option<Div> {
    if review.labels.is_empty() {
        return None;
    }
    let tags = review.labels.iter().map(|label| {
        h_flex()
            .h(dp(20.))
            .max_w_full()
            .px(dp(7.))
            .gap(dp(5.))
            .rounded_full()
            .border_1()
            .border_color(theme.border_2)
            .text_size(dp(12.))
            .children(
                label
                    .color
                    .map(|color| dot(gpui::rgb(color).into()).size(dp(8.))),
            )
            .child(div().min_w_0().truncate().child(label.name.clone()))
    });
    Some(side_section("Labels", None, theme).child(h_flex().flex_wrap().gap(dp(4.)).children(tags)))
}

/// A check, as Lite's: a dot for how it stands, its name over how it finished and how long
/// it took; a click opens its log.
fn check_row(ix: usize, run: &CheckRun, theme: &Theme) -> Stateful<Div> {
    let color = match run.failed {
        Some(true) => theme.danger,
        None => theme.warn,
        Some(false) if run.status == "Skipped" => theme.text_3,
        Some(false) => theme.safe,
    };
    let status = match (run.started_at, run.completed_at) {
        (Some(start), Some(end)) => format!("{} · {}", run.status, duration((end - start).max(0))),
        // How long it's run so far.
        (Some(start), None) => format!("{} · {}", run.status, relative_time(start)),
        (None, _) => run.status.clone(),
    };
    let url = run.url.clone();
    let group = SharedString::from("check-row");
    h_flex()
        .id(("check", ix))
        .group(group.clone())
        .mx(dp(-6.))
        .px(dp(6.))
        .py(dp(4.))
        .gap(dp(8.))
        .rounded(dp(4.))
        .text_size(dp(12.))
        .hover(|el| el.bg(theme.hover()))
        .child(dot(color).size(dp(8.)))
        .child(
            v_flex()
                .flex_grow(1.)
                .min_w_0()
                .child(div().truncate().child(run.name.clone()))
                .child(div().text_color(theme.text_3).child(status)),
        )
        .when(!url.is_empty(), |el| {
            el.child(
                small_icon(Icon::ArrowUpRight)
                    .text_color(theme.text_3)
                    .opacity(0.)
                    .group_hover(group, |el| el.opacity(1.)),
            )
            // The arrow says where a click goes, as Lite's rows do, so no tooltip covers the
            // next row.
            .role(Role::Link)
            .aria_label(format!("Open the log of {}", run.name))
            .on_click(move |_, _, cx| cx.open_url(&url))
        })
}

/// A tab of a page's header, as ui-react's and but.dev's: its glyph, its label faded unless
/// it's selected or under the pointer, and under the selected one a bar along the header's edge.
fn tab(
    id: &'static str,
    glyph: Icon,
    label: impl Into<SharedString>,
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
                .child(label.into()),
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

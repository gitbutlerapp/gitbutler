//! A branch's pull request, on a tab of the branch's page beside its changes: what it is,
//! where it stands, its description and its checks, as the forge cache last saw them.
//! The store's background refresh asks the forge, and the page says how old what it shows
//! is. Only whether an open one can merge is asked as the tab shows it, as the cache doesn't
//! keep that.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Context, Div, Hsla, MouseButton, Role,
    ScrollHandle, SharedString, Stateful, Toggled, Window, div, list, prelude::*, relative,
};

use super::diff_view::pressed;
use super::panes::{editable_title, measured, reword_editor};
use super::{Detail, PageFold};
use crate::backend::reviews::MergeMethod;
use crate::backend::{CheckRun, CheckState, Forge, FullName, Remark, Review, ReviewState};
use crate::overlays::{MenuItem, has_keys};
use crate::pr::{Mergeability, Offer};
use crate::state::{ProjectId, Selection, snack};
use crate::theme::Theme;
use crate::ui::fold;
use crate::ui::markdown;
use crate::ui::person::forge_avatar;
use crate::ui::review as ui_review;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    ButtonStyle, Icon, TextPoint, Tooltip, ago, button, count_badge, dot, dp, dropdown_button,
    duration, h_flex, icon, icon_button, keep_focus_on_press, local_time, now, relative_time,
    shortcut, small_icon, switch_grounds, switch_spring, tab_stop, v_flex,
};

/// How tall the tabs over the diff beside a list are.
pub(super) const TABS_HEIGHT: f32 = 40.;

/// How old, in seconds, what the pull request tab shows can get before a failed refresh
/// warns about it.
const STALE_AFTER: i64 = 30 * 60;

impl Detail {
    /// The selected branch's pull request, if it has one: the branch page's, the Branches
    /// page's, or the one the target commit selected, on the project page or in the
    /// navigator, merged.
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

    /// Show the pull request's tab, or the changes'. The tabs keep the keys when they have
    /// them; from anywhere else the keys go to what the tab shows.
    pub(crate) fn set_review_tab(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.review_tab = on;
        match self.tabs_focus.is_focused(window) {
            true => cx.notify(),
            false => self.focus_files(window, cx),
        }
        self.ask_forge(cx);
    }

    /// Whether the tabs have the keys.
    pub(crate) fn tabs_focused(&self, window: &Window) -> bool {
        self.tabs_focus.is_focused(window)
    }

    /// Give the keys to the tabs. Whether the page keeps them for coming back from the left
    /// is up to how the keys next leave.
    pub(crate) fn focus_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs_kept = false;
        window.focus(&self.tabs_focus, cx);
        cx.notify();
    }

    /// Whether coming back from the left lands on the tabs, as the keys last went left out of
    /// them on this page.
    pub(crate) fn tabs_kept(&self, cx: &App) -> bool {
        self.tabs_kept && self.selected_review(cx).is_some()
    }

    /// Keep the tabs for coming back from the left, or forget them.
    pub(crate) fn keep_tabs(&mut self, kept: bool) {
        self.tabs_kept = kept;
    }

    /// ← or → on the tabs: the tab on that side, if there is one. Says whether there was.
    pub(crate) fn step_tab(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // The pull request's tab is the right one.
        if self.review_tab == forward {
            return false;
        }
        self.set_review_tab(forward, window, cx);
        true
    }

    /// ↑ on the tabs stays there, and from the top of the files under them goes up to them,
    /// as it does from the top of the diff. Says whether it did either.
    pub(super) fn up_to_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.tabs_focus.is_focused(window) {
            return true;
        }
        let top = (self.listed_files(cx).first()).is_none_or(|&file| file == self.current_file());
        let up = self.files_focus.is_focused(window) && top && self.selected_review(cx).is_some();
        if up {
            self.focus_tabs(window, cx);
        }
        up
    }

    /// ↓ from the tabs: down to the files under Changes, the file list or the files beside a
    /// list page's list, or the diff while they're hidden. The pull request's tab has nothing
    /// under it to take the keys. Says whether the tabs had them.
    pub(super) fn down_from_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.tabs_focus.is_focused(window) {
            return false;
        }
        match (self.review_tab, self.list_page(cx)) {
            (true, _) => {}
            (false, true) if self.beside_files_shown(cx) => self.focus_beside_files(window, cx),
            (false, true) => self.focus(window, cx),
            (false, false) => self.focus_files(window, cx),
        }
        true
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

    /// What the pointer does to the text of the description or comment drawn as `id`, and
    /// what of it is selected. A press selects as on the diff, and the drag goes on until the
    /// button comes up anywhere on the tab.
    pub(super) fn note_select(&self, id: &str, cx: &Context<Self>) -> markdown::Select {
        let held = self.selecting && (self.note_selection.as_ref()).is_some_and(|(of, _)| of == id);
        let (down, drag) = (cx.entity().downgrade(), cx.entity().downgrade());
        let (down_id, drag_id) = (id.to_owned(), id.to_owned());
        markdown::Select {
            range: (self.note_range())
                .filter(|(of, _)| *of == id)
                .map(|(_, range)| range),
            held,
            down: Rc::new(move |at, text, event, _, cx| {
                let (clicks, extend) = (event.click_count, event.modifiers.shift);
                (down.update(cx, |this, cx| {
                    let old = (this.note_selection.take())
                        .filter(|(of, _)| *of == down_id)
                        .map(|(_, range)| range);
                    let range = pressed(old, at, text, clicks, extend);
                    this.note_selection = Some((down_id.clone(), range));
                    this.selecting = true;
                    cx.notify();
                }))
                .ok();
            }),
            drag: Rc::new(move |at, _, cx| {
                (drag.update(cx, |this, cx| {
                    if let (true, Some((of, (_, head)))) =
                        (this.selecting, &mut this.note_selection)
                        && *of == drag_id
                        && *head != at
                    {
                        *head = at;
                        cx.notify();
                    }
                }))
                .ok();
            }),
        }
    }

    /// The text selected in the description or a comment: which, by its id, and from where to
    /// where, in order. None while it's empty.
    pub(super) fn note_range(&self) -> Option<(&str, (TextPoint, TextPoint))> {
        let (id, (anchor, head)) = self.note_selection.as_ref()?;
        (anchor != head).then(|| (id.as_str(), (*anchor.min(head), *anchor.max(head))))
    }

    /// The text selected in the pull request's description or one of its comments.
    pub(super) fn selected_note_text(&self, cx: &App) -> Option<String> {
        let (id, range) = self.note_range()?;
        let review = self.selected_review(cx)?;
        let text = match id == description_id(review) {
            true => markdown::selected_text(&markdown::parse(&review.body), range),
            false => {
                let project = self.selected_project(cx)?;
                let store = self.store.read(cx);
                let Some(Ok(remarks)) = store.conversation(&project, review.number) else {
                    return None;
                };
                let note = (remarks.iter())
                    .flat_map(|remark| match remark {
                        Remark::Comment(note) | Remark::Review { note, .. } => {
                            std::slice::from_ref(note)
                        }
                        Remark::Thread { thread, .. } => &thread.comments[..],
                    })
                    .find(|note| note.url == id)?;
                markdown::selected_text(&note.body, range)
            }
        };
        (!text.is_empty()).then_some(text)
    }

    /// Merging, as Lite's Merge section: auto-merge as a switch and Merge with its method, each
    /// saying in its tooltip why it can't be pressed when GitHub's merge box wouldn't offer it.
    fn merge_section(&self, theme: &Theme, cx: &Context<Self>) -> Option<Div> {
        let (project, branch, review) = self.merge_box_review(cx)?;
        let store = self.store.read(cx);
        let merge = crate::pr::mergeability(store, project, review);
        let title = headline(&merge);
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
    /// with a glyph while its checks fail or run. They take the keys as the panes do.
    pub(super) fn review_tabs(
        &self,
        review: &Review,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let files = (self.shown_diff().map(|diff| diff.files.len())).filter(|n| *n > 0);
        let checks = review.checks.filter(|c| c.failed > 0 || c.running > 0);
        let solid = has_keys(&self.tabs_focus, window, cx);
        h_flex()
            .track_focus(&self.tabs_focus)
            .key_context("Tabs")
            // The first tab's label, not its hover ground, lines up with the text above.
            .ml(dp(-8.))
            .child(
                tab(
                    "tab-changes",
                    Icon::Diff,
                    "Changes",
                    !self.review_tab,
                    solid,
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
                    solid,
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

    /// How wide the pull request tab was last drawn on a page like this one: beside a list
    /// page's list, or on a branch's page. Kept apart, as the list makes them differ, so going
    /// from one to the other lays the tab out for its width from the first frame. Not the
    /// list page's own `beside_width`, which starts at nothing: the first tab drawn there would
    /// take a frame to find room for its panel.
    fn review_width(&self, cx: &App) -> &Rc<Cell<f32>> {
        let [branch_page, list_page] = &self.review_width;
        match self.list_page(cx) {
            true => list_page,
            false => branch_page,
        }
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
        let two_columns = self.review_width(cx).get() >= TWO_COLUMNS;
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
        // list of checks takes the other out of sight. The panel is a pane of its own along
        // the right edge, grey and edged as the navigator is, its cards under the header's
        // actions, so the room the column leaves reads as margin. Narrow, the panel goes
        // between the title and the description, so the page still opens on what the pull
        // request is.
        let page = match two_columns {
            true => h_flex().size_full().child(column.min_w_0()).child(
                scrolling(
                    "review-side",
                    "review-side-scrollbar",
                    div()
                        .px(dp(PANEL_INSET))
                        .child(self.side(project, review, theme, cx)),
                    &self.review_side_scroll,
                    theme,
                )
                .flex_none()
                // Narrower in a narrower pane, so the column keeps its room and the panel stays
                // beside it longer. Flex sizes it, so it's right on the frame the pane changes.
                .w(relative(0.3))
                .min_w(dp(PANEL_INSET + PANEL_MIN_WIDTH + PANEL_INSET))
                .max_w(dp(PANEL_INSET + PANEL_WIDTH + PANEL_INSET))
                .bg(theme.bg_2)
                .border_l_1()
                .border_color(theme.border_3),
            ),
            false => column,
        };
        let width = measured(self.review_width(cx), &self.app, |bounds| {
            bounds.size.width.into()
        });
        // A press on a note's text selects there before this sees it, and one anywhere else on
        // the tab drops the selection. Letting go ends a drag, as on the diff's ground.
        (div().relative().flex_1().min_h_0().w_full())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if !this.selecting && this.note_selection.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.let_go(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.let_go(cx)),
            )
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
        let two_columns = self.review_width(cx).get() >= TWO_COLUMNS;
        // Each row's content and the room above it. The list ignores a row's own margins, so
        // the room and the column's sides go on a row of its own around the content.
        let (above, content) = match ix {
            0 => (16., Some(self.head(&project, review, theme, cx))),
            1 => (
                20.,
                (!two_columns).then(|| self.side(&project, review, theme, cx)),
            ),
            // While it's edited, the editor above holds it.
            2 => (
                if two_columns { 12. } else { 20. },
                (self.shown_review_edit(&project, review, cx).is_none()).then(|| {
                    description(review, self.note_select(&description_id(review), cx), theme)
                }),
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
        // Capped by a wrapper, so the description's wash still reaches past its text.
        .child(div().max_w(dp(COLUMN_MAX_WIDTH)).child(content))
        .into_any_element()
    }

    /// The tab's title, who opened it and when it last changed. A click on the title edits it
    /// and the description in its place, on the forges the app can edit them on.
    fn head(&self, project: &ProjectId, review: &Review, theme: &Theme, cx: &Context<Self>) -> Div {
        let title = match review.title.as_str() {
            "" => format!("Pull request #{}", review.number),
            title => title.to_owned(),
        };
        let forge = (self.store.read(cx).workspace(project)).and_then(|ws| ws.forge);
        let title = match (self.shown_review_edit(project, review, cx), forge) {
            (Some((edit, saving)), _) => reword_editor(&edit.input, saving, theme),
            (None, Some(Forge::GitHub | Forge::GitLab | Forge::Bitbucket)) => editable_title(
                "review-title",
                title,
                "Edit title and description",
                None,
                theme,
            )
            .text_size(dp(18.))
            .line_height(dp(24.))
            .on_click(cx.listener(|this, _, window, cx| this.start_review_edit(window, cx)))
            .into_any_element(),
            (None, Some(Forge::Azure) | None) => div()
                .text_size(dp(18.))
                .line_height(dp(24.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title)
                .into_any_element(),
        };
        v_flex()
            .gap(dp(12.))
            .child(title)
            .children(byline(review, theme))
    }

    /// How fresh what the tab shows is: the button to ask the forge again, its tooltip saying
    /// when it last answered, and once a failed ask has let what shows age, how old it is beside
    /// it. Both sit on one line, so the warning coming and going moves nothing else. None for a
    /// merged or closed one, which is settled.
    fn freshness(
        &self,
        project: &ProjectId,
        review: &Review,
        theme: &Theme,
        cx: &App,
    ) -> Option<Div> {
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
            return None;
        }
        let ask = {
            let (store, project) = (self.store.clone(), project.clone());
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |store, _| store.refresh_forge(&project))
            }
        };
        // A failed refresh is worth a warning only once what shows has aged; until then the
        // button to check again is enough.
        let stale = failed && synced.is_none_or(|at| now() - at > STALE_AFTER);
        let tip = match synced {
            _ if stale => format!("Couldn't update from {forge}. Try again"),
            Some(at) => format!("Checked {}. Ask {forge} again", ago(at)),
            None => format!("Ask {forge}"),
        };
        let button = icon_button("review-check", Icon::Refresh)
            .small()
            .tooltip(tip)
            .disabled_because(checking.then(|| format!("Asking {forge}…")))
            .on_click(ask);
        let age = stale.then(|| {
            div().text_color(theme.warn).child(match synced {
                Some(at) => format!("{} old", relative_time(at)),
                None => "Out of date".to_owned(),
            })
        });
        Some(h_flex().gap(dp(6.)).children(age).child(button))
    }

    /// The panel of where the pull request stands, a card of facts as Lite's: merging, the
    /// state and a link to it, the checks, who's asked to review, labels, branches and when it
    /// was opened.
    fn side(&self, project: &ProjectId, review: &Review, theme: &Theme, cx: &Context<Self>) -> Div {
        let store = self.store.read(cx);
        let forge = store.workspace(project).and_then(|ws| ws.forge);
        let state = store.review_state(project, review);
        let github = forge == Some(Forge::GitHub);
        let forge = forge.map_or("the forge", Forge::name);
        let sections = [
            self.merge_section(theme, cx),
            Some(status_section(review, state, forge, theme)),
            self.checks_section(project, review, theme, cx),
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

    /// The checks, as GitHub's merge box has them: a bar of how they stand and what that comes
    /// to, over every check, failed first, folded under it. A click anywhere on the bar and its
    /// counts folds them. A pull request's open by themselves once any failed, the news, and stay
    /// open, through a rerun too, until the reader folds them. The button to ask the forge again
    /// sits by the heading, which shows on its own while the forge reports no checks.
    fn checks_section(
        &self,
        project: &ProjectId,
        review: &Review,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let runs = &review.check_runs;
        // Overhanging the heading's line, so the rows below stay put as it comes and goes. It
        // sits on what folds the checks, so a click on it, even while it's disabled, stops here.
        let fresh = (self.freshness(project, review, theme, cx)).map(|fresh| {
            div()
                .id("checks-fresh")
                .ml_auto()
                .mr(dp(-6.))
                .my(dp(-2.))
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(fresh)
        });
        let heading = || {
            h_flex()
                .min_h(dp(20.))
                .gap(dp(4.))
                .text_size(dp(12.))
                .text_color(theme.text_2)
        };
        let section = || v_flex().gap(dp(8.)).pl(dp(14.)).pr(dp(12.)).py(dp(12.));
        if runs.is_empty() {
            let fresh = fresh?;
            return Some(
                // As the status row is: the heading, and opposite it what there is.
                section().child(
                    heading().justify_between().child("Checks").child(
                        h_flex()
                            .gap(dp(8.))
                            .child(div().text_color(theme.text_3).child("None"))
                            .child(fresh),
                    ),
                ),
            );
        }
        let count = |states: &[CheckState]| {
            (runs.iter())
                .filter(|run| states.contains(&run.state))
                .count()
        };
        let failed = count(&[CheckState::Failed, CheckState::Cancelled]);
        let running = count(&[CheckState::Running]);
        let queued = count(&[CheckState::Queued]);
        let passed = count(&[CheckState::Passed]);
        let skipped = count(&[CheckState::Skipped]);
        // Skipped ones ran nothing, so they're counted but take no room on the bar.
        let bar = h_flex().h(dp(6.)).gap(dp(2.)).children(
            [
                (failed, theme.danger),
                (running + queued, theme.warn),
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
                // In the bar's order.
                [
                    (failed, format!("{failed} failed"), theme.danger),
                    (running, format!("{running} running"), theme.warn),
                    (queued, format!("{queued} queued"), theme.warn),
                    (
                        passed,
                        match passed == runs.len() {
                            true => format!("All {passed} passed"),
                            false => format!("{passed} passed"),
                        },
                        theme.safe,
                    ),
                    (skipped, format!("{skipped} skipped"), theme.text_3),
                ]
                .into_iter()
                .filter(|(n, ..)| *n > 0)
                .map(|(_, text, color)| div().text_color(color).child(text)),
            );
        let key = (project.clone(), review.number);
        // Opening by itself counts as the reader's choice, so a rerun that takes the failure
        // back to queued doesn't fold the list away while they watch it.
        let open = {
            let mut opened = self.checks_open.borrow_mut();
            match opened.get(&key) {
                Some(&open) => open,
                None if failed > 0 => {
                    opened.insert(key.clone(), true);
                    true
                }
                None => false,
            }
        };
        let tip = match open {
            true => "Fold the checks",
            false => "List the checks",
        };
        let summary = v_flex()
            .id("checks-fold")
            .role(Role::Button)
            .aria_label(tip)
            .gap(dp(8.))
            .child(
                heading()
                    .ml(dp(-4.))
                    .child(fold::chevron("fold", open, theme.text_3))
                    .child("Checks")
                    .children(fresh),
            )
            .child(bar)
            .child(counts)
            .tooltip(Tooltip::text(tip))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.checks_open.borrow_mut().insert(key.clone(), !open);
                this.page_folds.start(PageFold::Checks, !open, cx);
                cx.notify();
            }));
        // The gap above the list folds with it.
        let list = (self.page_folds.shows(&PageFold::Checks, open)).then(|| {
            let rows = (runs.iter().enumerate()).map(|(ix, run)| check_row(ix, run, theme));
            let rows = v_flex().pt(dp(8.)).children(rows);
            div()
                .mt(dp(-8.))
                .child(self.page_folds.reveal(&PageFold::Checks, rows))
        });
        Some(section().child(summary).children(list))
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
/// description: where the column, beside the narrowest panel, would get under about 300.
pub(super) const TWO_COLUMNS: f32 = 640.;

/// How wide the panel beside the description gets, as Lite's.
const PANEL_WIDTH: f32 = 315.;

/// How narrow the panel beside the description gets in a narrow pane: as narrow as the merge
/// row fits on one line.
const PANEL_MIN_WIDTH: f32 = 272.;

/// The room on each side of the panel's cards: the header's, so they line up under its actions.
const PANEL_INSET: f32 = 16.;

/// How wide the column's content gets, so a description's lines stay short enough to read and
/// 120 columns of code still fit.
const COLUMN_MAX_WIDTH: f32 = 840.;

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

/// Where an open pull request stands on merging, in a few words.
fn headline(merge: &Mergeability) -> SharedString {
    match merge {
        Mergeability::AutoMerging => "Auto-merge is on".into(),
        Mergeability::Ready(_) => "Ready to merge".into(),
        Mergeability::Waiting(why) | Mergeability::Blocked(why) => why.clone().into(),
        Mergeability::Unknown(None) => "Checking whether it can merge…".into(),
        Mergeability::Unknown(Some(_)) => "Couldn't ask whether it can merge".into(),
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

/// The id the description of `review` is drawn with.
fn description_id(review: &Review) -> String {
    format!("review-{}", review.number)
}

/// The description, on a wash just wider than its text, as Lite's card; or that there's none.
fn description(review: &Review, select: markdown::Select, theme: &Theme) -> Div {
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
            &description_id(review),
            &body,
            Some(select),
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

/// A check on one line, as GitHub lists them: a cross for one that failed, grey once
/// cancelled, a dot while it runs, a ring while it waits for a runner, a tick, or a dash for
/// one skipped or neutral; its name, keeping the end that tells a matrix's jobs apart; and how
/// long it took or has run, or what became of it when that says more. A click opens its log,
/// as the arrow in place of the time says on hover.
fn check_row(ix: usize, run: &CheckRun, theme: &Theme) -> Stateful<Div> {
    let glyph =
        |glyph, color| (small_icon(glyph).size(dp(14.)).text_color(color)).into_any_element();
    let took =
        (run.started_at.zip(run.completed_at)).map(|(start, end)| duration((end - start).max(0)));
    let mark = match run.state {
        CheckState::Failed => glyph(Icon::Cross, theme.danger),
        CheckState::Cancelled => glyph(Icon::Cross, theme.text_3),
        CheckState::Running => dot(theme.warn).size(dp(8.)).into_any_element(),
        CheckState::Queued => (dot(gpui::transparent_black()).size(dp(8.)).border_1())
            .border_color(theme.warn)
            .into_any_element(),
        CheckState::Passed => glyph(Icon::Tick, theme.safe),
        CheckState::Skipped => glyph(Icon::Minus, theme.text_3),
    };
    let aside = match (run.note, run.state) {
        (Some(note), _) => Some(note.to_owned()),
        (None, CheckState::Running) => run.started_at.map(relative_time),
        (None, _) => took,
    };
    // A matrix job's name ends in what tells it from its siblings, "(macos)", so that end
    // stays in sight while the start gives way.
    let (start, end) = match run.name.rsplit_once(" (") {
        Some((start, end)) if end.ends_with(')') => (start.to_owned(), Some(format!("({end}"))),
        _ => (run.name.clone(), None),
    };
    let name = h_flex()
        .id(("check-name", ix))
        .flex_grow(1.)
        .min_w_0()
        // A space's width, which a space in the text would add to what truncating leaves.
        .gap(dp(3.))
        .child(div().min_w_0().flex_shrink(1000.).truncate().child(start))
        .children(end.map(|end| div().min_w_0().truncate().child(end)))
        .tooltip(Tooltip::text(run.name.clone()));
    let link = !run.url.is_empty();
    let group = SharedString::from("check-row");
    let aside = div()
        .flex_none()
        .relative()
        .min_w(dp(12.))
        .text_color(theme.text_3)
        .child(
            div()
                .when(link, |el| {
                    el.group_hover(group.clone(), |el| el.opacity(0.))
                })
                .children(aside),
        )
        .when(link, |el| {
            el.child(
                h_flex()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .opacity(0.)
                    .group_hover(group.clone(), |el| el.opacity(1.))
                    .child(small_icon(Icon::ArrowUpRight).text_color(theme.text_3)),
            )
        });
    let hover = theme.hover();
    h_flex()
        .id(("check", ix))
        .group(group)
        .mx(dp(-6.))
        .px(dp(6.))
        .py(dp(3.))
        .gap(dp(8.))
        .rounded(dp(4.))
        .text_size(dp(12.))
        .line_height(dp(18.))
        .child(h_flex().flex_none().w(dp(14.)).justify_center().child(mark))
        .child(name)
        .child(aside)
        .when(link, |el| {
            let url = run.url.clone();
            // The arrow says where a click goes, as Lite's rows do.
            el.hover(move |el| el.bg(hover))
                .role(Role::Link)
                .aria_label(format!("Open the log of {}", run.name))
                .on_click(move |_, _, cx| cx.open_url(&url))
        })
}

/// A tab of a page's header, as ui-react's and but.dev's: its glyph, its label faded unless
/// it's selected or under the pointer, and under the selected one a bar along the header's edge,
/// solid while the tabs have the keys, as a list's selected row is.
fn tab(
    id: &'static str,
    glyph: Icon,
    label: impl Into<SharedString>,
    selected: bool,
    solid: bool,
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
                    .bg(match solid {
                        true => theme.fill_gray,
                        false => theme.selected_unfocused(),
                    }),
            )
        })
}

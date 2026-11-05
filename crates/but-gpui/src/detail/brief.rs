//! A branch's Brief tab. A branch is what a brief is anchored to, as a commit's id goes with a
//! rewrite and Briefs also counts a commit a brief merely starts from. The tab shows from what
//! the store knows, and its version is read as it opens.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Div, FontWeight, Hsla, Pixels, SharedString, Task, Window, div, list,
    prelude::*,
};

mod actions;
mod code;
mod head;
mod keys;
mod margin;
mod names;
mod notes;
pub(super) mod state;
mod talk;
pub(super) mod test_facts;

pub(crate) use keys::{
    ChangesBeside, GoBack, NextBlock, NextBrief, PreviousBrief, SendNotes, WriteNote,
};
pub(super) use state::BriefView;

use super::Detail;
use super::page::PageTab;
use super::pull_request::COLUMN_MAX_WIDTH;
use crate::backend::briefs::{
    self, BriefAnchor, BriefBlock, BriefBody, BriefPicture, CommentTo, Pictures,
};
use crate::backend::run;
use crate::state::{ProjectTab, RowKey, Selection, Side};
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{Icon, dp, fixed, fold_row, h_flex, markdown, plural, v_flex};
use state::BriefFold;

/// What a read of a brief is of: its id, the version, and the branch's tip as that was.
pub(super) type ReadKey = (String, Option<u32>, Option<crate::backend::CommitId>);

/// How long a branch stays shown before Briefs is asked, as a held arrow key passes many.
const SETTLE: Duration = Duration::from_millis(100);

/// How long what's known of a branch counts, in seconds, before opening it asks again.
const KNOWN_FOR: i64 = 5 * 60;

/// How long after failing to read a version it's tried again.
const READ_AGAIN: Duration = Duration::from_secs(30);

/// How wide words get, in design pixels, as lines much longer read badly. Code and pictures take
/// the column.
pub(super) const MOST_WORDS_WIDE: f32 = 680.;

/// How far ↓ and ↑ read on through a brief, in design pixels.
const STEP: f32 = 80.;

/// A row of the Brief tab's column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Head,
    Block(usize),
    /// One heading's worth of block `at`'s words: its paragraphs `from..to`. A row each, so a long
    /// block's words are drawn as they show, and j and k stop at each.
    Text {
        at: usize,
        from: usize,
        to: usize,
    },
    /// What's said about the whole brief, and that it ends.
    End,
    /// What's different since the version read, over what's different.
    Since,
    /// Blocks not changed since the version last read, left out for whoever read that.
    Unchanged(usize),
    /// The line neighbouring blocks with the same teaser fold away under, from block `first`.
    Fold {
        first: usize,
        count: usize,
    },
}

impl Row {
    /// Whether it draws anything new since the version last read.
    fn is_fresh(self, blocks: &[BriefBlock]) -> bool {
        match self {
            Row::Text { at, from, to } => (blocks.get(at)).is_some_and(|block| match &block.body {
                BriefBody::Text(words) => (words.get(from..to).unwrap_or_default().iter())
                    .any(|word| word.has_tint(markdown::Tint::Fresh)),
                _ => block.fresh,
            }),
            Row::Block(at) => blocks.get(at).is_some_and(|block| block.fresh),
            Row::Fold { first, count } => {
                (blocks.iter().skip(first).take(count)).any(|block| block.fresh)
            }
            Row::Head | Row::End | Row::Since | Row::Unchanged(_) => false,
        }
    }

    /// The block it draws of, if it does.
    fn block(self) -> Option<usize> {
        match self {
            Row::Block(at) | Row::Text { at, .. } => Some(at),
            Row::Head | Row::End | Row::Since | Row::Unchanged(_) | Row::Fold { .. } => None,
        }
    }
}

/// Block `at`'s rows.
fn block_rows(at: usize, block: &BriefBlock) -> Vec<Row> {
    let BriefBody::Text(paragraphs) = &block.body else {
        return vec![Row::Block(at)];
    };
    let starts: Vec<usize> = (paragraphs.iter().enumerate())
        .filter(|(ix, paragraph)| *ix == 0 || matches!(paragraph.kind, markdown::Kind::Heading(_)))
        .map(|(ix, _)| ix)
        .collect();
    let ends = starts.iter().skip(1).copied().chain([paragraphs.len()]);
    (starts.iter().zip(ends))
        .map(|(from, to)| Row::Text {
            at,
            from: *from,
            to,
        })
        .collect()
}

/// The column's rows: the head, then each block, those with a teaser under it while it's open.
/// With `only_changed`, for who read the version before, what hasn't changed folds away instead.
fn rows(blocks: &[BriefBlock], open: &HashSet<BriefFold>, only_changed: bool) -> Vec<Row> {
    if only_changed {
        let mut rows = vec![Row::Head, Row::Since];
        let mut same = 0;
        for row in (blocks.iter().enumerate()).flat_map(|(at, block)| block_rows(at, block)) {
            if row.is_fresh(blocks) {
                if same > 0 {
                    rows.push(Row::Unchanged(same));
                    same = 0;
                }
                rows.push(row);
            } else {
                same += 1;
            }
        }
        if same > 0 {
            rows.push(Row::Unchanged(same));
        }
        rows.push(Row::End);
        return rows;
    }
    let mut rows = vec![Row::Head];
    let mut first = 0;
    while let Some(block) = blocks.get(first) {
        if block.detail.is_none() {
            rows.extend(block_rows(first, block));
            first += 1;
            continue;
        }
        let same = (blocks.iter().skip(first)).take_while(|next| next.detail == block.detail);
        let count = same.count();
        rows.push(Row::Fold { first, count });
        if open.contains(&BriefFold::Part(first)) {
            rows.extend(
                (blocks.iter().enumerate().skip(first).take(count))
                    .flat_map(|(at, block)| block_rows(at, block)),
            );
        }
        first += count;
    }
    rows.push(Row::End);
    rows
}

impl Detail {
    /// The branch the page shows, while briefs are on and someone's signed in: a workspace
    /// branch by its name and commits, or the one opened on the Branches page by its name and
    /// tip.
    pub(super) fn brief_anchor(&self, cx: &App) -> Option<BriefAnchor> {
        let store = self.store.read(cx);
        if !store.briefs || store.account.user.is_none() {
            return None;
        }
        match self.selection()? {
            Selection::Branch(project, Some(ref_name)) => {
                let branch = store.workspace(&project)?.find_branch(Some(&ref_name))?;
                let commits = branch.commits.iter().map(|commit| commit.id);
                let path = store.path(&project)?;
                Some(BriefAnchor::new(path, &branch.name, commits))
            }
            _ => {
                let project = self.on_tab(ProjectTab::Branches)?;
                let branch = self.selected_other_branch(&project)?;
                let tip = std::iter::once(branch.tip);
                Some(BriefAnchor::new(store.path(&project)?, &branch.name, tip))
            }
        }
    }

    /// Take what the store knows of the branch the page shows. Opening one it knows nothing of
    /// lately, as the poll leaves out older branches, asks about it. As its tab shows, its
    /// brief is read.
    pub(super) fn ask_briefs(&mut self, cx: &mut Context<Self>) {
        let anchor = self.brief_anchor(cx);
        let project = self.selected_project();
        let store = self.store.read(cx);
        let known = (anchor.as_ref().zip(project.as_ref()))
            .and_then(|(anchor, project)| store.brief_at(project, &anchor.branch));
        let stale = known.is_none_or(|known| briefs::now() - known.asked_at >= KNOWN_FOR);
        let known_at_all = known.is_some();
        let found = known.and_then(|known| known.found.clone());
        let brief_first = self.brief.first;
        // Once what's known of the branch is in: a page opened on one the poll leaves out stays
        // open to its brief for the moment it takes to ask.
        let settled_now = known_at_all && (self.page.brief()).is_some_and(|brief| !brief.settled);
        if let Some(brief) = self.page.brief_mut() {
            if brief.found != found {
                // Which version shows is settled by the read, once, so a newer one is offered
                // rather than swapped in under whoever's reading. One picked was of the brief that
                // was here.
                let id_of = |found: &Option<briefs::Found>| found.as_ref().map(|it| it.id.clone());
                if id_of(&brief.found) != id_of(&found) {
                    brief.version = None;
                }
                brief.found = found;
            }
            if settled_now {
                brief.settled = true;
            }
        }
        if settled_now
            && brief_first
            && self.has_brief()
            && let Some(review) = self.page.review_mut()
        {
            review.tab = PageTab::Brief;
        }
        if anchor != self.brief.asked {
            self.brief.asked = anchor.clone();
            self.brief._ask = match anchor.zip(project).filter(|_| stale) {
                None => Task::ready(()),
                Some((anchor, project)) => cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(SETTLE).await;
                    let asked = run(move || briefs::refresh_branch(&anchor)).await;
                    this.update(cx, |this, cx| {
                        (this.store).update(cx, |store, cx| store.took_briefs(&project, asked, cx))
                    })
                    .ok();
                }),
            };
        }
        self.read_shown_brief(cx);
        self.link_brief_names();
        self.keep_talk_current(cx);
    }

    /// Join the code the shown brief names to the branch's diff, once for each of either.
    pub(super) fn link_brief_names(&mut self) {
        let Some(diff) = (self.diff.as_ref())
            .filter(|diff| self.loading.as_ref().is_none_or(|key| *key == diff.key))
        else {
            return;
        };
        let Some(brief) = self.page.brief_mut() else {
            return;
        };
        let Some(page) = (brief.shown.as_mut()).and_then(|shown| shown.as_mut().ok()) else {
            return;
        };
        let key = (diff.key.clone(), page.version, page.tip);
        if brief.linked.as_ref() != Some(&key) {
            names::link_names(page, &diff.files);
            brief.tests = Some(test_facts::TestFacts::of(&diff.files));
            brief.unnamed = head::unnamed_files(page, &diff.files);
            brief.drifted = code::drifted_blocks(page, &diff.files);
            brief.linked = Some(key);
            self.brief.list.remeasure();
        }
    }

    /// Read the version the tab shows, unless that's what it has or is reading. A version never
    /// changes, so the branch moving only asks again how it stands, and what's marked as new in
    /// it, the folds and the place stay. One that couldn't be read is tried again in a while.
    fn read_shown_brief(&mut self, cx: &mut Context<Self>) {
        let (Some(found), Some(anchor)) = (
            (self.page.brief())
                .filter(|_| self.page.brief_tab())
                .and_then(|brief| brief.found.clone()),
            self.brief.asked.clone(),
        ) else {
            return;
        };
        let version = self.page.brief().and_then(|brief| brief.version);
        let tip = anchor.tip();
        let key = (found.id.clone(), version, tip);
        let shown = (self.page.brief())
            .and_then(|brief| brief.page())
            .filter(|page| page.id == found.id && version.is_none_or(|it| it == page.version));
        let current = shown.is_some_and(|page| page.tip == tip);
        let reading = (self.brief.reading.as_ref()).is_some_and(|(reading, _)| *reading == key);
        if current || reading {
            return;
        }
        let task = match shown.map(|page| page.written_at.clone()) {
            Some(head) => cx.spawn(async move |this, cx| {
                let standing =
                    run(move || Ok(briefs::standing_now(head.as_deref(), &anchor))).await;
                this.update(cx, |this, cx| {
                    if let (Ok(standing), Some(page)) = (
                        standing,
                        this.page.brief_mut().and_then(|brief| brief.page_mut()),
                    ) && page.id == found.id
                        && version.is_none_or(|it| it == page.version)
                    {
                        page.standing = standing;
                        page.tip = tip;
                        this.brief_list_changed(cx);
                    }
                })
                .ok();
            }),
            None => cx.spawn(async move |this, cx| {
                // As a held arrow key passes many, and what's passed hasn't been read.
                cx.background_executor().timer(SETTLE).await;
                let read = {
                    let found = found.clone();
                    run(move || briefs::read(&found, version, &anchor)).await
                };
                let failed = read.is_err();
                this.update(cx, |this, cx| {
                    this.took_brief_page(&found.id, version, read, cx)
                })
                .ok();
                if failed {
                    cx.background_executor().timer(READ_AGAIN).await;
                    this.update(cx, |this, cx| {
                        this.brief.reading = None;
                        this.ask_briefs(cx);
                    })
                    .ok();
                }
            }),
        };
        self.brief.reading = Some((key, task));
    }

    /// Show `read`, a version of brief `id`, if that's still what the tab is to show. Anything
    /// that was showing was another version, so what was open in it goes.
    fn took_brief_page(
        &mut self,
        id: &str,
        version: Option<u32>,
        read: anyhow::Result<briefs::BriefPage>,
        cx: &mut Context<Self>,
    ) {
        if !(self.page.brief()).is_some_and(|brief| {
            brief.version == version && brief.found.as_ref().is_some_and(|now| now.id == id)
        }) {
            return;
        }
        // What's being written is about the version that's going, should this be another.
        let same = |page: &briefs::BriefPage| {
            (self.brief.input.as_ref()).is_some_and(|input| input.is_about(&page.id, page.version))
        };
        if !read.as_ref().is_ok_and(same) {
            self.keep_brief_writing();
        }
        let Some(brief) = self.page.brief_mut() else {
            return;
        };
        if let Ok(page) = &read {
            brief.version = Some(page.version);
            let (id, version) = (page.id.clone(), page.version);
            cx.spawn(async move |_, _| {
                if let Err(err) = run(move || briefs::saw(&id, version)).await {
                    tracing::debug!("couldn't note a brief as read: {err:#}");
                }
            })
            .detach();
        }
        brief.open.clear();
        brief.back = None;
        brief.cursor = None;
        brief.unheard = false;
        brief.linked = None;
        // Of the diff and the version that's going, until they're linked again.
        brief.tests = None;
        brief.unnamed = Default::default();
        brief.drifted = Default::default();
        brief.shown = Some(read.map_err(|err| format!("{err:#}")));
        // Places in words that may be others now, and the rows they're in with them.
        self.note_selection = None;
        self.brief.hovered = None;
        self.brief.said_hovered = None;
        self.link_brief_names();
        self.brief.list.reset(self.brief_rows().len());
        self.keep_talk_current(cx);
        self.run_find(false, cx);
        cx.notify();
    }

    /// ↓ or ↑ on the tab: on through the brief, or from its top up to the tabs.
    pub(super) fn step_brief(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let top = self.brief.list.logical_scroll_top();
        if !forward && top.item_ix == 0 && top.offset_in_item == Pixels::ZERO {
            return self.leave(Side::Top, window, cx);
        }
        let step = Theme::get(cx).scaled(if forward { STEP } else { -STEP });
        self.brief.list.scroll_by(step);
        cx.notify();
    }

    /// ⌘K's Show brief: the brief, or on the Branches page, the branches that have one.
    pub(crate) fn show_brief(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_brief() {
            return self.set_page_tab(PageTab::Brief, window, cx);
        }
        if self.on_tab(ProjectTab::Branches).is_some() {
            return self.filter_with_brief(self.selected_project(), cx);
        }
        (self.store).update(cx, |store, cx| {
            store.snack(
                "No brief here. The Branches page can list those with one",
                cx,
            )
        })
    }

    /// The text selected in the brief.
    pub(super) fn selected_brief_text(&self) -> Option<String> {
        let (id, range) = self.note_range()?;
        let page = self.page.brief()?.shown.as_ref()?.as_ref().ok()?;
        let text = (page.blocks.iter().enumerate()).find_map(|(at, block)| match &block.body {
            BriefBody::Text(paragraphs) => block_rows(at, block).into_iter().find_map(|row| {
                let Row::Text { from, to, .. } = row else {
                    return None;
                };
                (block_id(&page.id, at, from) == id)
                    .then(|| markdown::selected_text(paragraphs.get(from..to)?, range).into())
                    .flatten()
            }),
            _ => None,
        })?;
        let text: String = text;
        (!text.is_empty()).then_some(text)
    }

    /// Whether the page has a brief to show.
    pub(crate) fn has_brief(&self) -> bool {
        // A tile of a row's changes is there beside its brief, not to show that again.
        !matches!(self.pinned, Some((_, RowKey::Changes(_))))
            && self.page.brief().is_some_and(|brief| brief.found.is_some())
            // A commit of the branch picked, its diff says nothing of the branch as a whole.
            && !self.branch_commit_picked()
    }

    /// Whether the tab shows a brief, in place of the changes.
    pub fn showing_brief(&self) -> bool {
        self.page.brief_tab() && self.has_brief()
    }

    pub(super) fn render_brief_tab(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(state) = self.page.brief() else {
            return div().into_any_element();
        };
        let rows = self.brief_rows();
        if self.brief.list.item_count() != rows.len() {
            self.brief.list.reset(rows.len());
        }
        // Where it was left is among rows that are there once it's read.
        if let Some(Ok(_)) = state.shown
            && let Some(top) = state.top.take()
        {
            self.brief.list.scroll_to(top);
        }
        self.measure_brief_room(window);
        let column = list(
            self.brief.list.clone(),
            cx.processor(move |this, ix, _, cx| this.brief_row(&rows, ix, cx)),
        )
        .size_full();
        let column = scrolled(
            column,
            "brief-scrollbar",
            Scroll::List(self.brief.list.clone()),
            theme,
        );
        (self.text_tab_ground(cx))
            // Clicking in it gives it the keys, for its own to work from there on.
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if this.brief.input.is_none() {
                        window.focus(&this.focus, cx);
                    }
                }),
            )
            .child(
                v_flex().size_full().child(
                    div()
                        .id("brief-column")
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                            if !*hovering && this.brief.hovered.take().is_some() {
                                cx.notify();
                            }
                        }))
                        .child(column.h_full())
                        .children(self.render_find(window, theme, cx)),
                ),
            )
            .into_any_element()
    }

    /// The tab's rows as it's drawn now.
    fn brief_rows(&self) -> Vec<Row> {
        let state = self.page.brief();
        match state.and_then(|state| Some((state, state.page()?))) {
            Some((state, page)) => {
                let only_changed = !state.is_open(BriefFold::Unchanged)
                    && page
                        .since_read
                        .as_ref()
                        .is_some_and(|since| since.changed > 0);
                let mut rows = rows(&page.blocks, &state.open, only_changed);
                // With the rest open too, what changed still heads it, and the way back's there.
                if !only_changed
                    && (page.since_read.as_ref())
                        .is_some_and(|it| it.changed > 0 || !it.removed.is_empty())
                {
                    rows.insert(1, Row::Since);
                }
                rows
            }
            None => vec![Row::Head],
        }
    }

    fn brief_row(&self, rows: &[Row], ix: usize, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let (Some(state), Some(&row)) = (self.page.brief(), rows.get(ix)) else {
            return div().into_any_element();
        };
        let Some(found) = &state.found else {
            return div().into_any_element();
        };
        let page = state.page();
        let cursor = state.cursor == Some(ix) && state.cursor_shown;
        // With only what changed showing, that all of it did goes without saying.
        let only_changed = !state.is_open(BriefFold::Unchanged)
            && page.is_some_and(|page| page.since_read.is_some());
        let fresh = !only_changed && page.is_some_and(|page| row.is_fresh(&page.blocks));
        let (above, content) = match row {
            Row::Head => {
                let head = self.brief_head(found, state, cx).into_any_element();
                (
                    16.,
                    self.with_talk(
                        head,
                        margin::Beside {
                            key: 0,
                            wide: MOST_WORDS_WIDE,
                            spot: margin::Spot::Brief,
                            to: CommentTo::Brief,
                            held: false,
                            floats: false,
                        },
                        state,
                        cx,
                    ),
                )
            }
            Row::End => (24., self.brief_end(state, cx).into_any_element()),
            Row::Since => (
                14.,
                page.and_then(|page| self.brief_since_row(page, state, cx))
                    .map_or_else(|| div().into_any_element(), IntoElement::into_any_element),
            ),
            Row::Unchanged(count) => {
                let row = fold_row(("brief-unchanged", ix), Icon::ChevronRight, 0., 18., theme)
                    .role(gpui::Role::Button)
                    .aria_label(SharedString::from("Show what hasn't changed"))
                    .child(format!("{} unchanged", plural(count, "part")))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_brief_fold(BriefFold::Unchanged, cx)
                    }));
                (10., row.into_any_element())
            }
            Row::Fold { first, count } => {
                let block = page.and_then(|page| page.blocks.get(first));
                let teaser = block.and_then(|block| block.detail.clone());
                let glyph = match state.is_open(BriefFold::Part(first)) {
                    true => Icon::ChevronDown,
                    false => Icon::ChevronRight,
                };
                let row = fold_row(("brief-fold", first), glyph, 0., 18., theme)
                    .role(gpui::Role::Button)
                    .aria_label(SharedString::from(teaser.clone().unwrap_or_default()))
                    .child(teaser.unwrap_or_default())
                    .when(count > 1, |el| {
                        el.child(
                            div()
                                .ml(dp(6.))
                                .text_color(theme.text_3)
                                .child(format!("{count} parts")),
                        )
                    })
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_brief_part(ix, first, cx)),
                    );
                (14., row.into_any_element())
            }
            Row::Text { at, from, to } => {
                let next = rows.get(ix + 1).copied();
                let Some(words) = self.brief_words(ix, at, from, to, next, cx) else {
                    return div().into_any_element();
                };
                let after_code = (ix.checked_sub(1).and_then(|before| rows.get(before)))
                    .and_then(|before| before.block())
                    .and_then(|before| page?.blocks.get(before))
                    .is_some_and(|before| matches!(before.body, BriefBody::Code(_)));
                // A sentence is what's wrong when the code it leads into has moved on, so it says
                // so where it's read rather than only over the code.
                let moved_on =
                    (rows.get(ix + 1).and_then(|next| next.block())).is_some_and(|next| {
                        let code = page.and_then(|page| match &page.blocks.get(next)?.body {
                            BriefBody::Code(code) => Some((page, code)),
                            _ => None,
                        });
                        state.drifted.contains(&next)
                            && code
                                .is_some_and(|(page, code)| code::changed_since(page, &code.path))
                    });
                let words = v_flex().child(words).children(moved_on.then(|| {
                    div()
                        .mt(dp(4.))
                        .text_size(dp(12.))
                        .text_color(theme.warn)
                        .child("The code under this changed after it was written")
                }));
                (if after_code { 22. } else { 12. }, words.into_any_element())
            }
            Row::Block(at) => {
                let Some((page, block)) = page.and_then(|page| Some((page, page.blocks.get(at)?)))
                else {
                    return div().into_any_element();
                };
                let id = block_id(&page.id, at, 0);
                let body = match &block.body {
                    // Words have rows of their own.
                    BriefBody::Text(_) => div().into_any_element(),
                    BriefBody::Code(code) => self.brief_code(at, code, page, cx),
                    BriefBody::Files { files, omitted } => self.brief_files(at, files, omitted, cx),
                    BriefBody::Pictures(row) => {
                        render_pictures(&id, row, page.gallery.clone(), theme)
                    }
                    BriefBody::OnTheWeb(what) => div()
                        .text_size(dp(12.))
                        .text_color(theme.text_3)
                        .child(format!("{what}, which shows on the web"))
                        .into_any_element(),
                };
                (
                    12.,
                    self.with_talk(
                        body,
                        margin::Beside {
                            key: ix * 1000,
                            wide: COLUMN_MAX_WIDTH,
                            spot: margin::Spot::Block(block),
                            to: CommentTo::Block(*block.positions.start()),
                            held: false,
                            floats: false,
                        },
                        state,
                        cx,
                    ),
                )
            }
        };
        // Code and pictures take all there is, and all else one measure.
        let wide = match row {
            Row::Block(_) => COLUMN_MAX_WIDTH,
            _ => MOST_WORDS_WIDE,
        };
        // What has a margin beside it sets its own width, as the row takes in the margin too.
        let has_margin = matches!(row, Row::Head | Row::Block(_) | Row::Text { .. });
        let beside = has_margin && self.margin_beside(wide);
        (div().id(("brief-row", ix)).px(dp(24.)).pt(dp(above)))
            // A note without a passage goes on what was last clicked in.
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if let Some(brief) = this.page.brief_mut() {
                        brief.cursor = Some(ix);
                        brief.cursor_shown = false;
                        cx.notify();
                    }
                }),
            )
            .when(ix + 1 == rows.len(), |el| el.pb(dp(24.)))
            .child(
                div()
                    .relative()
                    .when(!beside, |el| el.max_w(dp(wide)))
                    .when(cursor || fresh, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(dp(-10.))
                                .map(|el| match cursor {
                                    true => el.w(dp(2.)),
                                    false => el.w(fixed(1.)),
                                })
                                .bg(theme.pop),
                        )
                    })
                    .child(content),
            )
            .into_any_element()
    }

    /// Open or shut the fold at row `ix`, moving the list as little as it can.
    pub(super) fn toggle_brief_part(&mut self, ix: usize, first: usize, cx: &mut Context<Self>) {
        let before = self.brief_rows().len();
        if let Some(brief) = self.page.brief_mut() {
            brief.toggle(BriefFold::Part(first));
        }
        let after = self.brief_rows().len();
        let under = ix + 1;
        self.brief.list.splice(
            under..under + before.saturating_sub(after),
            after.saturating_sub(before),
        );
        cx.notify();
    }

    /// One heading's worth of block `at`'s words, ready for row `ix`.
    fn brief_words(
        &self,
        ix: usize,
        at: usize,
        from: usize,
        to: usize,
        next: Option<Row>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.page.brief()?;
        let page = state.page()?;
        let block = page.blocks.get(at)?;
        let BriefBody::Text(paragraphs) = &block.body else {
            return None;
        };
        let id = block_id(&page.id, at, from);
        let select = self.note_select(&id, cx);
        let words = paragraphs.get(from..to).unwrap_or_default();
        let marked = self.found_in_words(at, from, words);
        let marked = self.said_in_words(block, state, marked);
        let selected = (self.note_range())
            .filter(|(of, _)| *of == id)
            .map(|(_, range)| range.0.0);
        let position = *block.positions.start();
        let beside = |drawn: std::ops::Range<usize>, el: AnyElement| {
            let held = selected.is_some_and(|selected| drawn.contains(&selected));
            // Worked out now, as pressing anything lets go of the selection before its click.
            let about = match words.get(drawn.start) {
                _ if held => self.brief_note_target(),
                Some(first) if !first.text.trim().is_empty() => notes::passage(block, &first.text),
                _ => CommentTo::Block(position),
            };
            self.with_talk(
                el,
                margin::Beside {
                    key: ix * 1000 + drawn.start,
                    wide: MOST_WORDS_WIDE,
                    spot: margin::Spot::Words(block, from + drawn.start..from + drawn.end),
                    to: about,
                    held,
                    floats: self.margin_floats(
                        from + drawn.start..from + drawn.end,
                        Row::Text { at, from, to },
                        next,
                        state,
                    ),
                },
                state,
                cx,
            )
        };
        Some(markdown::render_beside(
            &id,
            &marked,
            Some(select),
            Some(&beside),
            Theme::get(cx),
        ))
    }
}

/// What the paragraphs from `from` of block `at` of brief `brief` draw as, which selecting goes
/// by.
fn block_id(brief: &str, at: usize, from: usize) -> String {
    format!("brief-{brief}-{at}-{from}")
}

/// The block and paragraph a [`block_id`] of brief `brief` is of.
fn block_place(id: &str, brief: &str) -> Option<(usize, usize)> {
    let (at, from) = id
        .strip_prefix(&format!("brief-{brief}-"))?
        .split_once('-')?;
    Some((at.parse().ok()?, from.parse().ok()?))
}

/// A small line of words around a block.
fn small_line(words: &str, color: Hsla) -> Div {
    (div().text_size(dp(12.)).text_color(color)).child(words.to_owned())
}

/// Pictures to see together, across at one height, with the words that say which is which over
/// them. Each takes its share by how wide it is for how tall, which gives them all one height.
fn render_pictures(
    id: &str,
    row: &Pictures,
    gallery: Arc<[markdown::Image]>,
    theme: &Theme,
) -> AnyElement {
    let cell = |picture: &BriefPicture| {
        v_flex()
            .gap(dp(6.))
            .min_w_0()
            .children(
                (picture.title.as_deref())
                    .map(|title| small_line(title, theme.text_1).font_weight(FontWeight::SEMIBOLD)),
            )
            .child(markdown::figure(
                format!("{id}-{}", picture.at),
                gallery.clone(),
                picture.at,
            ))
            .when(!picture.caption.is_empty(), |el| {
                el.child(small_line(&picture.caption, theme.text_3))
            })
    };
    let holder = div().w_full().max_w(dp(row.widest()));
    match row.stacked {
        true => holder
            .child(
                v_flex()
                    .gap(dp(12.))
                    .children(row.pictures.iter().map(cell)),
            )
            .into_any_element(),
        false => holder
            .child(
                h_flex().items_start().gap(dp(10.)).children(
                    (row.pictures.iter())
                        .map(|picture| cell(picture).flex_grow(picture.ratio).flex_basis(dp(0.))),
                ),
            )
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(details: &[Option<&str>]) -> Vec<BriefBlock> {
        (details.iter())
            .map(|detail| BriefBlock {
                positions: 1..=1,
                fresh: false,
                detail: detail.map(str::to_owned),
                body: BriefBody::OnTheWeb(String::new()),
            })
            .collect()
    }

    #[test]
    fn neighbouring_blocks_with_the_same_teaser_fold_together() {
        let blocks = blocks(&[None, Some("How"), Some("How"), Some("Why"), None]);
        let shut = HashSet::new();
        assert_eq!(
            rows(&blocks, &shut, false),
            [
                Row::Head,
                Row::Block(0),
                Row::Fold { first: 1, count: 2 },
                Row::Fold { first: 3, count: 1 },
                Row::Block(4),
                Row::End,
            ],
            "a fold is shut at first, each teaser its own"
        );
        assert_eq!(
            rows(&blocks, &HashSet::from([BriefFold::Part(1)]), false),
            [
                Row::Head,
                Row::Block(0),
                Row::Fold { first: 1, count: 2 },
                Row::Block(1),
                Row::Block(2),
                Row::Fold { first: 3, count: 1 },
                Row::Block(4),
                Row::End,
            ],
            "an open fold lists its blocks under it"
        );
    }

    #[test]
    fn words_get_a_row_for_each_heading() {
        let mut blocks = blocks(&[None]);
        blocks[0].body = BriefBody::Text(markdown::parse("Opening\n\n## One\n\nA\n\n## Two\n\nB"));
        assert_eq!(
            rows(&blocks, &HashSet::new(), false),
            [
                Row::Head,
                Row::Text {
                    at: 0,
                    from: 0,
                    to: 1
                },
                Row::Text {
                    at: 0,
                    from: 1,
                    to: 3
                },
                Row::Text {
                    at: 0,
                    from: 3,
                    to: 5
                },
                Row::End,
            ],
        );
    }

    #[test]
    fn who_read_the_last_version_sees_only_what_changed() {
        let mut blocks = blocks(&[None, None, Some("Why"), None]);
        blocks[1].fresh = true;
        assert_eq!(
            rows(&blocks, &HashSet::new(), true),
            [
                Row::Head,
                Row::Since,
                Row::Unchanged(1),
                Row::Block(1),
                Row::Unchanged(2),
                Row::End,
            ],
            "what hasn't changed folds away in runs, whatever else folds it"
        );
    }

    #[test]
    fn who_read_the_last_version_sees_the_sections_that_changed_of_one_block() {
        let mut words = markdown::parse("The opening\n\n## Why\n\nBecause");
        let fresh = markdown::Style {
            tint: Some(markdown::Tint::Fresh),
            ..Default::default()
        };
        words[2].styles.push((0..7, fresh));
        let blocks = [BriefBlock {
            positions: 1..=1,
            fresh: true,
            detail: None,
            body: BriefBody::Text(words),
        }];
        assert_eq!(
            rows(&blocks, &HashSet::new(), true),
            [
                Row::Head,
                Row::Since,
                Row::Unchanged(1),
                Row::Text {
                    at: 0,
                    from: 1,
                    to: 3
                },
                Row::End,
            ],
            "one block can be the whole brief, and its other sections still fold"
        );
    }
}

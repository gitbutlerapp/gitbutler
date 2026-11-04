//! A branch's Brief tab. A branch is what a brief is anchored to, as a commit's id goes with a
//! rewrite and Briefs also counts a commit a brief merely starts from. Briefs is asked as the
//! page shows and as the branch moves, and the page keeps what it last said.

use std::collections::HashSet;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Div, FontWeight, Hsla, Pixels, SharedString, Task, Window, div, list,
    prelude::*,
};

use super::Detail;
use super::page::PageTab;
use super::pull_request::COLUMN_MAX_WIDTH;
use crate::backend::briefs::{self, BriefAnchor, BriefBlock, BriefBody, BriefPage};
use crate::backend::run;
use crate::state::{ProjectTab, Selection, Side};
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::{
    ButtonStyle, Icon, ago, button, diff, dp, fold_row, h_flex, markdown, plural, v_flex,
};

/// How long a branch stays shown before Briefs is asked, as a held arrow key passes many.
const SETTLE: Duration = Duration::from_millis(100);

/// How far ↓ and ↑ read on through a brief, in design pixels.
const STEP: f32 = 80.;

/// A row of the Brief tab's column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Head,
    Block(usize),
    /// The line neighbouring blocks with the same teaser fold away under, from block `first`.
    Fold {
        first: usize,
        count: usize,
    },
}

/// The column's rows: the head, then each block, those with a teaser under it while it's open.
fn rows(blocks: &[BriefBlock], open: &HashSet<usize>) -> Vec<Row> {
    let mut rows = vec![Row::Head];
    let mut first = 0;
    while let Some(block) = blocks.get(first) {
        if block.detail.is_none() {
            rows.push(Row::Block(first));
            first += 1;
            continue;
        }
        let same = (blocks.iter().skip(first)).take_while(|next| next.detail == block.detail);
        let count = same.count();
        rows.push(Row::Fold { first, count });
        if open.contains(&first) {
            rows.extend((first..first + count).map(Row::Block));
        }
        first += count;
    }
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

    /// Ask Briefs about the branch the page shows, if that's changed, for the page to keep. A
    /// failed ask logs and keeps what the page has.
    pub(super) fn ask_briefs(&mut self, cx: &mut Context<Self>) {
        let anchor = self.brief_anchor(cx);
        if anchor.is_none()
            && let Some(brief) = self.page.brief_mut()
        {
            brief.shown = None;
        }
        if anchor == self.brief_asked {
            return;
        }
        self.brief_asked = anchor.clone();
        self._brief_ask = match anchor {
            None => Task::ready(()),
            Some(anchor) => cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SETTLE).await;
                let shown = run(move || briefs::best(&anchor)).await;
                this.update(cx, |this, cx| match shown {
                    Ok(shown) => {
                        if let Some(brief) = this.page.brief_mut() {
                            brief.shown = shown;
                            cx.notify();
                        }
                    }
                    Err(err) => tracing::debug!("couldn't ask Briefs: {err:#}"),
                })
                .ok();
            }),
        };
    }

    /// ↓ or ↑ on the tab: on through the brief, or from its top up to the tabs.
    pub(super) fn step_brief(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let top = self.brief_list.logical_scroll_top();
        if !forward && top.item_ix == 0 && top.offset_in_item == Pixels::ZERO {
            return self.leave(Side::Top, window, cx);
        }
        let step = Theme::get(cx).scaled(if forward { STEP } else { -STEP });
        self.brief_list.scroll_by(step);
        cx.notify();
    }

    /// ⌘K's Show brief: the brief, or where they are, so asking on a page without one teaches
    /// that.
    pub(crate) fn show_brief(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.has_brief() {
            true => self.set_page_tab(PageTab::Brief, window, cx),
            false => (self.store).update(cx, |store, cx| {
                store.snack("No brief here, they show on a branch's page", cx)
            }),
        }
    }

    /// The text selected in the brief.
    pub(super) fn selected_brief_text(&self) -> Option<String> {
        let (id, range) = self.note_range()?;
        let page = self.page.brief()?.shown.as_ref()?;
        let text = (page.blocks.iter().enumerate()).find_map(|(at, block)| match &block.body {
            BriefBody::Text(blocks) if block_id(&page.id, at) == id => {
                Some(markdown::selected_text(blocks, range))
            }
            _ => None,
        })?;
        (!text.is_empty()).then_some(text)
    }

    /// Whether the page has a brief to show.
    pub(super) fn has_brief(&self) -> bool {
        self.page.brief().is_some_and(|brief| brief.shown.is_some())
    }

    /// Whether the tab shows a brief, in place of the changes.
    pub fn showing_brief(&self) -> bool {
        self.page.brief_tab() && self.has_brief()
    }

    pub(super) fn render_brief_tab(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let Some(state) = self.page.brief() else {
            return div().into_any_element();
        };
        let Some(page) = &state.shown else {
            return div().into_any_element();
        };
        let rows = rows(&page.blocks, &state.open);
        if self.brief_list.item_count() != rows.len() {
            self.brief_list.reset(rows.len());
        }
        if let Some(top) = state.top.take() {
            self.brief_list.scroll_to(top);
        }
        let column = list(
            self.brief_list.clone(),
            cx.processor(move |this, ix, _, cx| this.brief_row(&rows, ix, cx)),
        )
        .size_full();
        let column = scrolled(
            column,
            "brief-scrollbar",
            Scroll::List(self.brief_list.clone()),
            theme,
        );
        (self.text_tab_ground(cx))
            .child(column.h_full())
            .into_any_element()
    }

    fn brief_row(&self, rows: &[Row], ix: usize, cx: &Context<Self>) -> AnyElement {
        let theme = Theme::get(cx);
        let (Some(state), Some(&row)) = (self.page.brief(), rows.get(ix)) else {
            return div().into_any_element();
        };
        let Some(page) = &state.shown else {
            return div().into_any_element();
        };
        let (above, content) = match row {
            Row::Head => (16., brief_head(page, theme).into_any_element()),
            Row::Fold { first, count } => {
                let teaser = (page.blocks.get(first)).and_then(|block| block.detail.clone());
                let glyph = match state.open.contains(&first) {
                    true => Icon::ChevronDown,
                    false => Icon::ChevronRight,
                };
                let row = fold_row(("brief-fold", first), glyph, 0., 18., theme)
                    .role(gpui::Role::Button)
                    .aria_label(SharedString::from(teaser.clone().unwrap_or_default()))
                    .child(teaser.unwrap_or_default())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_brief_fold(ix, first, count, cx)
                    }));
                (14., row.into_any_element())
            }
            Row::Block(at) => {
                let Some(block) = page.blocks.get(at) else {
                    return div().into_any_element();
                };
                let id = block_id(&page.id, at);
                let select = self.note_select(&id, cx);
                (12., render_block(&id, &block.body, select, theme))
            }
        };
        (div().px(dp(24.)).pt(dp(above)))
            .when(ix + 1 == rows.len(), |el| el.pb(dp(24.)))
            .child(div().max_w(dp(COLUMN_MAX_WIDTH)).child(content))
            .into_any_element()
    }

    /// Open or shut the fold at row `ix`, moving the list as little as it can.
    fn toggle_brief_fold(&mut self, ix: usize, first: usize, count: usize, cx: &mut Context<Self>) {
        let Some(brief) = self.page.brief_mut() else {
            return;
        };
        let under = ix + 1;
        match brief.open.insert(first) {
            true => self.brief_list.splice(under..under, count),
            false => {
                brief.open.remove(&first);
                self.brief_list.splice(under..under + count, 0);
            }
        }
        cx.notify();
    }
}

/// What the brief is, who wrote it and how it stands, with the way to the web, which often shows
/// more than this does.
fn brief_head(page: &BriefPage, theme: &Theme) -> Div {
    let mut meta: Vec<String> = Vec::new();
    meta.extend(page.published.map(ago));
    meta.push(match page.latest > page.version {
        true => format!("Version {} of {}", page.version, page.latest),
        false => format!("Version {}, the latest", page.version),
    });
    meta.push(page.standing.clone());
    // A brief can explain code at one commit, which compares nothing.
    if let Some(changed) = page.changed.filter(|changed| changed.files > 0) {
        meta.push(format!(
            "Covers {} +{} -{}",
            plural(changed.files as usize, "file"),
            changed.additions,
            changed.deletions
        ));
    }
    let url = page.url.clone();
    v_flex()
        .gap(dp(10.))
        .child(
            h_flex()
                .items_start()
                .flex_wrap()
                .gap(dp(12.))
                .child(
                    // In a narrow tile the button goes under, rather than crushing this.
                    div()
                        .flex_1()
                        .min_w(dp(200.))
                        .text_size(dp(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_1)
                        .child(page.title.clone()),
                )
                .child(
                    button("brief-web", "Open on the web")
                        .icon(Icon::Globe)
                        .style(ButtonStyle::Outline)
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                ),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(dp(8.))
                .text_size(dp(12.))
                .text_color(theme.text_2)
                .child(person_avatar(&page.author, 18., theme))
                .child(page.author.clone())
                .children(meta.into_iter().map(|part| div().min_w_0().child(part))),
        )
}

/// What block `at` of brief `brief` draws its text as, which selecting it goes by.
fn block_id(brief: &str, at: usize) -> String {
    format!("brief-{brief}-{at}")
}

/// A small line of words around a block.
fn small_line(words: &str, color: Hsla) -> Div {
    (div().text_size(dp(12.)).text_color(color)).child(words.to_owned())
}

fn render_block(id: &str, body: &BriefBody, select: markdown::Select, theme: &Theme) -> AnyElement {
    match body {
        BriefBody::Text(blocks) => markdown::render(id, blocks, Some(select), theme),
        BriefBody::Code { path, parts } => v_flex()
            .gap(dp(6.))
            .child(small_line(path, theme.text_2))
            .children(parts.iter().map(|lines| diff::excerpt(lines, theme)))
            .into_any_element(),
        BriefBody::Picture {
            title,
            image,
            caption,
        } => v_flex()
            .gap(dp(6.))
            .children(
                title
                    .as_deref()
                    .map(|title| small_line(title, theme.text_1).font_weight(FontWeight::SEMIBOLD)),
            )
            .child(markdown::render(id, image, None, theme))
            .when(!caption.is_empty(), |el| {
                el.child(small_line(caption, theme.text_3))
            })
            .into_any_element(),
        BriefBody::OnTheWeb(what) => div()
            .text_size(dp(12.))
            .text_color(theme.text_3)
            .child(format!("{what}, which shows on the web"))
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(details: &[Option<&str>]) -> Vec<BriefBlock> {
        (details.iter())
            .map(|detail| BriefBlock {
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
            rows(&blocks, &shut),
            [
                Row::Head,
                Row::Block(0),
                Row::Fold { first: 1, count: 2 },
                Row::Fold { first: 3, count: 1 },
                Row::Block(4),
            ],
            "a fold is shut at first, each teaser its own"
        );
        assert_eq!(
            rows(&blocks, &HashSet::from([1])),
            [
                Row::Head,
                Row::Block(0),
                Row::Fold { first: 1, count: 2 },
                Row::Block(1),
                Row::Block(2),
                Row::Fold { first: 3, count: 1 },
                Row::Block(4),
            ],
            "an open fold lists its blocks under it"
        );
    }
}

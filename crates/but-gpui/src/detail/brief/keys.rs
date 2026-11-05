//! Getting around a Brief tab with the keys, and from it to where else a reader goes next: the part
//! after this one, a link in it, the branch's next brief, and back.

use gpui::{Context, Div, ListOffset, Window, actions, prelude::*};

use super::talk::ROW_MEASURED;
use super::{Row, markdown};
use crate::backend::briefs::BriefBody;
use crate::detail::Detail;
use crate::detail::page::{BriefFold, PageTab};
use crate::state::Selection;

actions!(
    brief,
    [
        NextBlock,
        PreviousBlock,
        OpenBlock,
        WriteNote,
        ChangesBeside,
        SendNotes,
        GoBack,
        NextBrief,
        PreviousBrief,
    ]
);

/// A brief on another applied branch, to go on to.
#[derive(Debug, Clone)]
pub(super) struct BriefOnBranch {
    pub(super) name: String,
    pub(super) ref_name: gix::refs::FullName,
    pub(super) title: String,
}

impl Detail {
    /// Whether the diff shows from following something in a brief, which Esc goes back to.
    pub fn back_to_brief(&self) -> bool {
        !self.showing_brief() && (self.page.brief()).is_some_and(|brief| brief.left_for_code)
    }

    /// Where j and k go from, and a note without a passage goes: the row they last went to, or
    /// the first that shows.
    pub(super) fn brief_cursor(&self) -> usize {
        // Not where it was left, if the list has been scrolled away from that.
        let cursor = (self.page.brief().and_then(|brief| brief.cursor))
            .filter(|cursor| self.brief_list.bounds_for_item(*cursor).is_some());
        cursor.unwrap_or_else(|| {
            // The row being read: the next, when the one at the top is partly gone.
            let top = self.brief_list.logical_scroll_top();
            top.item_ix + usize::from(top.offset_in_item > gpui::Pixels::ZERO)
        })
    }

    /// Go to the next or the last block, with it at the top.
    fn hop_brief(&mut self, by: isize, cx: &mut Context<Self>) {
        let last = self.brief_rows().len().saturating_sub(1);
        let to = self.brief_cursor().saturating_add_signed(by).min(last);
        if let Some(brief) = self.page.brief_mut() {
            brief.cursor = Some(to);
            brief.cursor_shown = true;
        }
        self.brief_list.scroll_to(ListOffset {
            item_ix: to,
            offset_in_item: Default::default(),
        });
        cx.notify();
    }

    /// Make `change`, which rows may come or go with, and say where what's at the top is then.
    pub(super) fn brief_place_after(
        &mut self,
        change: impl FnOnce(&mut crate::detail::page::BriefState),
    ) -> Option<ListOffset> {
        let top = self.brief_list.logical_scroll_top();
        let block = self
            .brief_rows()
            .get(top.item_ix)
            .and_then(|row| row.block());
        change(self.page.brief_mut()?);
        let row = (self.brief_rows().iter()).position(|row| row.block() == block)?;
        Some(ListOffset {
            item_ix: row,
            offset_in_item: top.offset_in_item,
        })
    }

    /// Follow a link to block `part` of the brief, `p8` for its eighth, opening the fold it's
    /// in, with the place left to come back to.
    fn follow_brief_link(&mut self, part: &str, cx: &mut Context<Self>) {
        let position: Option<usize> = part.strip_prefix('p').and_then(|n| n.parse().ok());
        let Some(state) = self.page.brief() else {
            return;
        };
        let Some(page) = state.page() else {
            return;
        };
        let Some(at) = position.and_then(|position| {
            (page.blocks.iter()).position(|block| block.positions.contains(&position))
        }) else {
            return;
        };
        let opens = (page.blocks.get(at).and_then(|block| block.detail.as_ref())).map(|detail| {
            (page.blocks.get(..at).unwrap_or_default().iter().rev())
                .take_while(|before| before.detail.as_ref() == Some(detail))
                .count()
        });
        let opens = opens.map(|back| at - back);
        let back = self.brief_place_after(|brief| {
            brief.open.extend(opens.map(BriefFold::Part));
            // It may be among what's folded away as unchanged.
            brief.open.insert(BriefFold::Unchanged);
        });
        if let Some(brief) = self.page.brief_mut() {
            brief.back = back;
        }
        self.scroll_to_brief_row(move |row| row.block() == Some(at), cx);
    }

    /// Put the first row `which` picks at the top, marked, once the rows there are now are drawn
    /// and measured.
    fn scroll_to_brief_row(
        &mut self,
        which: impl Fn(Row) -> bool + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ROW_MEASURED).await;
            this.update(cx, |this, cx| {
                let row = this.brief_rows().iter().position(|row| which(*row));
                if let (Some(row), Some(brief)) = (row, this.page.brief_mut()) {
                    brief.cursor = Some(row);
                    brief.cursor_shown = true;
                    this.brief_list.scroll_to(ListOffset {
                        item_ix: row,
                        offset_in_item: Default::default(),
                    });
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Bring part `part` of block `at` into view, its paragraph or its line of `parts` in a code
    /// card, opening what folds it away. A section can run longer than the window, so the place
    /// goes by how far down its row the part comes.
    pub(in crate::detail) fn reveal_in_brief(
        &mut self,
        at: usize,
        part: usize,
        parts: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(brief) = self.page.brief_mut() {
            brief.open.insert(BriefFold::Unchanged);
            // What the card shows then isn't what's searched.
            if brief.open.remove(&BriefFold::WholeFile(at)) {
                self.brief_list.remeasure();
            }
        }
        let folded = self.brief_rows().iter().find_map(|row| match *row {
            Row::Fold { first, count } if (first..first + count).contains(&at) => Some(first),
            _ => None,
        });
        if let (Some(first), Some(brief)) = (folded, self.page.brief_mut()) {
            brief.open.insert(BriefFold::Part(first));
        }
        let rows = self.brief_rows();
        let found = rows.iter().enumerate().find_map(|(ix, row)| match *row {
            Row::Text {
                at: there,
                from,
                to,
            } if there == at && (from..to).contains(&part) => {
                Some((ix, (part - from) as f32 / (to - from) as f32))
            }
            Row::Block(there) if there == at => Some((ix, part as f32 / parts.max(1) as f32)),
            _ => None,
        });
        let (Some((row, share)), Some(brief)) = (found, self.page.brief_mut()) else {
            return;
        };
        let top = ListOffset {
            item_ix: row,
            offset_in_item: Default::default(),
        };
        if self.brief_list.item_count() != rows.len() {
            // Rows came, so where the list and the keys were means something else now.
            self.brief_list.reset(rows.len());
            brief.cursor = None;
            brief.top.set(Some(top));
        } else if self.brief_list.bounds_for_item(row).is_none() {
            brief.top.set(Some(top));
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ROW_MEASURED).await;
            this.update(cx, |this, cx| {
                let Some(bounds) = this.brief_list.bounds_for_item(row) else {
                    return;
                };
                let view = this.brief_list.viewport_bounds();
                let there = bounds.size.height * share;
                let margin = view.size.height / 6.;
                let y = bounds.top() + there;
                if y < view.top() + margin || y > view.bottom() - margin {
                    this.brief_list.scroll_to(ListOffset {
                        item_ix: row,
                        offset_in_item: (there - view.size.height / 3.).max(Default::default()),
                    });
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Go back to where a link was followed from, if one was.
    pub(super) fn go_back_in_brief(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page.brief().is_some_and(|brief| brief.left_for_code) {
            return self.set_page_tab(PageTab::Brief, window, cx);
        }
        let Some(top) = self.page.brief_mut().and_then(|brief| brief.back.take()) else {
            return cx.propagate();
        };
        if let Some(brief) = self.page.brief_mut() {
            brief.cursor = None;
        }
        self.brief_list.scroll_to(top);
        cx.notify();
    }

    /// Go on to the applied branch with a brief after this one, or before it, to keep reading: a
    /// morning's worth, one after another, without going back to the list.
    pub(super) fn hop_to_brief(&mut self, forward: bool, cx: &mut Context<Self>) {
        // The key that steps through matches, while there are matches to step through.
        if self.find.is_some() {
            return self.step_find(if forward { 1 } else { -1 }, cx);
        }
        let Some(project) = self.selected_project() else {
            return;
        };
        let Some(next) = self.next_brief(forward, cx) else {
            return (self.store).update(cx, |store, cx| {
                store.snack("No other branch here has a brief", cx)
            });
        };
        (self.store).update(cx, |store, cx| {
            store.select(Some(Selection::Branch(project, Some(next.ref_name))), cx)
        });
    }

    /// The applied branch with a brief after this one, or before it, round to the other end, as
    /// the point is to go through them all.
    pub(super) fn next_brief(&self, forward: bool, cx: &gpui::App) -> Option<BriefOnBranch> {
        let project = self.selected_project()?;
        let at = self.brief_asked.as_ref().map(|asked| &asked.branch);
        let store = self.store.read(cx);
        let with_briefs: Vec<BriefOnBranch> = (store.workspace(&project)?.all_branches())
            .filter_map(|branch| {
                let known = store.brief_at(&project, &branch.name)?;
                Some(BriefOnBranch {
                    title: known.found.as_ref()?.title.clone(),
                    name: branch.name.clone(),
                    ref_name: branch.ref_name.clone()?,
                })
            })
            .collect();
        let here = (with_briefs.iter()).position(|brief| Some(&brief.name) == at);
        let count = with_briefs.len().max(1);
        let next = match (here, forward) {
            (Some(here), true) => with_briefs.get((here + 1) % count),
            (Some(here), false) => with_briefs.get((here + count - 1) % count),
            (None, _) => with_briefs.first(),
        };
        next.filter(|next| Some(&next.name) != at).cloned()
    }

    /// Where the reading is, as Changes says the file: which part of how many, and what it's under.
    pub fn brief_place(&self) -> Option<String> {
        let page = self.page.brief()?.page()?;
        let rows = self.brief_rows();
        let parts: Vec<&Row> = rows.iter().filter(|row| row.block().is_some()).collect();
        let cursor = self.page.brief()?.cursor?;
        let at = (rows.get(..=cursor).unwrap_or(&rows).iter())
            .filter(|row| row.block().is_some())
            .count()
            .max(1);
        let heading = (rows.get(..=cursor).unwrap_or(&rows).iter().rev()).find_map(|row| {
            let Row::Text { at, from, .. } = row else {
                return None;
            };
            let BriefBody::Text(words) = &page.blocks.get(*at)?.body else {
                return None;
            };
            let first = words.get(*from)?;
            matches!(first.kind, markdown::Kind::Heading(_)).then(|| first.text.clone())
        });
        (parts.len() > 1).then(|| match heading {
            Some(heading) => format!("Part {at} of {} · {heading}", parts.len()),
            None => format!("Part {at} of {}", parts.len()),
        })
    }

    /// Enter on a block: open a fold, or show a code card's file in Changes.
    fn open_brief_block(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.brief_cursor();
        match self.brief_rows().get(at).copied() {
            Some(Row::Fold { first, .. }) => self.toggle_brief_part(at, first, cx),
            Some(Row::Unchanged(_)) => self.toggle_brief_fold(BriefFold::Unchanged, cx),
            Some(Row::Block(block)) => {
                let path = (self.page.brief())
                    .and_then(|brief| brief.shown.as_ref()?.as_ref().ok())
                    .and_then(|page| page.blocks.get(block))
                    .and_then(|block| match &block.body {
                        BriefBody::Code(code) => Some(code.path.clone()),
                        _ => None,
                    });
                if let Some(path) = path {
                    self.brief_open_file(&path, None, window, cx);
                }
            }
            _ => {}
        }
    }

    /// Put the brief's keys on `el`. They go by the block, and what can be said about it.
    pub(in crate::detail) fn brief_actions(
        &self,
        el: gpui::Stateful<Div>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        el.on_action(cx.listener(|this, _: &NextBrief, _, cx| this.hop_to_brief(true, cx)))
            .on_action(cx.listener(|this, _: &PreviousBrief, _, cx| this.hop_to_brief(false, cx)))
            .on_action(
                cx.listener(|this, link: &markdown::FollowAnchor, window, cx| {
                    // So the key back works from here, which a click on words doesn't see to.
                    window.focus(&this.focus, cx);
                    match link.0.strip_prefix("code:") {
                        Some(place) => {
                            let mut parts = place.splitn(3, ':');
                            let hunk = parts.next().and_then(|it| it.parse().ok());
                            let line = parts.next().and_then(|it| it.parse().ok());
                            if let (Some(hunk), Some(line), Some(path)) = (hunk, line, parts.next())
                            {
                                this.brief_go_to_line(path, hunk, line, window, cx);
                            }
                        }
                        None => this.follow_brief_link(&link.0, cx),
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &GoBack, window, cx| this.go_back_in_brief(window, cx)),
            )
            .on_action(cx.listener(|this, _: &NextBlock, _, cx| this.hop_brief(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousBlock, _, cx| this.hop_brief(-1, cx)))
            .on_action(
                cx.listener(|this, _: &OpenBlock, window, cx| this.open_brief_block(window, cx)),
            )
            .on_action(cx.listener(|this, _: &ChangesBeside, window, cx| {
                this.show_changes_beside(window, cx)
            }))
            .on_action(cx.listener(|this, _: &WriteNote, window, cx| {
                let to = this.brief_note_target();
                this.write_brief_note(to, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SendNotes, _, cx| this.send_brief_talk(cx)))
    }
}

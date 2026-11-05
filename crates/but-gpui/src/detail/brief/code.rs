//! Code a brief shows: what git knows about it over its lines, how sure it is to be real, and the
//! way from it to the branch's own diff of that file.

use gpui::{AnyElement, Context, Div, Hsla, SharedString, Window, div, prelude::*};

use std::collections::HashSet;

use crate::backend::briefs::{BriefBody, BriefCode, BriefPage, CodeSource, Standing};
use crate::backend::{FileDiff, LineKind};
use crate::detail::Detail;
use crate::detail::page::{BriefFold, PageTab};
use crate::theme::Theme;
use crate::ui::diff::{DiffRow, split_path};
use crate::ui::file_icons::file_icon;
use crate::ui::{Icon, diff, dp, h_flex, icon, v_flex};

/// How many hunks of a file a card shows, past which Changes is the place for it.
const MOST_WHOLE_HUNKS: usize = 30;

/// A small marked word by a file's name.
fn chip(words: &str, color: Hsla, theme: &Theme) -> Div {
    div()
        .flex_none()
        .px(dp(5.))
        .rounded(dp(4.))
        .border_1()
        .border_color(color.opacity(0.5))
        .text_size(dp(11.))
        .line_height(dp(16.))
        .text_color(color)
        .bg(theme.bg_1)
        .child(words.to_owned())
}

/// The code cards of `page` that show being added what `files` no longer have, by their blocks. What
/// a brief shows is checked against what's there, so it needn't be taken on trust.
pub(super) fn drifted_blocks(page: &BriefPage, files: &[FileDiff]) -> HashSet<usize> {
    (page.blocks.iter().enumerate())
        .filter_map(|(at, block)| match &block.body {
            BriefBody::Code(code) if code.source != CodeSource::Proposal => {
                let file = files.iter().find(|file| file.file.path == code.path)?;
                let there: HashSet<&str> = (file.hunks().iter())
                    .flat_map(|hunk| hunk.lines.iter())
                    .filter(|line| line.kind != LineKind::Removed)
                    .map(|line| line.text.as_ref())
                    .collect();
                (code.parts.iter().flatten())
                    .filter(|line| line.kind == LineKind::Added && !line.text.trim().is_empty())
                    .any(|line| !there.contains(line.text.as_ref()))
                    .then_some(at)
            }
            _ => None,
        })
        .collect()
}

/// The foot of a card, for what it leaves out.
fn not_shown(omitted: &[String], theme: &Theme) -> Option<Div> {
    (!omitted.is_empty()).then(|| {
        div()
            .text_size(dp(11.))
            .text_color(theme.text_3)
            .child(format!("Not shown: {}", omitted.join(", ")))
    })
}

/// Whether `page` knows `path` changed on the branch since the brief was written.
pub(super) fn changed_since(page: &BriefPage, path: &str) -> bool {
    match &page.standing {
        Standing::Since { files, .. } => files.iter().any(|file| file == path),
        _ => false,
    }
}

impl Detail {
    /// Code in a card, under a header that says what it is and goes to the branch's own diff.
    pub(super) fn brief_code(
        &self,
        at: usize,
        code: &BriefCode,
        page: &BriefPage,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let path = code.path.clone();
        let whole = (self.shown_diff())
            .and_then(|loaded| loaded.files.iter().find(|file| file.file.path == code.path))
            .filter(|file| !file.hunks().is_empty());
        let shows_whole =
            (self.page.brief()).is_some_and(|brief| brief.is_open(BriefFold::WholeFile(at)));
        let first = (code.parts.first())
            .and_then(|lines| lines.iter().find(|line| line.kind != LineKind::Context))
            .or_else(|| code.parts.first().and_then(|lines| lines.first()))
            .and_then(|line| line.new);
        let header = self
            .brief_file_row(
                ("brief-code", at),
                &code.path,
                code.status.as_deref(),
                code.stats,
                first,
                cx,
            )
            .children(
                (code.source == CodeSource::Worktree)
                    .then(|| chip("uncommitted when written", theme.text_3, theme)),
            )
            .children(
                (code.source == CodeSource::Proposal)
                    .then(|| chip("proposal, not written yet", theme.warn, theme)),
            )
            .children(changed_since(page, &path).then(|| chip("changed since", theme.warn, theme)))
            .children(
                (self.page.brief())
                    .filter(|brief| brief.drifted.contains(&at))
                    .map(|_| chip("differs from the branch now", theme.warn, theme)),
            );
        v_flex()
            .rounded(dp(6.))
            .border_1()
            .border_color(match code.source {
                CodeSource::Proposal => theme.warn.opacity(0.6),
                _ => theme.border_3,
            })
            .overflow_hidden()
            .child(header.border_b_1().border_color(theme.border_3))
            .children(match whole.as_ref().filter(|_| shows_whole) {
                // All the branch changes in the file, up to where a row would get too long to
                // draw, as what an agent picks to show can be too little.
                Some(file) => (file.hunks().iter().enumerate())
                    .take(MOST_WHOLE_HUNKS)
                    .map(|(ix, hunk)| {
                        div()
                            .when(ix > 0, |el| el.border_t_1().border_color(theme.border_3))
                            .child(diff::excerpt_flat(&hunk.lines, theme))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>(),
                None => (code.parts.iter().enumerate())
                    .map(|(ix, lines)| {
                        div()
                            .when(ix > 0, |el| el.border_t_1().border_color(theme.border_3))
                            .child(diff::excerpt_flat(lines, theme))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>(),
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_x(dp(16.))
                    .px(dp(10.))
                    .py(dp(4.))
                    .border_t_1()
                    .border_color(theme.border_3)
                    .text_size(dp(11.))
                    .children(whole.as_ref().map(|file| {
                        let (added, removed) = file.stats();
                        let words = match shows_whole {
                            true => "Show what the brief picked".to_owned(),
                            false => {
                                format!("Show all this branch changes here, +{added} -{removed}")
                            }
                        };
                        div()
                            .id(("brief-whole-file", at))
                            .text_color(theme.text_2)
                            .hover(|el| el.text_color(theme.text_1).underline())
                            .role(gpui::Role::Button)
                            .aria_label(SharedString::from(words.clone()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_brief_fold(BriefFold::WholeFile(at), cx)
                            }))
                            .child(words)
                    }))
                    .children(not_shown(&code.omitted, theme)),
            )
            .into_any_element()
    }

    /// The files of a comparison, each going to the branch's own diff of it.
    pub(super) fn brief_files(
        &self,
        at: usize,
        files: &[but_briefs::ChangedFile],
        omitted: &[String],
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        v_flex()
            .rounded(dp(6.))
            .border_1()
            .border_color(theme.border_3)
            .overflow_hidden()
            .children(files.iter().enumerate().map(|(ix, file)| {
                self.brief_file_row(
                    SharedString::from(format!("brief-files-{at}-{ix}")),
                    &file.path,
                    file.status.as_deref(),
                    file.stats,
                    None,
                    cx,
                )
                .when(ix > 0, |el| el.border_t_1().border_color(theme.border_3))
            }))
            .children(not_shown(omitted, theme).map(|words| {
                words
                    .px(dp(10.))
                    .py(dp(4.))
                    .border_t_1()
                    .border_color(theme.border_3)
            }))
            .into_any_element()
    }

    /// A file by its mark, name, folder and lines changed, a click from the branch's own diff.
    pub(super) fn brief_file_row(
        &self,
        id: impl Into<gpui::ElementId>,
        path: &str,
        status: Option<&str>,
        stats: Option<but_briefs::LineStats>,
        line: Option<u32>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = Theme::get(cx);
        let (name, dir) = split_path(path);
        let (name, dir) = (name.to_owned(), dir.to_owned());
        let clicked = path.to_owned();
        let deleted = status == Some("deleted");
        // One the branch doesn't change now has no diff to go to.
        let goes = (self.shown_diff())
            .is_some_and(|loaded| loaded.files.iter().any(|file| file.file.path == path));
        h_flex()
            .id(id)
            .group("brief-file")
            .min_h(dp(28.))
            .px(dp(10.))
            .gap(dp(6.))
            .bg(theme.bg_2)
            .text_size(dp(12.))
            .when(goes, |el| {
                el.role(gpui::Role::Button)
                    .aria_label(SharedString::from(format!("Show {path} in Changes")))
                    .hover(|el| el.bg(theme.fill_gray.opacity(0.08)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.brief_open_file(&clicked, line, window, cx);
                    }))
            })
            .when(!goes, |el| el.opacity(0.6))
            .child(file_icon(path, cx).flex_none())
            .child(
                div()
                    .flex_none()
                    .text_color(theme.text_1)
                    .when(deleted, |el| el.line_through())
                    .child(name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .text_color(theme.text_3)
                    .child(dir),
            )
            .children(
                stats.map(|stats| diff::stats(stats.additions, stats.deletions, false, theme)),
            )
            .when(!goes, |el| {
                el.child(
                    div()
                        .flex_none()
                        .text_size(dp(11.))
                        .text_color(theme.text_3)
                        .child("not on the branch now"),
                )
            })
            .when(goes, |el| {
                el.child(
                    div()
                        .flex_none()
                        .opacity(0.)
                        .group_hover("brief-file", |el| el.opacity(1.))
                        .child(
                            icon(Icon::ArrowUpRight)
                                .size(dp(12.))
                                .text_color(theme.text_2),
                        ),
                )
            })
    }

    /// Show the branch's own diff of `path`, at the hunk with `line` of its new side if it has
    /// one, the brief a tab away where it was left. A file the branch doesn't change now says so,
    /// as that's worth knowing about a brief.
    pub(super) fn brief_open_file(
        &mut self,
        path: &str,
        line: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let found = (self.shown_diff()).and_then(|loaded| {
            let (file_ix, file) =
                (loaded.files.iter().enumerate()).find(|(_, file)| file.file.path == path)?;
            let hunk = line.and_then(|line| {
                (file.hunks().iter())
                    .position(|hunk| hunk.lines.iter().any(|at| at.new == Some(line)))
            });
            Some((file_ix, hunk))
        });
        let Some((file_ix, hunk)) = found else {
            return (self.store).update(cx, |store, cx| {
                store.snack("The branch doesn't change that file now", cx)
            });
        };
        self.leave_brief_for_code(window, cx);
        self.scroll_to_file(file_ix, false, cx);
        if let Some(hunk) = hunk {
            self.show_hunk(file_ix, hunk, cx);
        }
        self.focus_files(window, cx);
        cx.notify();
    }

    /// Go to the branch's own diff, with the key back to here on it.
    fn leave_brief_for_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_page_tab(PageTab::Changes, window, cx);
        if let Some(brief) = self.page.brief_mut() {
            brief.left_for_code = true;
        }
    }

    /// Go to line `line` of hunk `hunk` of file `path` in the branch's own diff.
    pub(super) fn brief_go_to_line(
        &mut self,
        path: &str,
        hunk: usize,
        line: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(file_ix) = (self.shown_diff())
            .and_then(|loaded| loaded.files.iter().position(|file| file.file.path == path))
        else {
            return;
        };
        self.leave_brief_for_code(window, cx);
        self.scroll_to_file(file_ix, false, cx);
        self.show_hunk(file_ix, hunk, cx);
        let row = self.row_of(|row| match *row {
            DiffRow::Line {
                file,
                hunk: of,
                line: at,
            } => (file, of, at) == (file_ix, hunk, line),
            DiffRow::Pair {
                file,
                hunk: of,
                left,
                right,
            } => (file, of) == (file_ix, hunk) && (left == Some(line) || right == Some(line)),
            _ => false,
        });
        if let Some(row) = row {
            // With a few lines above it, for what it's in.
            self.scroll_to_row(row.saturating_sub(4), false);
        }
        self.focus_files(window, cx);
        cx.notify();
    }
}

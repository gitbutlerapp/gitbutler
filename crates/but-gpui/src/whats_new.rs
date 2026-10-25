//! What's new: GitButler's release notes in a large modal laid out as Settings, the releases
//! down the left with the running one marked and the picked one's notes beside them, and a
//! search across every release that says which matches the running release has.

use std::collections::HashSet;
use std::ops::Range;

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, ScrollHandle, SharedString, Subscription, Task, Window, actions, div, prelude::*,
};

use crate::app::AppView;
use crate::backend::{self, settings::Published};
use crate::help::OpenReleaseNotes;
use crate::theme::Theme;
use crate::ui::markdown::{self, Block, Kind, Style};
use crate::ui::modal::{MODAL_RADIUS, ModalSize, modal};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    self, ButtonStyle, Icon, button, dp, error_summary, faint, field, fold_glyph, h_flex, muted,
    plural, quiet, quoted, section_label, side_row, small_icon, v_flex,
};

actions!(whats_new, [PreviousRelease, NextRelease]);

const RELEASES: &str = "https://github.com/gitbutlerapp/gitbutler/releases";

/// How many matches a search shows at most. Each is laid out again every frame, and a letter
/// or two matches nearly every note.
const SHOWN: usize = 30;

/// The headings whose sections fold to a count, after the others: what's new comes first.
const FOLDED: [&str; 3] = ["Fixes", "CLI", "TUI"];

/// A release, its notes read into sections.
struct Release {
    version: SharedString,
    /// Its numbers, "0.22.3" as `[0, 22, 3]`, to compare with the running release's; `None`
    /// when it isn't a version.
    numbers: Option<Vec<u64>>,
    released_at: Option<i64>,
    sections: Vec<Section>,
}

/// A heading's part of a release's notes.
struct Section {
    title: String,
    blocks: Vec<Block>,
}

impl Section {
    /// How many changes it lists: its items, or its blocks when it has none.
    fn count(&self) -> usize {
        let items = (self.blocks.iter())
            .filter(|block| matches!(block.kind, Kind::Item(_)) && block.depth == 0)
            .count();
        if items > 0 { items } else { self.blocks.len() }
    }

    /// What it folds to: "17 fixes", "28 CLI changes".
    fn folded(&self) -> String {
        match self.title.as_str() {
            "Fixes" => plural(self.count(), "fix"),
            title => plural(self.count(), &format!("{title} change")),
        }
    }
}

fn numbers(version: &str) -> Option<Vec<u64>> {
    version.split('.').map(|n| n.parse().ok()).collect()
}

fn releases(published: Vec<Published>) -> Vec<Release> {
    (published.into_iter())
        .map(|release| Release {
            numbers: numbers(&release.version),
            sections: sections(
                &release.version,
                release.notes.as_deref().unwrap_or_default(),
            ),
            released_at: (chrono::DateTime::parse_from_rfc3339(&release.released_at).ok())
                .map(|at| at.timestamp()),
            version: release.version.into(),
        })
        .collect()
}

/// The parts of `notes` that are `version`'s own, by heading. A patch release's notes
/// sometimes carry the releases before it, each under its version on a line of its own, as
/// 0.22.3's carry 0.22.2's: then only what's under `version` counts. What comes before any
/// heading is under Changes, parts under the same heading join, and the folded ones go last.
fn sections(version: &str, notes: &str) -> Vec<Section> {
    let blocks = markdown::parse(notes);
    let names_version = |block: &Block| {
        matches!(block.kind, Kind::Paragraph | Kind::Heading(_))
            && numbers(block.text.trim()).is_some_and(|n| n.len() == 3)
    };
    let split = (blocks.iter()).any(|block| names_version(block) && block.text.trim() == version);
    let (mut under, mut title) = (true, "Changes".to_owned());
    let mut sections: Vec<Section> = Vec::new();
    for block in blocks {
        if names_version(&block) {
            under = !split || block.text.trim() == version;
            title = "Changes".to_owned();
            continue;
        }
        if !under {
            continue;
        }
        if let Kind::Heading(_) = block.kind {
            title = block.text.trim().to_owned();
            continue;
        }
        match sections.iter_mut().find(|section| section.title == title) {
            Some(section) => section.blocks.push(block),
            None => sections.push(Section {
                title: title.clone(),
                blocks: vec![block],
            }),
        }
    }
    // Stable, so each keeps its place among its kind.
    sections.sort_by_key(|section| FOLDED.contains(&section.title.as_str()));
    sections
}

/// What's picked once `incoming` replaces `shown`: the reader's pick while `incoming` lists it,
/// or else the newest. A pick of what `shown` had newest was the default, so it moves on.
fn still_picked(
    picked: Option<&SharedString>,
    shown: &[Release],
    incoming: &[Release],
) -> Option<SharedString> {
    let newest = |releases: &[Release]| releases.first().map(|release| release.version.clone());
    match picked {
        Some(picked)
            if newest(shown).as_ref() != Some(picked)
                && incoming.iter().any(|release| &release.version == picked) =>
        {
            Some(picked.clone())
        }
        _ => newest(incoming),
    }
}

/// `release`'s blocks that mention `query`, marked where they do. `query` is lower-case ASCII,
/// and the text is lowered the same way, which keeps every byte where it was.
fn matches(release: &Release, query: &str) -> Vec<Block> {
    let mark = Style {
        mark: true,
        ..Style::default()
    };
    (release.sections.iter().flat_map(|section| &section.blocks))
        .filter_map(|block| {
            let text = block.text.to_ascii_lowercase();
            let found: Vec<Range<usize>> = (text.match_indices(query))
                .map(|(at, hit)| at..at + hit.len())
                .collect();
            if found.is_empty() {
                return None;
            }
            let mut block = block.clone();
            block
                .styles
                .extend(found.into_iter().map(|range| (range, mark)));
            Some(block)
        })
        .collect()
}

pub struct WhatsNew {
    search: Entity<TextInput>,
    /// The running release's numbers; `None` on a nightly or a build of its own, whose
    /// versions count apart from releases'.
    running: Option<Vec<u64>>,
    /// On a nightly, whose versions can't say which releases it has.
    nightly: bool,
    /// The releases, newest first, or why they couldn't be read; `None` while they're read.
    releases: Option<Result<Vec<Release>, SharedString>>,
    selected: Option<SharedString>,
    /// The folded sections opened, by release and heading.
    unfolded: HashSet<(SharedString, String)>,
    releases_scroll: ScrollHandle,
    notes_scroll: ScrollHandle,
    _read: Task<()>,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for WhatsNew {}

impl Focusable for WhatsNew {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl WhatsNew {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search release notes", false, cx));
        let subscription =
            cx.subscribe_in(&search, window, |this, search, event, _, cx| match event {
                InputEvent::Changed => {
                    this.notes_scroll.set_offset(Default::default());
                    cx.notify()
                }
                InputEvent::Cancel if search.read(cx).text().is_empty() => cx.emit(DismissEvent),
                InputEvent::Cancel => search.update(cx, |search, cx| search.set_text("", cx)),
                InputEvent::Submit => {}
            });
        let channel = backend::settings::channel();
        let running = match channel {
            "release" => cx
                .try_global::<crate::Embedded>()
                .and_then(|e| numbers(&e.version)),
            _ => None,
        };
        let mut view = Self {
            search,
            running,
            nightly: channel == "nightly",
            releases: None,
            selected: None,
            unfolded: HashSet::new(),
            releases_scroll: ScrollHandle::new(),
            notes_scroll: ScrollHandle::new(),
            _read: Task::ready(()),
            _subscription: subscription,
        };
        view.read(cx);
        view
    }

    /// Show the releases as last read, at once, then read them again, which takes most of a
    /// second. A read that fails leaves what was shown.
    fn read(&mut self, cx: &mut Context<Self>) {
        self.releases = None;
        self._read = cx.spawn(async move |this, cx| {
            let cached = backend::run(|| Ok(releases(backend::settings::cached_releases()?)));
            if let Ok(cached) = cached.await {
                this.update(cx, |this, cx| this.show(Ok(cached), cx)).ok();
            }
            let read = backend::run(|| Ok(releases(backend::settings::releases()?))).await;
            this.update(cx, |this, cx| match read {
                Ok(read) => this.show(Ok(read), cx),
                Err(_) if matches!(this.releases, Some(Ok(_))) => {}
                Err(err) => this.show(Err(error_summary(&format!("{err:#}")).into()), cx),
            })
            .ok();
        });
    }

    /// Show `releases`, picking the newest unless another was picked.
    fn show(&mut self, releases: Result<Vec<Release>, SharedString>, cx: &mut Context<Self>) {
        let shown = match &self.releases {
            Some(Ok(shown)) => shown.as_slice(),
            _ => &[],
        };
        let incoming = releases.as_deref().unwrap_or_default();
        self.selected = still_picked(self.selected.as_ref(), shown, incoming);
        self.releases = Some(releases);
        cx.notify();
    }

    /// Show `version`'s notes, the `row`th release. As a Settings page does, it ends a search.
    fn pick(&mut self, version: SharedString, row: usize, cx: &mut Context<Self>) {
        self.selected = Some(version);
        self.releases_scroll.scroll_to_item(row);
        self.search.update(cx, |search, cx| search.set_text("", cx));
        self.notes_scroll.set_offset(Default::default());
        cx.notify();
    }

    /// What's searched for, lowered as [`matches`] wants it.
    fn query(&self, cx: &App) -> String {
        self.search.read(cx).text().trim().to_ascii_lowercase()
    }

    fn previous_release(&mut self, _: &PreviousRelease, _: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, cx);
    }

    fn next_release(&mut self, _: &NextRelease, _: &mut Window, cx: &mut Context<Self>) {
        self.step(1, cx);
    }

    /// Pick the release `by` rows from the picked one, while not searching.
    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(Ok(releases)) = &self.releases else {
            return;
        };
        if !self.query(cx).is_empty() {
            return;
        }
        let at = (releases.iter()).position(|r| Some(&r.version) == self.selected.as_ref());
        let next = (at.and_then(|at| at.checked_add_signed(by)))
            .and_then(|row| Some((row, releases.get(row)?.version.clone())));
        if let Some((row, version)) = next {
            self.pick(version, row, cx);
        }
    }

    /// Whether the running release has `release`; `None` when that can't be told.
    fn has(&self, release: &Release) -> Option<bool> {
        Some(release.numbers.as_ref()? <= self.running.as_ref()?)
    }

    fn is_running(&self, release: &Release) -> bool {
        (self.running.as_ref()).is_some_and(|n| release.numbers.as_ref() == Some(n))
    }

    /// `release`'s row, under its minor version when it's the first of it.
    fn release_row(
        &self,
        release: &Release,
        row: usize,
        heading: Option<String>,
        searching: bool,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let selected = !searching && self.selected.as_ref() == Some(&release.version);
        let version = release.version.clone();
        // Not installed yet: a ring, as a dot that isn't filled.
        let marker = div().flex_none().size(dp(6.)).rounded_full();
        let marker = match self.has(release) {
            Some(false) => marker.border_1().border_color(match selected {
                true => theme.muted_on_solid(),
                false => theme.text_3,
            }),
            _ => marker,
        };
        let date = release.released_at.map(ui::local_date).unwrap_or_default();
        let line = side_row(
            SharedString::from(format!("release-{version}")),
            selected,
            true,
            theme,
        )
        .mx_0()
        .h(dp(28.))
        .gap(dp(8.))
        .rounded(dp(6.))
        .child(marker)
        .child(
            div()
                .font_family(theme.mono_font.clone())
                .text_size(dp(12.))
                .child(version.clone()),
        )
        .when(self.is_running(release), |el| {
            el.child(
                div()
                    .px(dp(5.))
                    .rounded(dp(4.))
                    // On a solid selection, in its own muted colour, as the row's other text.
                    .bg(match selected {
                        true => theme.text_inverted.opacity(0.15),
                        false => theme.pop.opacity(0.15),
                    })
                    .text_color(match selected {
                        true => theme.muted_on_solid(),
                        false => theme.pop,
                    })
                    .text_size(dp(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Yours"),
            )
        })
        .child(div().flex_grow(1.))
        .child(quiet(faint(date, theme), selected, theme))
        .on_click(cx.listener(move |this, _, _, cx| this.pick(version.clone(), row, cx)));
        v_flex()
            .children(heading.map(|heading| {
                section_label(heading, theme)
                    .px(dp(8.))
                    .pt(dp(12.))
                    .pb(dp(4.))
            }))
            .child(line)
            .into_any_element()
    }

    fn notes(&self, release: &Release, theme: &Theme, cx: &Context<Self>) -> Vec<AnyElement> {
        let mut said: Vec<String> = release.released_at.map(ui::day_label).into_iter().collect();
        if self.is_running(release) {
            said.push("Your version".to_owned());
        }
        let mut parts = vec![
            v_flex()
                .gap(dp(4.))
                .child(
                    div()
                        .font_family(theme.mono_font.clone())
                        .text_size(dp(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(release.version.clone()),
                )
                .child(faint(said.join(" · "), theme).text_size(dp(12.)))
                .into_any_element(),
        ];
        if self.has(release) == Some(false) {
            parts.push(
                div()
                    .px(dp(12.))
                    .py(dp(8.))
                    .rounded(dp(6.))
                    .bg(theme.pop.opacity(0.1))
                    .text_size(dp(13.))
                    .child("Not in your version yet")
                    .into_any_element(),
            );
        }
        for section in &release.sections {
            let folds = FOLDED.contains(&section.title.as_str());
            let key = (release.version.clone(), section.title.clone());
            let open = !folds || self.unfolded.contains(&key);
            let id = format!("notes-{}-{}", release.version, section.title);
            let head = match folds {
                true => h_flex()
                    .id(SharedString::from(id.clone()))
                    .gap(dp(4.))
                    .text_size(dp(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_2)
                    .hover(|el| el.text_color(theme.text_1))
                    .child(fold_glyph(Some(open), theme.text_3))
                    .child(section.folded())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.unfolded.remove(&key) {
                            this.unfolded.insert(key.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
                false => section_label(section.title.clone(), theme).into_any_element(),
            };
            parts.push(
                v_flex()
                    .gap(dp(6.))
                    .child(head)
                    .when(open, |el| {
                        el.child(markdown::render(&id, &section.blocks, theme))
                    })
                    .into_any_element(),
            );
        }
        if release.sections.is_empty() {
            parts.push(muted("No notes for this release", theme).into_any_element());
        }
        let url = format!("{RELEASES}/tag/release%2F{}", release.version);
        parts.push(
            div()
                .child(
                    button("whats-new-github", "Open on GitHub")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                )
                .into_any_element(),
        );
        parts
    }

    /// The matches for `typed` in each release that has any, under a line that says how many.
    fn found(
        &self,
        releases: &[Release],
        found: &[(&Release, Vec<Block>)],
        typed: &str,
        theme: &Theme,
    ) -> Vec<AnyElement> {
        if found.is_empty() {
            let said = format!(
                "Nothing in the last {} mentions {}",
                plural(releases.len(), "release"),
                quoted(typed)
            );
            return vec![muted(said, theme).into_any_element()];
        }
        let total: usize = found.iter().map(|(_, blocks)| blocks.len()).sum();
        let said = format!(
            "{} in {}",
            plural(total, "match"),
            plural(found.len(), "release")
        );
        let mut parts = vec![faint(said, theme).text_size(dp(12.)).into_any_element()];
        let mut left = SHOWN;
        for (release, blocks) in found {
            if left == 0 {
                break;
            }
            let blocks = blocks.get(..left).unwrap_or(blocks);
            left -= blocks.len();
            let has = self.has(release).map(|has| match has {
                true => h_flex()
                    .gap(dp(4.))
                    .text_color(theme.safe)
                    .child(small_icon(Icon::Tick).text_color(theme.safe))
                    .child("In your version"),
                false => div()
                    .text_color(theme.text_2)
                    .child(format!("Needs {}", release.version)),
            });
            parts.push(
                v_flex()
                    .gap(dp(6.))
                    .child(
                        h_flex()
                            .gap(dp(8.))
                            .pb(dp(4.))
                            .border_b_1()
                            .border_color(theme.border_3)
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_size(dp(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(release.version.clone()),
                            )
                            .child(faint(
                                release.released_at.map(ui::local_date).unwrap_or_default(),
                                theme,
                            ))
                            .child(div().flex_grow(1.))
                            .children(has.map(|has| has.text_size(dp(12.)))),
                    )
                    .child(markdown::render(
                        &format!("found-{}", release.version),
                        blocks,
                        theme,
                    ))
                    .into_any_element(),
            );
        }
        if total > SHOWN {
            let said = format!("Showing the first {SHOWN}. Type more to narrow them down");
            parts.push(muted(said, theme).into_any_element());
        }
        parts
    }

    fn failed(&self, err: &SharedString, theme: &Theme, cx: &Context<Self>) -> Vec<AnyElement> {
        vec![
            v_flex()
                .gap(dp(8.))
                .items_start()
                .child(muted("Couldn't read the release notes", theme))
                .child(faint(err.clone(), theme))
                .child(
                    h_flex()
                        .gap(dp(8.))
                        .child(
                            button("whats-new-retry", "Try again")
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| this.read(cx))),
                        )
                        .child(
                            button("whats-new-releases", "Open on GitHub")
                                .small()
                                .style(ButtonStyle::Ghost)
                                .on_click(|_, _, cx| cx.open_url(RELEASES)),
                        ),
                )
                .into_any_element(),
        ]
    }
}

impl Render for WhatsNew {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let typed = self.search.read(cx).text().trim().to_owned();
        let query = self.query(cx);
        let searching = !query.is_empty();
        let focused = self.search.focus_handle(cx).is_focused(window);
        let height = (f32::from(window.viewport_size().height) / theme.scale * 0.76).min(1000.);
        let listed = match &self.releases {
            Some(Ok(releases)) => releases.as_slice(),
            _ => &[],
        };
        let mut rows = Vec::new();
        let mut minor = None;
        for (row, release) in listed.iter().enumerate() {
            // The minor version, "0.22", heads its releases.
            let heading = (release.numbers.as_ref())
                .and_then(|n| Some(format!("{}.{}", n.first()?, n.get(1)?)));
            let starts = heading != minor;
            minor = heading.clone();
            let heading = heading.filter(|_| starts);
            rows.push(self.release_row(release, row, heading, searching, &theme, cx));
        }
        let list = v_flex()
            .id("whats-new-releases")
            .size_full()
            .px(dp(12.))
            .pb(dp(12.))
            .overflow_y_scroll()
            .track_scroll(&self.releases_scroll)
            .children(rows);
        let side = v_flex()
            .w(dp(232.))
            .flex_none()
            .h_full()
            .bg(theme.bg_2)
            // Its ground fills the modal's left corners, which it rounds as the modal does.
            .rounded_l(dp(MODAL_RADIUS - 1.))
            .child(
                div()
                    .px(dp(20.))
                    .pt(dp(20.))
                    .pb(dp(10.))
                    .text_size(dp(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_1)
                    .child("What's new"),
            )
            .when(self.nightly, |el| {
                el.child(
                    faint("You're on nightly, which gets changes first", &theme)
                        .px(dp(20.))
                        .pb(dp(10.)),
                )
            })
            .child(field(&self.search, focused, &theme).mx(dp(12.)).mb(dp(10.)))
            .child(scrolled(
                list,
                "whats-new-releases-scrollbar",
                Scroll::Div(self.releases_scroll.clone()),
                &theme,
            ));
        let content = match &self.releases {
            None => None,
            Some(Err(err)) => Some(self.failed(err, &theme, cx)),
            Some(Ok(releases)) if searching => {
                let found: Vec<(&Release, Vec<Block>)> = (releases.iter())
                    .map(|release| (release, matches(release, &query)))
                    .filter(|(_, blocks)| !blocks.is_empty())
                    .collect();
                Some(self.found(releases, &found, &typed, &theme))
            }
            Some(Ok(releases)) => Some(
                (releases.iter())
                    .find(|release| self.selected.as_ref() == Some(&release.version))
                    .map(|release| self.notes(release, &theme, cx))
                    // Only when there are none to pick.
                    .unwrap_or_default(),
            ),
        };
        let pane = match content {
            Some(content) => scrolled(
                v_flex()
                    .id("whats-new-notes")
                    .size_full()
                    .p(dp(24.))
                    .gap(dp(20.))
                    .overflow_y_scroll()
                    .track_scroll(&self.notes_scroll)
                    .children(content),
                "whats-new-notes-scrollbar",
                Scroll::Div(self.notes_scroll.clone()),
                &theme,
            )
            .into_any_element(),
            None => ui::loading_state("Loading release notes", &theme, cx),
        };
        modal(ModalSize::Large, &theme)
            .id("whats-new")
            .key_context("WhatsNew")
            .on_action(cx.listener(Self::previous_release))
            .on_action(cx.listener(Self::next_release))
            .h(dp(height))
            .flex_row()
            .child(side)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .border_l_1()
                    .border_color(theme.border_3)
                    .bg(theme.bg_1)
                    .rounded_r(dp(MODAL_RADIUS - 1.))
                    .child(pane),
            )
    }
}

impl AppView {
    pub fn open_whats_new(
        &mut self,
        _: &OpenReleaseNotes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.new(|cx| WhatsNew::new(window, cx));
        self.open_modal(view, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(sections: &[Section]) -> Vec<(&str, usize)> {
        (sections.iter())
            .map(|section| (section.title.as_str(), section.count()))
            .collect()
    }

    #[test]
    fn notes_read_into_sections_with_the_folded_ones_last() {
        let notes =
            "## Fixes\n- one\n- two\n\n## Changes\n- new\n\n## CLI\n- cli\n\n## Fixes\n- three\n";
        assert_eq!(
            titles(&sections("0.22.2", notes)),
            [("Changes", 1), ("Fixes", 3), ("CLI", 1)],
            "Changes leads, both Fixes headings join, and the folded ones keep their order"
        );
        assert_eq!(
            titles(&sections("0.22.2", "- untitled\n- items\n")),
            [("Changes", 2)],
            "what comes before any heading is a change"
        );
    }

    #[test]
    fn a_patch_release_keeps_only_its_own_notes() {
        let notes = "**0.22.3**\n\n- fix a\n- fix b\n\n**0.22.2**\n\n## Changes\n- older\n\n## Fixes\n- older fix\n";
        assert_eq!(
            titles(&sections("0.22.3", notes)),
            [("Changes", 2)],
            "the notes it carries for 0.22.2 aren't 0.22.3's"
        );
        assert_eq!(
            titles(&sections(
                "0.20.2",
                "## 0.20.2\n- first\n\n## Changes\n- second\n"
            )),
            [("Changes", 2)],
            "a heading that only names the version starts its notes"
        );
    }

    #[test]
    fn a_search_marks_every_match_in_the_blocks_that_have_one() {
        let release = Release {
            version: "0.22.1".into(),
            numbers: numbers("0.22.1"),
            released_at: None,
            sections: sections(
                "0.22.1",
                "- Conflicts keep both, conflict marked\n- Pushes\n",
            ),
        };
        let found = matches(&release, "conflict");
        let marked: Vec<_> = (found.iter().flat_map(|block| &block.styles))
            .filter(|(_, style)| style.mark)
            .map(|(range, _)| range.clone())
            .collect();
        assert_eq!(found.len(), 1, "only the block that mentions it");
        assert_eq!(marked, [0..8, 21..29], "both, whatever their case");
    }

    fn listed(versions: &[&str]) -> Vec<Release> {
        (versions.iter())
            .map(|version| Release {
                version: SharedString::from(version.to_string()),
                numbers: numbers(version),
                released_at: None,
                sections: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn a_fresh_list_keeps_the_readers_pick_only_while_it_lists_it() {
        let (cached, fresh) = (listed(&["0.22.3", "0.1.0"]), listed(&["0.23.0", "0.22.3"]));
        let picked = |version: &str| still_picked(Some(&version.into()), &cached, &fresh);
        assert_eq!(
            picked("0.22.3"),
            Some("0.23.0".into()),
            "the default moves to the new newest"
        );
        assert_eq!(
            still_picked(
                Some(&"0.22.3".into()),
                &listed(&["0.23.0", "0.22.3"]),
                &fresh
            ),
            Some("0.22.3".into()),
            "a release the reader picked stays while it's listed"
        );
        assert_eq!(
            picked("0.1.0"),
            Some("0.23.0".into()),
            "one the fresh list dropped gives way to the newest, rather than nothing shown"
        );
    }

    #[test]
    fn versions_compare_by_their_numbers() {
        assert!(
            numbers("0.22.10") > numbers("0.22.9"),
            "10 comes after 9, as text it wouldn't"
        );
        assert_eq!(
            numbers("nightly"),
            None,
            "a name that isn't a version has no numbers"
        );
    }
}

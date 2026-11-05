//! The top of a Brief tab: what it is and who wrote it, then what's known about it apart from
//! what the agent wrote. Those facts sit in a panel of their own, so what the repository and the
//! CLI measured never reads like a sentence the agent could have made up. Colour goes only on
//! what should change a decision.

use std::sync::Arc;

use gpui::{AppContext as _, Context, Div, FontWeight, SharedString, div, prelude::*};

use super::state::{BriefFold, BriefState};
use crate::backend::briefs::{self, Ask, BriefPage, Concern, Found, ReadBack, Standing, Thread};
use crate::backend::{CheckState, FileDiff};
use crate::detail::Detail;
use crate::detail::page::PageTab;
use crate::state::Selection;
use crate::theme::Theme;
use crate::ui::person::person_avatar;
use crate::ui::{
    ButtonStyle, Icon, Tooltip, ago, button, diff, dp, h_flex, icon, icon_button, plural, v_flex,
};

/// How wide the column of what's measured is, in design pixels.
const LABEL_WIDTH: f32 = 100.;

/// How many of what was said on earlier versions the top lists.
const MOST_READ_BACK: usize = 4;

/// How many of the files a brief shows no code of get listed, the biggest first.
const MOST_UNSHOWN_LISTED: usize = 12;

/// A brief's words for what it asks, as its web page says them.
fn ask_words(ask: &Ask) -> Option<&'static str> {
    match ask {
        Ask::SignOff => Some("For sign-off"),
        Ask::HeadsUp => Some("Heads-up"),
        Ask::Decision { .. } => Some("For a decision"),
        Ask::Other => None,
    }
}

/// A row of the facts panel.
fn fact(words: impl IntoElement) -> Div {
    div().min_w_0().line_height(dp(18.)).child(words)
}

/// A row of the facts panel's measures: what's measured in a column of its own, and what it came to.
fn measure(label: &str, value: impl IntoElement, theme: &Theme) -> Div {
    h_flex()
        .items_start()
        .line_height(dp(18.))
        .child(
            div()
                .flex_none()
                .w(dp(LABEL_WIDTH))
                .text_color(theme.text_3)
                .child(label.to_owned()),
        )
        .child(div().flex_1().min_w_0().child(value))
}

impl Detail {
    /// What the brief is, who wrote it, and what's known about it.
    pub(super) fn brief_head(&self, found: &Found, state: &BriefState, cx: &Context<Self>) -> Div {
        let theme = Theme::get(cx);
        let page = state.page();
        let version = page.map_or(found.matched.version, |page| page.version);
        let latest = (found.latest)
            .max(version)
            .max(state.activity.latest.unwrap_or(0));
        let title = page.map_or(found.title.as_str(), |page| page.title.as_str());
        let url = briefs::web_url(&found.id, version);
        let status = match &state.shown {
            None => Some("Loading…".to_owned()),
            Some(Err(err)) => Some(format!("Couldn't read this brief: {err}")),
            Some(Ok(_)) => None,
        };
        v_flex()
            .gap(dp(10.))
            .child(
                div()
                    .text_size(dp(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_1)
                    .child(title.to_owned()),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_x(dp(10.))
                    .text_size(dp(12.))
                    .text_color(theme.text_3)
                    .child(person_avatar(&found.author.name, 18., theme))
                    .child(
                        div()
                            .text_color(theme.text_2)
                            .child(found.author.name.clone()),
                    )
                    .children(
                        (page.and_then(|page| page.agent.as_deref()))
                            .map(|agent| div().child(format!("with {agent}"))),
                    )
                    .children(page.and_then(|page| page.published).map(ago))
                    .child(self.brief_versions(version, latest, cx))
                    .child(
                        // In a narrow tile these go under, rather than crushing the rest.
                        h_flex()
                            .ml_auto()
                            .children(self.can_show_changes_beside(cx).then(|| {
                                button("brief-beside", "Changes beside")
                                    .icon(Icon::SplitRight)
                                    .style(ButtonStyle::Ghost)
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.show_changes_beside(window, cx)
                                    }))
                            }))
                            .child(
                                icon_button("brief-web", Icon::Globe)
                                    .small()
                                    .tooltip("Open on the web")
                                    .on_click(move |_, _, cx| cx.open_url(&url)),
                            ),
                    ),
            )
            .children(status.map(|status| super::small_line(&status, theme.text_3)))
            .children(page.map(|page| self.brief_facts(page, state, cx)))
    }

    /// Which version this is of how many, and the way to the others.
    fn brief_versions(&self, version: u32, latest: u32, cx: &Context<Self>) -> Div {
        let step = |id: &'static str, glyph: Icon, to: Option<u32>, tip: &'static str| {
            icon_button(id, glyph)
                .small()
                .tooltip(if to.is_some() {
                    tip
                } else {
                    "There's no other that way"
                })
                .disabled_because(to.is_none().then_some("There's no other that way"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let (Some(to), Some(brief)) = (to, this.page.brief_mut()) {
                        brief.version = Some(to);
                        this.ask_briefs(cx);
                        cx.notify();
                    }
                }))
        };
        let words = match latest > 1 {
            true => format!("Version {version} of {latest}"),
            false => format!("Version {version}"),
        };
        h_flex().gap(dp(2.)).child(words).when(latest > 1, |el| {
            el.child(step(
                "brief-version-older",
                Icon::ChevronLeft,
                (version > 1).then(|| version - 1),
                "The version before",
            ))
            .child(step(
                "brief-version-newer",
                Icon::ChevronRight,
                (version < latest).then(|| version + 1),
                "The version after",
            ))
        })
    }

    /// What's known about the brief that the agent didn't write, newest news first: what's come
    /// since, what became of what was said, whether it's still true of the branch, how big the
    /// change is, what it leaves out, and what the CLI measured.
    fn brief_facts(&self, page: &BriefPage, state: &BriefState, cx: &Context<Self>) -> Div {
        let theme = Theme::get(cx);
        // Where it can be answered, the row of the tabs says what's asked.
        let ask = (page.ask.as_ref()).filter(|_| !state.activity.answers.can_answer);
        let ask = ask.and_then(ask_words).map(|words| {
            fact(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_1)
                    .child(words),
            )
        });
        let options = match &page.ask {
            Some(Ask::Decision { options }) => options.clone(),
            _ => Vec::new(),
        };
        let (stale, standing) = standing_fact(page, theme, cx);
        let (stale_alert, current) = match stale {
            true => (Some(standing), None),
            false => (None, Some(standing)),
        };
        let others: Vec<Div> = (self.brief_failed_checks_fact(cx).into_iter())
            .chain(self.brief_turned_off_fact(state, cx))
            .chain(self.brief_unnamed_facts(state, cx))
            .collect();
        // Whether it matches, first, as that's what decides how far to go by the rest.
        let verdict = |words: &str| {
            fact(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.warn)
                    .child(words.to_owned()),
            )
        };
        // Written at the tip, though not of what the branch now compares as, can't be said to match.
        let at_tip = page.standing == Standing::Current && !stale && !self.brief_sizes_differ(page);
        let alert_verdict = match (stale, at_tip, others.is_empty()) {
            (true, _, _) => Some(verdict("Doesn't match the branch as it is")),
            (false, true, false) => Some(verdict("Matches the latest commit, but check these")),
            _ => None,
        };
        let current = match (at_tip, others.is_empty()) {
            (true, true) => Some(fact(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_1)
                    .child("Matches the branch's latest commit"),
            )),
            (true, false) => None,
            (false, _) => current,
        };
        let alerts: Vec<Div> = (alert_verdict.into_iter())
            .chain(stale_alert)
            .chain(others)
            .collect();
        let news: Vec<Div> = (self.brief_read_back_fact(page, state, cx).into_iter())
            .chain(self.brief_answered_fact(state, theme))
            .collect();
        v_flex()
            .rounded(dp(6.))
            .border_1()
            .border_color(theme.border_3)
            .overflow_hidden()
            .text_size(dp(12.))
            .text_color(theme.text_2)
            .when(!news.is_empty(), |el| {
                el.child(
                    v_flex()
                        .gap(dp(5.))
                        .px(dp(10.))
                        .py(dp(7.))
                        .border_b_1()
                        .border_color(theme.border_3)
                        .children(news),
                )
            })
            .when(!alerts.is_empty(), |el| {
                el.child(
                    v_flex()
                        .gap(dp(8.))
                        .px(dp(10.))
                        .py(dp(7.))
                        .bg(theme.warn.opacity(0.1))
                        .children(alerts),
                )
            })
            .child(
                v_flex()
                    .gap(dp(3.))
                    .px(dp(10.))
                    .py(dp(7.))
                    .bg(theme.fill_gray.opacity(0.04))
                    .children(ask)
                    .children(options.into_iter().map(|option| fact(div().child(option))))
                    .children(current)
                    .children(self.brief_checks_fact(cx))
                    .children(self.brief_size_facts(page, theme))
                    .children(self.brief_tests_fact(state, cx))
                    .children(self.brief_unshown_fact(page, state, cx))
                    .children(self.brief_labels_facts(page, theme)),
            )
    }

    /// Who's answered what the brief asks.
    fn brief_answered_fact(&self, state: &BriefState, theme: &Theme) -> Option<Div> {
        let words: Vec<String> = (state.activity.answers.by.iter())
            .map(|by| format!("{} by {}", answer_verb(&by.answer), by.name))
            .collect();
        (!words.is_empty()).then(|| {
            h_flex()
                .flex_wrap()
                .gap_x(dp(10.))
                .text_color(theme.text_1)
                .children(words)
        })
    }

    /// Whether what the brief measured is of the branch as it is.
    fn brief_sizes_differ(&self, page: &BriefPage) -> bool {
        let (Some(changed), Some(loaded)) = (
            page.changed.filter(|changed| changed.files > 0),
            self.shown_diff().filter(|diff| !diff.files.is_empty()),
        ) else {
            return false;
        };
        let (added, removed) = diff::total_stats(loaded.files.iter());
        (changed.files, changed.additions, changed.deletions)
            != (loaded.files.len() as u32, added, removed)
    }

    /// How big the brief says the change is, and how big the branch's is now, when those differ.
    fn brief_size_facts(&self, page: &BriefPage, theme: &Theme) -> Vec<Div> {
        // A brief can explain code at one commit, which compares nothing.
        let Some(changed) = page.changed.filter(|changed| changed.files > 0) else {
            return Vec::new();
        };
        let sized = |files: usize, added: u32, removed: u32| {
            h_flex()
                .gap(dp(5.))
                .child(plural(files, "file"))
                .child(diff::stats(added, removed, false, theme))
        };
        let written = sized(changed.files as usize, changed.additions, changed.deletions);
        match self.brief_sizes_differ(page) {
            false => vec![measure("Changes", written, theme)],
            true => {
                let mut rows = vec![measure("Brief covers", written, theme)];
                rows.extend(self.shown_diff().map(|loaded| {
                    let (added, removed) = diff::total_stats(loaded.files.iter());
                    measure(
                        "Branch changes",
                        sized(loaded.files.len(), added, removed),
                        theme,
                    )
                }));
                rows
            }
        }
    }

    /// A line of the branch's that turns a test off, whatever the brief says about its tests. It's
    /// the line itself, which says which test, and goes to where it is.
    fn brief_turned_off_fact(&self, state: &BriefState, cx: &Context<Self>) -> Option<Div> {
        let theme = Theme::get(cx);
        let tests = state.tests.as_ref()?;
        let first = tests.turned_off.first()?;
        let more = tests.turned_off.len() - 1;
        let (path, hunk, line) = (first.path.clone(), first.hunk, first.line);
        let name = (first.path.rsplit('/').next())
            .unwrap_or(&first.path)
            .to_owned();
        Some(
            v_flex()
                .id("brief-test-off")
                .gap(dp(2.))
                .hover(|el| el.bg(theme.warn.opacity(0.06)))
                .role(gpui::Role::Button)
                .aria_label(SharedString::from(format!("Show the line in {name}")))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.brief_go_to_line(&path, hunk, line, window, cx)
                }))
                .child(div().text_color(theme.warn).child(match more {
                    0 => format!("Turns off a test in {name}"),
                    more => format!("Turns off a test in {name}, and {more} more"),
                }))
                .child(
                    div()
                        .truncate()
                        .font_family(theme.diff_font.clone())
                        .text_color(theme.text_1)
                        .child(first.text.clone()),
                )
                .into_any_element(),
        )
        .map(|el| div().child(el))
    }

    /// What the diff says of the branch's tests: how much they change, against what the brief says
    /// about them.
    fn brief_tests_fact(&self, state: &BriefState, cx: &Context<Self>) -> Option<Div> {
        let theme = Theme::get(cx);
        let tests = state.tests.as_ref()?;
        if !tests.deleted.is_empty() {
            return Some(fact(
                div()
                    .text_color(theme.warn)
                    .child(format!("Deletes tests: {}", tests.deleted.join(", "))),
            ));
        }
        match tests.files.len() {
            0 if tests.changes_code => Some(measure("Tests", "none changed", theme)),
            0 => None,
            files => {
                let row = h_flex()
                    .gap_x(dp(5.))
                    .child(plural(files, "file"))
                    .child(diff::stats(tests.added, tests.removed, false, theme));
                Some(measure("Tests", row, theme))
            }
        }
    }

    /// What's been said on versions before this one, and what became of each: who answered, and
    /// what the version shown did about it, by what's there rather than by the answer.
    fn brief_read_back_fact(
        &self,
        page: &BriefPage,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let theme = Theme::get(cx);
        // Those on the version last read, which this one can be held to.
        let earlier: Vec<&Thread> = (state.activity.threads.iter())
            .filter(|thread| thread.resolved.is_none())
            .filter(|thread| page.read_back(thread) != ReadBack::CannotTell)
            .collect();
        if earlier.is_empty() {
            return None;
        }
        let rows = earlier.into_iter().take(MOST_READ_BACK).map(|thread| {
            let reply = thread.replies.last().map(|reply| {
                format!(
                    "{}: {}",
                    agent_name(reply.actor.agent.as_deref().unwrap_or(&reply.actor.name)),
                    reply.body
                )
            });
            let (color, verdict) = match (page.read_back(thread), reply.is_some()) {
                (ReadBack::Untouched, true) => (
                    theme.warn,
                    Some(format!(
                        "But that part didn't change in version {}",
                        page.version
                    )),
                ),
                (ReadBack::Untouched, false) => (
                    theme.warn,
                    Some(format!(
                        "That part didn't change in version {}",
                        page.version
                    )),
                ),
                (ReadBack::Changed, _) => (
                    theme.pop,
                    Some(format!("That part changed in version {}", page.version)),
                ),
                (ReadBack::CannotTell, _) => (theme.text_3, None),
            };
            v_flex()
                .child(
                    div()
                        .truncate()
                        .text_color(theme.text_3)
                        .child(format!("{}: {}", thread.actor.name, thread.body)),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_x(dp(6.))
                        .child(
                            div()
                                .text_color(theme.text_1)
                                .child(reply.unwrap_or_else(|| "No answer yet.".to_owned())),
                        )
                        .children(verdict.map(|verdict| {
                            div()
                                .text_color(color)
                                .font_weight(FontWeight::MEDIUM)
                                .child(verdict)
                        })),
                )
        });
        Some(v_flex().gap(dp(5.)).children(rows))
    }

    /// What the forge says ran on the branch, apart from what the brief says was checked.
    fn brief_checks_fact(&self, cx: &Context<Self>) -> Option<Div> {
        let theme = Theme::get(cx);
        let review = self.selected_review(cx)?;
        let checks = review.checks?;
        let number = review.number;
        let words = match (checks.failed, checks.running) {
            (0, 0) => format!("{} passed on PR #{number}", plural(checks.passed, "check")),
            (0, running) => format!("{} running on PR #{number}", plural(running, "check")),
            (_, _) => return None,
        };
        Some(measure("Checks", words, theme))
    }

    /// The checks that failed on the branch's pull request, whatever the brief says of them, and
    /// the way to them.
    fn brief_failed_checks_fact(&self, cx: &Context<Self>) -> Option<Div> {
        let theme = Theme::get(cx);
        let review = self.selected_review(cx)?;
        let checks = review.checks.filter(|checks| checks.failed > 0)?;
        let failed: Vec<&str> = (review.check_runs.iter())
            .filter(|run| matches!(run.state, CheckState::Failed | CheckState::Cancelled))
            .map(|run| run.name.as_str())
            .collect();
        let names = match failed.is_empty() {
            true => plural(checks.failed, "check"),
            false => failed.join(", "),
        };
        Some(
            div().child(
                div()
                    .id("brief-checks-failed")
                    .text_color(theme.warn)
                    .hover(|el| el.underline())
                    .role(gpui::Role::Button)
                    .aria_label(SharedString::from("Show the pull request's checks"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_page_tab(PageTab::Review, window, cx)
                    }))
                    .child(format!("{names} failed on PR #{}", review.number)),
            ),
        )
    }

    /// Which of the files the branch changes now the brief shows no code of: what it leaves out.
    /// Those that tend to matter and that it never even names are always in view, the rest a
    /// click from the list, and each a click from its diff.
    fn brief_unshown_fact(
        &self,
        page: &BriefPage,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let theme = Theme::get(cx);
        let loaded = self
            .shown_diff()
            .filter(|loaded| !loaded.files.is_empty())?;
        let shown = page.code_paths();
        let unshown: Vec<&str> = (loaded.files.iter())
            .map(|file| file.file.path.as_str())
            .filter(|path| !shown.contains(*path))
            .collect();
        let others: Vec<&str> = (unshown.iter().copied())
            .filter(|path| !state.unnamed.iter().any(|unnamed| unnamed == path))
            .collect();
        let total = loaded.files.len();
        let words = match unshown.len() {
            0 => format!("None, of {}", plural(total, "changed file")),
            n => format!("{n} of {}", plural(total, "changed file")),
        };
        let open = state.is_open(BriefFold::Unshown);
        let toggle = h_flex()
            .id("brief-unshown")
            .gap(dp(6.))
            .role(gpui::Role::Button)
            .aria_label(SharedString::from(words.clone()))
            .hover(|el| el.text_color(theme.text_1))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_brief_fold(BriefFold::Unshown, cx)))
            .child(words)
            .when(!others.is_empty(), |el| {
                el.child(
                    icon(if open {
                        Icon::ChevronUp
                    } else {
                        Icon::ChevronDown
                    })
                    .size(dp(10.))
                    .text_color(theme.text_3),
                )
            });
        let listed = (open && !others.is_empty()).then(|| {
            // The biggest first, as that's what's worth a look.
            let mut files: Vec<_> = (loaded.files.iter())
                .filter(|file| others.contains(&file.file.path.as_str()))
                .collect();
            files.sort_by_key(|file| {
                let (added, removed) = file.stats();
                std::cmp::Reverse(added + removed)
            });
            let more = files.len().saturating_sub(MOST_UNSHOWN_LISTED);
            v_flex()
                .mt(dp(2.))
                .rounded(dp(4.))
                .border_1()
                .border_color(theme.border_3)
                .overflow_hidden()
                .children(files.into_iter().take(MOST_UNSHOWN_LISTED).enumerate().map(
                    |(ix, file)| {
                        let (added, removed) = file.stats();
                        self.brief_file_row(
                            SharedString::from(format!("brief-unshown-{ix}")),
                            &file.file.path,
                            None,
                            Some(but_briefs::LineStats {
                                additions: added,
                                deletions: removed,
                            }),
                            None,
                            cx,
                        )
                        .when(ix > 0, |el| el.border_t_1().border_color(theme.border_3))
                    },
                ))
                .when(more > 0, |el| {
                    el.child(
                        div()
                            .px(dp(10.))
                            .py(dp(3.))
                            .border_t_1()
                            .border_color(theme.border_3)
                            .text_color(theme.text_3)
                            .child(format!("and {more} more in Changes")),
                    )
                })
        });
        Some(
            v_flex()
                .gap(dp(4.))
                .child(measure("Not shown", toggle, theme))
                .children(listed),
        )
    }

    /// A row for each changed file that tends to matter which the brief neither shows nor names,
    /// a look away on hover and a click from its diff.
    fn brief_unnamed_facts(&self, state: &BriefState, cx: &Context<Self>) -> Vec<Div> {
        let theme = Theme::get(cx);
        let Some(loaded) = self.shown_diff() else {
            return Vec::new();
        };
        (state.unnamed.iter().enumerate())
            .map(|(ix, path)| {
                let clicked = path.clone();
                h_flex()
                    .flex_wrap()
                    .gap_x(dp(5.))
                    .child(
                        div()
                            .text_color(theme.warn)
                            .child(match changes_data(path) {
                                true => "Database change the brief doesn't mention:",
                                false => "Not mentioned in the brief:",
                            }),
                    )
                    .child(
                        div()
                            .id(("brief-unnamed-file", ix))
                            .truncate()
                            .text_color(theme.text_1)
                            .underline()
                            .text_decoration_color(theme.text_3)
                            .hover(|el| el.text_decoration_color(theme.text_1))
                            .role(gpui::Role::Button)
                            .aria_label(SharedString::from(format!("Show {path} in Changes")))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.brief_open_file(&clicked, None, window, cx)
                            }))
                            .tooltip(peek_of(&loaded.files, path))
                            .child(path.clone()),
                    )
            })
            .collect()
    }

    /// What's different from the version last read, over what's different: what's gone is listed
    /// here, as nowhere else shows it, each a click from asking for it back.
    pub(super) fn brief_since_row(
        &self,
        page: &BriefPage,
        state: &BriefState,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let theme = Theme::get(cx);
        let since = page.since_read.as_ref()?;
        let open = state.is_open(BriefFold::Removed);
        let mut parts = vec![plural(since.changed, "part") + " new or changed"];
        if !since.removed.is_empty() {
            parts.push(format!("{} removed", since.removed.len()));
        }
        let words = format!("Since version {}: {}", since.version, parts.join(", "));
        let toggle =
            h_flex()
                .id("brief-since-read")
                .gap(dp(6.))
                .text_color(theme.pop)
                .font_weight(FontWeight::MEDIUM)
                .when(!since.removed.is_empty(), |el| {
                    el.role(gpui::Role::Button)
                        .aria_label(SharedString::from(words.clone()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_brief_fold(BriefFold::Removed, cx)
                        }))
                })
                .child(words)
                .when(!since.removed.is_empty(), |el| {
                    el.child(
                        icon(if open {
                            Icon::ChevronUp
                        } else {
                            Icon::ChevronDown
                        })
                        .size(dp(10.))
                        .text_color(theme.text_3),
                    )
                });
        let listed = (open && !since.removed.is_empty()).then(|| {
            v_flex()
                .pt(dp(4.))
                .gap(dp(2.))
                .children(since.removed.iter().enumerate().map(|(ix, gone)| {
                    let body = format!("Bring this back: {gone}");
                    let asked = state.notes.iter().any(|note| note.body == body);
                    h_flex()
                        .gap(dp(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(theme.text_2)
                                .child(gone.clone()),
                        )
                        .child(
                            button(
                                ("brief-bring-back", ix),
                                if asked {
                                    "Asked for it"
                                } else {
                                    "Bring it back"
                                },
                            )
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip("Keep a comment asking for this back")
                            .disabled_because(asked.then_some("There's a comment asking for it"))
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.keep_note_on_brief(body.clone(), cx),
                            )),
                        )
                }))
        });
        let only_changed = state.is_open(BriefFold::Unchanged).then(|| {
            button("brief-only-changed", "Show only what changed")
                .small()
                .style(ButtonStyle::Ghost)
                .on_click(
                    cx.listener(|this, _, _, cx| this.toggle_brief_fold(BriefFold::Unchanged, cx)),
                )
        });
        Some(
            v_flex()
                .text_size(dp(12.))
                .child(
                    h_flex()
                        .flex_wrap()
                        .justify_between()
                        .child(toggle)
                        .children(only_changed),
                )
                .children(listed),
        )
    }

    /// What the CLI measured that it thinks is worth a look, in its own numbers, as the name of a
    /// measure says little without them. Of what the brief was written from, so only while that's
    /// what the branch has.
    fn brief_labels_facts(&self, page: &BriefPage, theme: &Theme) -> Vec<Div> {
        if self.brief_sizes_differ(page) {
            return Vec::new();
        }
        (page.labels.iter())
            .filter(|label| matches!(label.concern, Concern::Serious | Concern::Notable))
            .map(|label| {
                let why = div()
                    .when(label.concern == Concern::Serious, |el| {
                        el.text_color(theme.warn)
                    })
                    .child(label.why.clone());
                measure(&label.name, why, theme)
            })
            .collect()
    }
}

/// What goes on hover over file `path`: how it starts to change.
fn peek_of(
    files: &Arc<Vec<FileDiff>>,
    path: &str,
) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView + 'static {
    let (files, path) = (files.clone(), path.to_owned());
    move |window, cx| match super::names::peek_of_file(&files, &path) {
        Some(peek) => cx.new(|_| peek).into(),
        None => Tooltip::text(path.clone())(window, cx),
    }
}

/// The files of `files` that tend to matter and that `page` neither shows nor names.
pub(super) fn unnamed_files(page: &BriefPage, files: &[FileDiff]) -> Vec<String> {
    let shown = page.code_paths();
    let mut unnamed: Vec<String> = (files.iter())
        .map(|file| file.file.path.as_str())
        .filter(|path| !shown.contains(*path) && briefs::weighty(path) && !page.mentions(path))
        .map(str::to_owned)
        .collect();
    // What can change data that's there, first, as it's what can't be taken back by a revert.
    unnamed.sort_by_key(|path| !changes_data(path));
    unnamed
}

/// Whether `path` looks like it changes a database: a migration, or SQL.
fn changes_data(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("migration") || lower.ends_with(".sql")
}

/// The name of an agent without the model it says it runs, as there's little room.
pub(super) fn agent_name(agent: &str) -> &str {
    agent.split(',').next().unwrap_or(agent).trim()
}

fn answer_verb(answer: &str) -> &str {
    match answer {
        "approve" => "Approved",
        "hold" => "Held",
        "acknowledge" => "Seen",
        _ => "Answered",
    }
}

/// Whether the brief is still true of the branch, as the repository says, whether that's bad
/// news, and what's moved on: each later commit, a click from it, and the files they change, a
/// click from their diffs.
fn standing_fact(page: &BriefPage, theme: &Theme, cx: &Context<Detail>) -> (bool, Div) {
    let bad = |words: &str| fact(div().text_color(theme.text_1).child(words.to_owned()));
    match &page.standing {
        Standing::Current if !page.uncommitted => (
            false,
            measure("Written at", "this branch's latest commit", theme),
        ),
        Standing::Current => (
            true,
            bad("Written with uncommitted changes on top of this branch's latest commit"),
        ),
        Standing::Since { commits, files } => {
            let words = match files.len() {
                0 => format!("{} newer than this brief", plural(commits.len(), "commit")),
                n => format!(
                    "{} newer than this brief, changing {}",
                    plural(commits.len(), "commit"),
                    plural(n, "file")
                ),
            };
            let row = v_flex()
                .child(bad(&words))
                .children(commits.iter().take(5).enumerate().map(|(ix, commit)| {
                    let id = commit.id;
                    div()
                        .id(("brief-since-commit", ix))
                        .truncate()
                        .text_color(theme.text_1)
                        .hover(|el| el.underline())
                        .role(gpui::Role::Button)
                        .aria_label(SharedString::from(format!("Show commit {}", commit.title)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let Some(project) = this.selected_project() else {
                                return;
                            };
                            (this.store).update(cx, |store, cx| {
                                store.select(Some(Selection::Commit(project, id)), cx)
                            });
                        }))
                        .child(commit.title.clone())
                }));
            (true, row)
        }
        Standing::Rewritten => (
            true,
            bad("Written at a commit this branch doesn't have. It was rewritten, or isn't fetched"),
        ),
        Standing::Unknown if page.uncommitted => (
            true,
            bad("Written from uncommitted work. No commit matches it"),
        ),
        Standing::Unknown => (false, measure("Written at", "no commit here", theme)),
    }
}

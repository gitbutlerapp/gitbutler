//! The project page's overview of what landed on its target, beside the history until a commit
//! is opened: over the days picked off the activity or the last 30, only what landed the
//! commits of the person picked, and only what changed the area picked. What's under way is
//! on the Branches page.

use std::collections::{BTreeSet, HashMap, HashSet};

use gpui::{AnyElement, Context, MouseButton, Window, div, prelude::*};

use super::{Upstream, days_said, noon};
use crate::backend::CommitId;
use crate::backend::upstream::{ACTIVITY_DAYS, Landing, Overview};
use crate::detail::Detail;
use crate::detail::panes::{
    OVERVIEW_COUNT_WIDTH, OVERVIEW_TIME_WIDTH, and_more, back_to_list, clear_button, grouped,
    overview_column, overview_heading, overview_sections, overview_tiles, overview_title,
    share_row,
};
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::person::author_avatar;
use crate::ui::relative_time;
use crate::ui::{Icon, dp, faint, keep_focus_on_press, list_row, plural, small_icon, v_flex};

/// How many of the newest releases are listed.
const RELEASES_SHOWN: usize = 3;
/// How many of the areas most landed in are listed.
const AREAS_SHOWN: usize = 6;
/// How many of the largest landings are listed.
const LARGEST_SHOWN: usize = 5;

/// What of `landings` landed on `days`, or on any of the activity's while none is picked, and
/// landed commits of whoever `theirs` lists the landings of, if someone's picked.
fn landed_then<'a>(
    landings: &'a [Landing],
    days: Option<&BTreeSet<i64>>,
    theirs: Option<&HashSet<CommitId>>,
) -> Vec<&'a Landing> {
    (landings.iter())
        .filter(|l| days.is_none_or(|days| days.contains(&l.day)))
        .filter(|l| theirs.is_none_or(|theirs| theirs.contains(&l.id)))
        .collect()
}

/// How many of `landed` changed each area, most first.
fn area_counts(landed: &[&Landing]) -> Vec<(String, usize)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for area in landed.iter().flat_map(|l| &l.areas) {
        *counts.entry(area).or_default() += 1;
    }
    let mut counts: Vec<_> = (counts.into_iter())
        .map(|(area, n)| (area.to_owned(), n))
        .collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts
}

/// The middle of how long the pull requests of `landed` took to land, from their first
/// commit being written, in seconds.
fn median_took(landed: &[&Landing]) -> Option<i64> {
    let mut took: Vec<i64> = (landed.iter())
        .filter(|l| l.pull)
        .filter_map(|l| l.took)
        .collect();
    took.sort_unstable();
    took.get(took.len() / 2).copied()
}

/// How many pull requests of `kept` each release of `releases` carried: those landed since
/// the release before it, on `landings`' order, newest first; and whether that release is
/// among `landings`, or the count stops where they do, short of it.
fn carried(
    releases: &[CommitId],
    landings: &[Landing],
    kept: &HashSet<CommitId>,
) -> Vec<(usize, bool)> {
    let at: HashMap<CommitId, usize> = (landings.iter().enumerate())
        .map(|(ix, l)| (l.id, ix))
        .collect();
    let starts: Vec<usize> = releases
        .iter()
        .filter_map(|id| at.get(id).copied())
        .collect();
    (starts.iter().enumerate())
        .map(|(ix, &start)| {
            // Tags on one commit carried the same, back to the release before them all.
            let end = (starts.iter().skip(ix + 1)).find(|&&next| next != start);
            let count = (landings.iter().take(end.map_or(landings.len(), |end| *end)))
                .skip(start)
                .filter(|l| l.pull && kept.contains(&l.id))
                .count();
            (count, end.is_some())
        })
        .collect()
}

/// How long something took, said short: hours under a day, else days to one place.
fn took_said(seconds: i64) -> String {
    match seconds {
        ..3600 => "<1h".to_owned(),
        3600..86_400 => format!("{}h", seconds / 3600),
        _ => format!("{:.1}d", seconds as f64 / 86_400.),
    }
}

impl Detail {
    /// Open commit `id` of what landed lately in the history, which takes the focus, as the
    /// list it's picked in would.
    fn open_landed(
        &mut self,
        project: &ProjectId,
        id: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.side_focus, cx);
        self.pick_landed(project, id, cx);
    }

    /// Beside the history until a commit is opened: how much landed and how
    /// fast; the releases that carried it; the areas it changed, each a filter of its own;
    /// and the largest pull requests. Clicking a release or one of those opens it.
    pub(in crate::detail) fn render_shipped(
        &self,
        project: &ProjectId,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let Some((upstream, summary)) =
            (self.upstream_for(project)).and_then(|u| Some((u, u.summary.as_ref()?)))
        else {
            return div().flex_1().into_any_element();
        };
        let days = upstream.picked_days();
        let (who, area) = (upstream.view.person.as_ref(), upstream.view.area.as_ref());
        let person = upstream.picked_person();
        // A person picked who's no longer among those who landed anything matches nothing.
        let nobody = HashSet::new();
        let theirs = who.map(|_| person.map_or(&nobody, |p| &p.landed));
        let by_who = landed_then(&summary.landings, days, theirs);
        let kept: Vec<&Landing> = (by_who.iter().copied())
            .filter(|l| area.is_none_or(|area| l.areas.contains(area)))
            .collect();
        let kept_ids: HashSet<CommitId> = kept.iter().map(|l| l.id).collect();
        let people = (summary.people.iter())
            .filter(|p| !p.landed.is_disjoint(&kept_ids))
            .count();
        let tiles = [
            (
                "Pull requests",
                grouped(kept.iter().filter(|l| l.pull).count()),
                "Pull requests that landed then",
            ),
            (
                "Commits",
                grouped(kept.iter().map(|l| l.commits as usize).sum()),
                "The commits they brought in, merges left out",
            ),
            ("People", grouped(people), "Who wrote them"),
            (
                "Time to land",
                median_took(&kept).map_or("–".to_owned(), took_said),
                "The middle time from a pull request's first commit being written to it landing",
            ),
        ];
        let scope = [
            Some(
                days.and_then(days_said)
                    .unwrap_or(format!("Last {ACTIVITY_DAYS} days")),
            ),
            person.map(|p| p.name.clone()),
            area.cloned(),
        ];
        let filtered = days.is_some() || who.is_some() || area.is_some();
        let solid = self.shipped_focus.is_focused(window);
        v_flex()
            .id("shipped")
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_y_scroll()
            .track_focus(&self.shipped_focus)
            .on_mouse_down_out(back_to_list(&self.shipped_focus, cx))
            .child(
                v_flex()
                    .w_full()
                    .px(dp(14.))
                    .py(dp(16.))
                    .gap(dp(6.))
                    .child(overview_title(
                        "Shipped",
                        scope.into_iter().flatten().collect::<Vec<_>>().join(" · "),
                        theme,
                    ))
                    .child(overview_tiles(tiles, theme))
                    .when(kept.is_empty(), |el| {
                        el.child(
                            faint(
                                match filtered {
                                    true => "Nothing landed that the filters keep".to_owned(),
                                    false => {
                                        format!("Nothing landed in the last {ACTIVITY_DAYS} days")
                                    }
                                },
                                theme,
                            )
                            .px(dp(14.))
                            .py(dp(8.)),
                        )
                    })
                    // Largest's titles want the most room, so it's the first to go under.
                    .child(overview_sections([
                        (self.releases(project, upstream, &kept_ids, theme, cx), 320.),
                        (self.areas(project, &by_who, area, solid, cx), 420.),
                        (self.largest(project, summary, &kept, theme, cx), 560.),
                    ])),
            )
            .into_any_element()
    }

    /// The newest releases of those in the days picked, each with how many pull requests it
    /// carried, and how many more there were.
    fn releases(
        &self,
        project: &ProjectId,
        upstream: &Upstream,
        kept: &HashSet<CommitId>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let (summary, days) = (upstream.summary.as_ref()?, upstream.picked_days());
        let ids: Vec<CommitId> = summary.releases.iter().map(|r| r.id).collect();
        let counts = carried(&ids, &summary.landings, kept);
        let releases: Vec<_> = (summary.releases.iter().zip(counts))
            .filter(|(r, _)| days.is_none_or(|days| days.contains(&r.day)))
            .collect();
        let first = releases.first()?.0.id;
        let rest = releases.len().saturating_sub(RELEASES_SHOWN);
        let rows = (releases.iter().take(RELEASES_SHOWN).enumerate()).map(|(ix, (release, n))| {
            let (project, id) = (project.clone(), release.id);
            list_row(("release", ix), false, theme)
                .child(small_icon(Icon::Tag).text_color(match release.id == first {
                    true => theme.pop,
                    false => theme.text_3,
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(release.name.clone()),
                )
                // At least, when the release before it landed before the days read.
                .child(overview_column(
                    match n {
                        (n, true) => plural(*n, "pull request"),
                        (n, false) => format!("{n}+ pull requests"),
                    },
                    OVERVIEW_COUNT_WIDTH,
                    theme,
                ))
                .child(overview_column(
                    relative_time(noon(release.day)),
                    OVERVIEW_TIME_WIDTH,
                    theme,
                ))
                .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_landed(&project, id, window, cx)
                }))
        });
        Some(
            v_flex()
                .child(overview_heading(
                    "Releases",
                    faint(plural(releases.len(), "release"), theme),
                    theme,
                ))
                .children(rows)
                .children(and_more(rest, theme))
                .into_any_element(),
        )
    }

    /// The parts of the repository what landed changed, most first, each with a bar of how
    /// much landed there. Clicking one filters the history and the rest of this to it,
    /// shown as a selection, solid while this has the focus; Clear lets go of it.
    fn areas(
        &self,
        project: &ProjectId,
        landed: &[&Landing],
        picked: Option<&String>,
        solid: bool,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::get(cx);
        let counts = area_counts(landed);
        // While one's picked, its Clear stays, whatever's left to count.
        if counts.is_empty() && picked.is_none() {
            return None;
        }
        let most = counts.first().map_or(1, |(_, n)| (*n).max(1));
        // The one picked stays listed, however little landed there.
        let listed = |ix: usize, area: &String| ix < AREAS_SHOWN || picked == Some(area);
        let rest = (counts.iter().enumerate())
            .filter(|(ix, (area, _))| !listed(*ix, area))
            .count();
        let rows = (counts.into_iter().enumerate())
            .filter(|(ix, (area, _))| listed(*ix, area))
            .map(|(ix, (area, n))| {
                let project = project.clone();
                let label = div().flex_1().min_w_0().truncate().child(area.clone());
                let bar = div().flex_1().max_w(dp(360.));
                let on = (picked == Some(&area), solid);
                share_row(("area", ix), label, bar, (n, most), on, theme).on_click(cx.listener(
                    move |this, _, _, cx| this.filter_area(&project, Some(area.clone()), cx),
                ))
            });
        let detail = match picked {
            Some(_) => {
                let project = project.clone();
                clear_button("area-clear")
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.filter_area(&project, None, cx)),
                    )
                    .into_any_element()
            }
            None => faint("how many landed there", theme).into_any_element(),
        };
        Some(
            v_flex()
                .child(overview_heading("Where it landed", detail, theme))
                .children(rows)
                .children(and_more(rest, theme))
                .into_any_element(),
        )
    }

    /// The pull requests of `kept` that brought in the most commits, each with who and how
    /// long it took to land.
    fn largest(
        &self,
        project: &ProjectId,
        summary: &Overview,
        kept: &[&Landing],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let mut largest: Vec<&Landing> = kept.iter().copied().filter(|l| l.pull).collect();
        largest.sort_by(|a, b| b.commits.cmp(&a.commits).then(b.day.cmp(&a.day)));
        largest.first()?;
        let rows = (largest.into_iter().take(LARGEST_SHOWN).enumerate()).map(|(ix, landing)| {
            let by = (landing.by.as_ref())
                .and_then(|email| summary.people.iter().find(|p| p.email == *email));
            let (project, id) = (project.clone(), landing.id);
            list_row(("largest", ix), false, theme)
                .child(small_icon(Icon::BranchMerge).text_color(theme.purple))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(landing.title.clone()),
                )
                .children(by.map(|p| author_avatar(&p.name, &p.email, 16., theme).flex_none()))
                .child(overview_column(
                    plural(landing.commits as usize, "commit"),
                    OVERVIEW_COUNT_WIDTH,
                    theme,
                ))
                .child(overview_column(
                    landing.took.map_or(String::new(), took_said),
                    OVERVIEW_TIME_WIDTH,
                    theme,
                ))
                .on_mouse_down(MouseButton::Left, keep_focus_on_press)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_landed(&project, id, window, cx)
                }))
        });
        Some(
            v_flex()
                .child(overview_heading(
                    "Largest",
                    faint("commits · time to land", theme),
                    theme,
                ))
                .children(rows)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::{area_counts, carried, landed_then, median_took};
    use crate::backend::CommitId;
    use crate::backend::upstream::Landing;

    fn id(n: u8) -> CommitId {
        CommitId(gix::ObjectId::from_bytes_or_panic(&[n; 20]))
    }

    /// Landing `n` on `day`, a pull request when `pull`, taking `took` hours, changing `areas`.
    fn landing(n: u8, day: i64, pull: bool, took: i64, areas: &[&str]) -> Landing {
        Landing {
            id: id(n),
            title: format!("landing {n}"),
            day,
            pull,
            commits: n as u32,
            took: Some(took * 3600),
            by: None,
            areas: areas.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn sums_up_what_landed_on_the_days_and_by_the_person_picked() {
        // Newest first, as the target's line has them.
        let landings = [
            landing(1, 10, true, 2, &["crates/but", "apps/lite"]),
            landing(2, 9, true, 30, &["crates/but"]),
            landing(3, 9, false, 100, &["Top level"]),
            landing(4, 5, true, 50, &["apps/lite"]),
        ];
        let all = landed_then(&landings, None, None);
        assert_eq!(all.len(), 4, "nothing picked keeps everything");
        assert_eq!(
            area_counts(&all),
            [
                ("apps/lite".to_owned(), 2),
                ("crates/but".to_owned(), 2),
                ("Top level".to_owned(), 1)
            ],
            "landings per area, most first, then by name"
        );
        assert_eq!(
            median_took(&all),
            Some(30 * 3600),
            "the middle of the pull requests' times, the direct commit left out"
        );

        let days: BTreeSet<i64> = [9, 10].into();
        let theirs: HashSet<CommitId> = [id(2), id(3), id(4)].into();
        let picked = landed_then(&landings, Some(&days), Some(&theirs));
        let ids: Vec<_> = picked.iter().map(|l| l.id).collect();
        assert_eq!(ids, [id(2), id(3)], "on the days picked, and theirs");

        // Releases at landings 1 and 3: the newer carries 1 and 2, the older 3 and 4.
        let kept: HashSet<CommitId> = landings.iter().map(|l| l.id).collect();
        assert_eq!(
            carried(&[id(1), id(3)], &landings, &kept),
            [(2, true), (1, false)],
            "pull requests since the release before, the direct commit left out, and the \
             oldest's only back to where the landings read stop"
        );
        assert_eq!(
            carried(&[id(1), id(1), id(3)], &landings, &kept),
            [(2, true), (2, true), (1, false)],
            "two tags on one commit carried the same"
        );
        let crates: HashSet<CommitId> = [id(2)].into();
        assert_eq!(
            carried(&[id(1), id(3)], &landings, &crates),
            [(1, true), (0, false)],
            "only those the filters keep"
        );
    }
}

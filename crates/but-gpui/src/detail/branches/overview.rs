//! The Branches page's overview, beside the list until a branch is opened: what's cooking in
//! the branches the filters keep, over the last activity they pick or the last 30 days, and
//! only in the commits of the person picked, if one is. Work under way, not what landed,
//! which the project's page has.

use std::collections::{BTreeSet, HashSet};

use gpui::{AnyElement, Context, Hsla, div, prelude::*, px, relative};

use super::{Branches, Recency, email_of, title};
use crate::backend::ReviewState;
use crate::backend::branches::OtherBranch;
use crate::backend::upstream::{self, ACTIVITY_DAYS};
use crate::detail::Detail;
use crate::detail::upstream::noon;
use crate::state::{BranchStatus, ProjectId};
use crate::theme::Theme;
use crate::ui::person::author_avatar;
use crate::ui::{
    Icon, Tooltip, faint, h_flex, list_row, plural, review, section_label, small_icon, v_flex,
};

/// How many of the busiest branches get a lane.
const LANES: usize = 7;
/// How many of the others that went quiet are listed.
const QUIET_SHOWN: usize = 5;
/// How many days without a commit make a branch quiet.
const QUIET_DAYS: i64 = 7;

/// A branch with commits on the days summed up.
struct Lane {
    ix: usize,
    /// Its commits on each of the activity's days, oldest first.
    daily: Vec<u32>,
    /// Its commits on the days summed up.
    n: usize,
    /// The day of its newest commit, whenever that was.
    latest: i64,
    /// Whose those commits are, by where they are in its `people`, most first.
    people: Vec<usize>,
}

struct Summary {
    commits: usize,
    /// How many people wrote them.
    people: usize,
    /// How many of the lanes' branches had their first commit on the days summed up.
    started: usize,
    /// Every branch with commits then, the busiest first.
    lanes: Vec<Lane>,
}

/// The `kept` of `list` summed up on `days`, or the activity's last 30 up to `today` while
/// none is picked, counting only `who`'s commits when there's someone.
fn sum_up(
    list: &[OtherBranch],
    kept: &[usize],
    who: Option<&str>,
    days: &BTreeSet<i64>,
    today: i64,
) -> Summary {
    let first_day = today - (ACTIVITY_DAYS as i64 - 1);
    let summed = |day: i64| match days.is_empty() {
        true => day >= first_day,
        false => days.contains(&day),
    };
    let mut summary = Summary {
        commits: 0,
        people: 0,
        started: 0,
        lanes: Vec::new(),
    };
    let mut people = HashSet::new();
    for &ix in kept {
        let branch = &list[ix];
        // Its tip changed last, and none of its commits was written after that.
        if (branch.time).is_some_and(|time| upstream::day_of(time) < first_day) {
            continue;
        }
        let mut lane = Lane {
            ix,
            daily: vec![0; ACTIVITY_DAYS],
            n: 0,
            latest: i64::MIN,
            people: Vec::new(),
        };
        let mut theirs = vec![0; branch.people.len()];
        for commit in &branch.commits {
            // A clock ahead of this one counts as today.
            let day = upstream::day_of(commit.time).min(today);
            if who.is_some_and(|who| branch.people[commit.person] != who) {
                continue;
            }
            lane.latest = lane.latest.max(day);
            if day >= first_day {
                lane.daily[(day - first_day) as usize] += 1;
            }
            if summed(day) {
                lane.n += 1;
                theirs[commit.person] += 1;
                people.insert(&branch.people[commit.person]);
            }
        }
        if lane.n == 0 {
            continue;
        }
        summary.commits += lane.n;
        let born = (branch.commits.iter())
            .map(|commit| upstream::day_of(commit.time).min(today))
            .min();
        summary.started += usize::from(born.is_some_and(summed));
        lane.people = (0..theirs.len()).filter(|p| theirs[*p] > 0).collect();
        lane.people.sort_by_key(|p| std::cmp::Reverse(theirs[*p]));
        summary.lanes.push(lane);
    }
    summary
        .lanes
        .sort_by(|a, b| b.n.cmp(&a.n).then(b.latest.cmp(&a.latest)));
    summary.people = people.len();
    summary
}

impl Detail {
    /// Beside the list until a branch is opened: how many branches moved and how, the
    /// busiest with a lane of their days, and the others that went quiet. Clicking one
    /// opens it.
    pub(in crate::detail) fn render_branches_overview(
        &self,
        project: &ProjectId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(branches) = self.branches_for(project) else {
            return div().flex_1().into_any_element();
        };
        let today = upstream::day_of(crate::ui::now());
        let kept = self.shown_branches(branches, cx);
        let days = recent_days(branches, today);
        let summary = sum_up(&branches.list, &kept, branches.who.as_deref(), &days, today);
        // What the list keeps, by its pull request, so it reads as work under way at rest.
        let listed = match self.store.read(cx).prefs.branches_status {
            BranchStatus::NotMerged => "In progress",
            BranchStatus::Open => "Open pull requests",
            BranchStatus::Draft => "Draft pull requests",
            BranchStatus::Closed => "Closed pull requests",
            BranchStatus::NoPullRequest => "Without a pull request",
            BranchStatus::Any => "All branches",
        };
        let when = match branches.recency {
            Recency::Any => format!("Last {ACTIVITY_DAYS} days"),
            recency => recency.label().to_owned(),
        };
        let scope = [Some(when), branches.who.clone()];
        let tiles = [
            (
                "Branches moving",
                summary.lanes.len().to_string(),
                "Branches with commits then",
            ),
            ("Commits", summary.commits.to_string(), "Their commits then"),
            (
                "Started",
                summary.started.to_string(),
                "Of those, how many had their first commit then",
            ),
            ("People", summary.people.to_string(), "Who wrote them"),
        ];
        let most = (summary.lanes.iter().take(LANES))
            .flat_map(|lane| lane.daily.iter().copied())
            .max()
            .unwrap_or(1)
            .max(1);
        let lanes: Vec<_> = (summary.lanes.iter().take(LANES).enumerate())
            .map(|(ix, lane)| self.lane(ix, lane, branches, most, today, theme, cx))
            .collect();
        let quiet: Vec<_> = (summary.lanes.iter().skip(LANES))
            .filter(|lane| today - lane.latest >= QUIET_DAYS)
            .filter(|lane| {
                let review = branches.list[lane.ix].review.as_ref();
                review.is_none_or(|r| matches!(r.state, ReviewState::Open | ReviewState::Draft))
            })
            .take(QUIET_SHOWN)
            .enumerate()
            .map(|(ix, lane)| self.quiet_row(ix, lane, branches, today, theme, cx))
            .collect();
        let quiet_shown = !quiet.is_empty();
        let nothing = match days.is_empty() {
            true => format!("No commits in the last {ACTIVITY_DAYS} days"),
            false => "No commits then".to_owned(),
        };
        v_flex()
            .id("branches-overview")
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(820.))
                    .px(px(14.))
                    .py(px(16.))
                    .gap(px(6.))
                    .child(
                        h_flex()
                            .px(px(14.))
                            .pb(px(4.))
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(15.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(listed),
                            )
                            .child(faint(
                                scope.into_iter().flatten().collect::<Vec<_>>().join(" · "),
                                theme,
                            )),
                    )
                    .child(h_flex().gap(px(8.)).px(px(6.)).children(
                        tiles.into_iter().enumerate().map(|(ix, (label, n, tip))| {
                            v_flex()
                                .id(("overview-tile", ix))
                                .flex_1()
                                .min_w(px(0.))
                                .px(px(12.))
                                .py(px(10.))
                                .gap(px(2.))
                                .border_1()
                                .border_color(theme.border_3)
                                .rounded(px(8.))
                                .tooltip(Tooltip::text(tip))
                                .child(faint(label, theme).truncate())
                                .child(
                                    div()
                                        .text_size(px(22.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(n),
                                )
                        }),
                    ))
                    .children(self.review_split(&summary, branches, theme))
                    .child(heading("Cooking", "commits a day", theme).mt(px(14.)))
                    .when(lanes.is_empty(), |el| {
                        el.child(faint(nothing, theme).px(px(14.)).py(px(4.)))
                    })
                    .children(lanes)
                    .when(quiet_shown, |el| {
                        el.child(heading("Gone quiet", "no commits for a week", theme).mt(px(14.)))
                    })
                    .children(quiet),
            )
            .into_any_element()
    }

    /// The moving branches by their pull request, as a bar of each kind's share over what
    /// each is and how many.
    fn review_split(
        &self,
        summary: &Summary,
        branches: &Branches,
        theme: &Theme,
    ) -> Option<AnyElement> {
        let kinds = [
            Some(ReviewState::Open),
            Some(ReviewState::Draft),
            None,
            Some(ReviewState::Closed),
            Some(ReviewState::Merged),
        ];
        let split: Vec<(Icon, Hsla, String, usize)> = (kinds.into_iter())
            .map(|kind| {
                let n = (summary.lanes.iter())
                    .filter(|lane| branches.list[lane.ix].review.as_ref().map(|r| r.state) == kind)
                    .count();
                let (glyph, color, said) = match kind {
                    Some(state) => {
                        let (glyph, color) = review::state_glyph(state, theme);
                        let said = match state {
                            ReviewState::Open => "open",
                            ReviewState::Draft => "draft",
                            ReviewState::Closed => "closed",
                            ReviewState::Merged => "merged",
                        };
                        (glyph, color, said)
                    }
                    None => (Icon::Branch, theme.text_3, "no pull request"),
                };
                (glyph, color, format!("{n} {said}"), n)
            })
            .filter(|(.., n)| *n > 0)
            .collect();
        if split.is_empty() {
            return None;
        }
        // Draft and none share a gray, a step apart, so the bar reads as how far along.
        let fill = |glyph: Icon, color: Hsla| match glyph {
            Icon::Branch => theme.border_1,
            _ => color,
        };
        Some(
            v_flex()
                .px(px(6.))
                .pt(px(8.))
                .gap(px(6.))
                .child(h_flex().h(px(6.)).gap(px(2.)).children(split.iter().map(
                    |(glyph, color, _, n)| {
                        div()
                            .h_full()
                            .flex_grow(*n as f32)
                            .flex_basis(px(0.))
                            .rounded(px(2.))
                            .bg(fill(*glyph, *color))
                    },
                )))
                .child(
                    h_flex()
                        .gap(px(12.))
                        .flex_wrap()
                        .children(split.into_iter().map(|(glyph, color, said, _)| {
                            h_flex()
                                .gap(px(4.))
                                .child(small_icon(glyph).text_color(color))
                                .child(faint(said, theme))
                        })),
                )
                .into_any_element(),
        )
    }

    /// A busy branch: its pull request's state, its title, whose commits they are, a cell
    /// for each of the activity's days shaded by how many it had (the days not picked
    /// dimmed), and how many in all.
    #[allow(clippy::too_many_arguments)]
    fn lane(
        &self,
        ix: usize,
        lane: &Lane,
        branches: &Branches,
        most: u32,
        today: i64,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let branch = &branches.list[lane.ix];
        let days = recent_days(branches, today);
        let first_day = today - (ACTIVITY_DAYS as i64 - 1);
        let cells = lane.daily.iter().enumerate().map(|(day_ix, &n)| {
            let day = first_day + day_ix as i64;
            let dim = !days.is_empty() && !days.contains(&day);
            let shade = (n as f32 / most as f32).sqrt();
            let color = match n {
                0 => theme.border_3.opacity(0.6),
                _ => theme.pop.opacity(0.3 + 0.7 * shade),
            };
            let tip = format!(
                "{}: {}",
                crate::ui::local_weekday(noon(day)),
                plural(n as usize, "commit")
            );
            div()
                .id(("lane-day", day_ix))
                .flex_1()
                .h(px(14.))
                .rounded(px(2.))
                .bg(if dim { color.opacity(0.3) } else { color })
                .tooltip(Tooltip::text(tip))
        });
        branch_row(("cooking-lane", ix), branch, lane, branches, theme, cx)
            .child(
                h_flex()
                    .flex_none()
                    .w(relative(0.45))
                    .gap(px(2.))
                    .children(cells),
            )
            .child(
                faint(lane.n.to_string(), theme)
                    .flex_none()
                    .w(px(26.))
                    .text_right(),
            )
            .into_any_element()
    }

    /// A branch that went quiet: how many commits it had then, and for how long it's had none.
    fn quiet_row(
        &self,
        ix: usize,
        lane: &Lane,
        branches: &Branches,
        today: i64,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let branch = &branches.list[lane.ix];
        branch_row(("quiet-branch", ix), branch, lane, branches, theme, cx)
            .child(faint(plural(lane.n, "commit"), theme).flex_none())
            .child(
                div()
                    .flex_none()
                    .w(px(90.))
                    .text_right()
                    .text_size(px(11.))
                    .text_color(theme.warn)
                    .child(format!("quiet {} days", today - lane.latest)),
            )
            .into_any_element()
    }
}

/// A row naming `branch`, as the list does, with whose its commits are; clicking it opens it.
fn branch_row(
    id: (&'static str, usize),
    branch: &OtherBranch,
    lane: &Lane,
    branches: &Branches,
    theme: &Theme,
    cx: &Context<Detail>,
) -> gpui::Stateful<gpui::Div> {
    let (glyph, color) = match &branch.review {
        Some(review) => review::state_glyph(review.state, theme),
        None => (Icon::Branch, theme.text_2),
    };
    let avatars = lane.people.iter().take(3).enumerate().map(|(at, person)| {
        let name = &branch.people[*person];
        author_avatar(name, &email_of(&branches.list, name), 16., theme)
            .flex_none()
            .when(at > 0, |el| el.ml(px(-4.)))
    });
    let ref_name = branch.ref_name.clone();
    list_row(id, false, theme)
        .h_auto()
        .py(px(4.))
        .child(small_icon(glyph).text_color(color))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .child(title(branch).to_owned()),
        )
        .child(h_flex().flex_none().w(px(40.)).children(avatars))
        .on_click(
            cx.listener(move |this, _, _, cx| this.select_other_branch(Some(ref_name.clone()), cx)),
        )
}

/// The days the list's last activity reaches back over, up to `today`; none, which
/// [`sum_up`] takes as the last 30, for any time.
fn recent_days(branches: &Branches, today: i64) -> BTreeSet<i64> {
    (branches.recency.days())
        .map(|n| (today - n + 1..=today).collect())
        .unwrap_or_default()
}

/// A section's heading: `title`, and what its numbers count.
fn heading(title: &'static str, detail: &'static str, theme: &Theme) -> gpui::Div {
    h_flex()
        .px(px(14.))
        .pb(px(2.))
        .gap(px(8.))
        .child(section_label(title, theme).flex_grow(1.))
        .child(faint(detail, theme))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::sum_up;
    use crate::backend::CommitId;
    use crate::backend::branches::{OtherBranch, OwnCommit};
    use crate::detail::upstream::noon;

    const TODAY: i64 = 20_000;

    /// A branch with `people`, whose own commits are `(days ago, person)`, newest first.
    fn branch(name: &str, people: &[&str], commits: &[(i64, usize)]) -> OtherBranch {
        let commits: Vec<_> = (commits.iter())
            .map(|&(ago, person)| OwnCommit {
                time: noon(TODAY - ago),
                person,
            })
            .collect();
        OtherBranch {
            ref_name: format!("refs/heads/{name}"),
            name: name.to_owned(),
            local: true,
            author: None,
            people: people.iter().map(|p| p.to_string()).collect(),
            email: None,
            agent: None,
            time: commits.first().map(|c| c.time),
            ahead: None,
            own_commits: Some(commits.len()),
            commits,
            stack: 0,
            tip: CommitId(gix::ObjectId::null(gix::hash::Kind::Sha1)),
            review: None,
            description: None,
        }
    }

    #[test]
    fn sums_up_only_the_days_and_the_person_picked() {
        let list = [
            branch("a", &["Ann", "Bob"], &[(0, 0), (1, 0), (3, 1), (40, 0)]),
            branch("b", &["Bob"], &[(10, 0), (12, 0)]),
            branch("c", &["Cat"], &[(50, 0)]),
        ];
        let kept = [0, 1, 2];
        let lanes = |summary: &super::Summary| -> Vec<(usize, usize)> {
            summary.lanes.iter().map(|l| (l.ix, l.n)).collect()
        };

        let all = sum_up(&list, &kept, None, &BTreeSet::new(), TODAY);
        assert_eq!(
            lanes(&all),
            [(0, 3), (1, 2)],
            "the last 30 days' commits, the busiest first, and none of c's from before"
        );
        assert_eq!(
            (all.commits, all.people, all.started),
            (5, 2, 1),
            "Ann and Bob, and not Cat; a started before the window, b in it"
        );
        assert_eq!(
            all.lanes[0].daily[26..],
            [1, 0, 1, 1],
            "a's lane has a cell for each day"
        );
        assert_eq!(
            all.lanes[0].people,
            [0, 1],
            "Ann's two commits, then Bob's one"
        );

        let bob = sum_up(&list, &kept, Some("Bob"), &BTreeSet::new(), TODAY);
        assert_eq!(lanes(&bob), [(1, 2), (0, 1)], "only Bob's commits count");
        assert_eq!(bob.lanes[1].latest, TODAY - 3, "his last on a, not Ann's");

        let two: BTreeSet<i64> = (TODAY - 1..=TODAY).collect();
        let picked = sum_up(&list, &kept, None, &two, TODAY);
        assert_eq!(lanes(&picked), [(0, 2)], "b had nothing on those two days");
        assert_eq!(
            picked.lanes[0].daily.iter().sum::<u32>(),
            3,
            "a's lane keeps Bob's commit of three days ago, on a day not picked"
        );
    }
}

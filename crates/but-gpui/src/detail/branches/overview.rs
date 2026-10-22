//! The Branches page's overview, beside the list until a branch is opened: the work under way
//! in the branches the filters keep, over the last activity they pick or the last 30 days,
//! counting only the commits of the person picked, if one is. What landed is on the project's
//! page.

use std::collections::HashSet;

use gpui::{AnyElement, Context, Hsla, div, prelude::*, px};

use super::{Branches, Recency, email_of, title};
use crate::backend::ReviewState;
use crate::backend::branches::OtherBranch;
use crate::backend::upstream::{self, ACTIVITY_DAYS};
use crate::detail::Detail;
use crate::detail::panes::{
    OVERVIEW_TIME_WIDTH, grouped, overview_heading, overview_tiles, overview_title,
};
use crate::detail::upstream::noon;
use crate::state::{BranchStatus, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::person::author_avatar;
use crate::ui::{Icon, Tooltip, faint, h_flex, list_row, plural, review, small_icon, v_flex};

/// How many of the busiest branches get a lane.
const LANES: usize = 7;
/// How many of the others that went quiet are listed.
const QUIET_SHOWN: usize = 5;
/// How many days without a commit make a branch quiet.
const QUIET_DAYS: i64 = 7;

/// A branch with commits on the days summed up.
struct Lane<'a> {
    branch: &'a OtherBranch,
    /// Its commits on each of the activity's days, oldest first.
    daily: Vec<u32>,
    /// Its commits on the days summed up.
    n: usize,
    /// The day of its newest commit, whenever that was.
    latest: i64,
    /// Whose those commits are, by where they are in its `people`, most first.
    people: Vec<usize>,
}

struct Summary<'a> {
    commits: usize,
    /// How many people wrote them.
    people: usize,
    /// How many of the lanes' branches had their first commit on the days summed up.
    started: usize,
    /// Every branch with commits then, the busiest first.
    lanes: Vec<Lane<'a>>,
}

/// The `kept` branches summed up from day `since` to `today`, counting only `who`'s commits
/// when there's someone; their lanes keep the activity's last 30 days.
fn sum_up<'a>(kept: &[&'a OtherBranch], who: Option<&str>, since: i64, today: i64) -> Summary<'a> {
    let first_day = today - (ACTIVITY_DAYS as i64 - 1);
    let summed = |day: i64| day >= since;
    let mut summary = Summary {
        commits: 0,
        people: 0,
        started: 0,
        lanes: Vec::new(),
    };
    let mut people = HashSet::new();
    for &branch in kept {
        let mut lane = Lane {
            branch,
            daily: vec![0; ACTIVITY_DAYS],
            n: 0,
            latest: i64::MIN,
            people: Vec::new(),
        };
        let mut theirs = vec![0; branch.people.len()];
        for commit in &branch.commits {
            // A clock ahead of this one counts as today.
            let day = commit.day.min(today);
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
            .map(|commit| commit.day.min(today))
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
    /// Beside the list until a branch is opened: how many branches moved and how; those
    /// still moving, with a commit in the last week, the busiest first, each a lane of its
    /// days; and those that went quiet since. Clicking one opens it.
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
        // The applied ones too, as the filters keep them, as they're under way as much.
        let shown = self.shown_branches(branches, cx).into_iter();
        let applied = (self.kept_in(branches, &branches.applied, None, cx)).into_iter();
        let kept: Vec<&OtherBranch> = (shown.map(|ix| &branches.list[ix]))
            .chain(applied.map(|ix| &branches.applied[ix]))
            .collect();
        let since = branches.recency.since(today);
        let summary = sum_up(&kept, branches.who.as_deref(), since, today);
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
                "Branches",
                grouped(summary.lanes.len()),
                "Branches with commits then",
            ),
            ("Commits", grouped(summary.commits), "Their commits then"),
            (
                "Started",
                grouped(summary.started),
                "Of those, how many had their first commit then",
            ),
            ("People", grouped(summary.people), "Who wrote them"),
        ];
        // Each branch that moved is still cooking, with a commit in the last week, or has
        // gone quiet since; one whose pull request merged or closed is done, not quiet.
        let (cooking, quiet): (Vec<&Lane>, Vec<&Lane>) =
            (summary.lanes.iter()).partition(|lane| today - lane.latest < QUIET_DAYS);
        let quiet = quiet.into_iter().filter(|lane| {
            let review = lane.branch.review.as_ref();
            review.is_none_or(|r| matches!(r.state, ReviewState::Open | ReviewState::Draft))
        });
        let shown: Vec<_> = (cooking.iter().copied().take(LANES))
            .map(|lane| ("cooking-lane", lane))
            .chain(quiet.take(QUIET_SHOWN).map(|lane| ("quiet-lane", lane)))
            .collect();
        let most = (shown.iter())
            .flat_map(|(_, lane)| lane.daily.iter().copied())
            .max()
            .unwrap_or(1)
            .max(1);
        let lanes = |section: &str| -> Vec<AnyElement> {
            (shown.iter().filter(|(id, _)| *id == section).enumerate())
                .map(|(ix, (id, lane))| lane_row((id, ix), lane, branches, most, today, theme, cx))
                .collect()
        };
        let (cooking_lanes, quiet_lanes) = (lanes("cooking-lane"), lanes("quiet-lane"));
        let narrowed = self.menu_narrows(branches, cx)
            || !self.branch_filter.read(cx).text().trim().is_empty();
        let nothing = match (kept.is_empty() && narrowed, summary.lanes.is_empty()) {
            (true, _) => "The filters hide every branch".to_owned(),
            (false, true) => match branches.recency.days() {
                1 => "No commits today".to_owned(),
                days => format!("No commits in the last {days} days"),
            },
            (false, false) => "No commits in the last week".to_owned(),
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
                    .child(overview_title(
                        listed,
                        scope.into_iter().flatten().collect::<Vec<_>>().join(" · "),
                        theme,
                    ))
                    .child(overview_tiles(tiles, theme))
                    .children(review_split(&summary, theme))
                    .child(overview_heading(
                        "Cooking",
                        faint("commits a day", theme),
                        theme,
                    ))
                    .when(cooking_lanes.is_empty(), |el| {
                        el.child(faint(nothing, theme).px(px(14.)).py(px(4.)))
                    })
                    .children(cooking_lanes)
                    .when(!quiet_lanes.is_empty(), |el| {
                        el.child(overview_heading(
                            "Gone quiet",
                            faint("no commits for a week", theme),
                            theme,
                        ))
                    })
                    .children(quiet_lanes),
            )
            .into_any_element()
    }
}

/// The moving branches by their pull request, as a bar of each kind's share over what each
/// is and how many.
fn review_split(summary: &Summary, theme: &Theme) -> Option<AnyElement> {
    let kinds = [
        Some(ReviewState::Open),
        Some(ReviewState::Draft),
        None,
        Some(ReviewState::Closed),
        Some(ReviewState::Merged),
    ];
    // Draft and none share a gray, a step apart, so the bar reads as how far along.
    let split: Vec<(Icon, Hsla, Hsla, String, usize)> = (kinds.into_iter())
        .map(|kind| {
            let n = (summary.lanes.iter())
                .filter(|lane| lane.branch.review.as_ref().map(|r| r.state) == kind)
                .count();
            let (glyph, color, fill, said) = match kind {
                Some(state) => {
                    let (glyph, color) = review::state_glyph(state, theme);
                    (
                        glyph,
                        color,
                        color,
                        review::state_label(state).to_lowercase(),
                    )
                }
                None => (
                    Icon::Branch,
                    theme.text_3,
                    theme.border_1,
                    "no pull request".into(),
                ),
            };
            (glyph, color, fill, format!("{n} {said}"), n)
        })
        .filter(|(.., n)| *n > 0)
        .collect();
    if split.is_empty() {
        return None;
    }
    Some(
        v_flex()
            .px(px(6.))
            .pt(px(8.))
            .gap(px(6.))
            .child(
                h_flex()
                    .h(px(6.))
                    .gap(px(2.))
                    .children(split.iter().map(|(.., fill, _, n)| {
                        div()
                            .h_full()
                            .flex_grow(*n as f32)
                            .flex_basis(px(0.))
                            .rounded(px(2.))
                            .bg(*fill)
                    })),
            )
            .child(
                h_flex()
                    .gap(px(12.))
                    .flex_wrap()
                    .children(split.into_iter().map(|(glyph, color, _, said, _)| {
                        h_flex()
                            .gap(px(4.))
                            .child(small_icon(glyph).text_color(color))
                            .child(faint(said, theme))
                    })),
            )
            .into_any_element(),
    )
}

/// A branch that moved: its pull request's state, its title, that it's applied and how
/// long it's been quiet when it is or has, whose commits they are, a cell for each of
/// the activity's days shaded by how many it had (those outside the last activity picked
/// dimmed), and how many in those days. Clicking it opens it, an applied one in the navigator.
fn lane_row(
    id: (&'static str, usize),
    lane: &Lane,
    branches: &Branches,
    most: u32,
    today: i64,
    theme: &Theme,
    cx: &Context<Detail>,
) -> AnyElement {
    let branch = lane.branch;
    let (glyph, color) = match &branch.review {
        Some(review) => review::state_glyph(review.state, theme),
        None => (Icon::Branch, theme.text_2),
    };
    let quiet = Some(today - lane.latest).filter(|days| *days >= QUIET_DAYS);
    let since = branches.recency.since(today);
    let avatars = lane.people.iter().take(3).enumerate().map(|(at, person)| {
        let name = &branch.people[*person];
        author_avatar(name, &email_of(branches, name), 16., theme)
            .flex_none()
            .when(at > 0, |el| el.ml(px(-4.)))
    });
    let first_day = today - (ACTIVITY_DAYS as i64 - 1);
    let cells = lane.daily.iter().enumerate().map(|(day_ix, &n)| {
        let day = first_day + day_ix as i64;
        let dim = day < since;
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
    let ref_name = branch.ref_name.clone();
    let applied =
        (branch.applied).then(|| Selection::Branch(branches.project.clone(), ref_name.clone()));
    // The title and the cells share the room, the cells no wider than a cell a day needs.
    list_row(id, false, theme)
        .h_auto()
        .py(px(4.))
        .child(small_icon(glyph).text_color(color))
        .child(
            h_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(6.))
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .child(title(branch).to_owned()),
                )
                .when(branch.applied, |el| {
                    el.child(faint("applied", theme).flex_none())
                })
                // Short, as the list's ages are, to leave the title room.
                .children(quiet.map(|days| {
                    div()
                        .id("quiet-days")
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(theme.warn)
                        .tooltip(Tooltip::text(format!("No commits for {days} days")))
                        .child(format!("{days}d"))
                })),
        )
        .child(h_flex().flex_none().w(px(40.)).children(avatars))
        .child(
            h_flex()
                .flex_1()
                .max_w(px(360.))
                .gap(px(2.))
                .children(cells),
        )
        .child(
            faint(grouped(lane.n), theme)
                .flex_none()
                .w(px(OVERVIEW_TIME_WIDTH))
                .text_right(),
        )
        .on_click(cx.listener(move |this, _, window, cx| match &applied {
            Some(selection) => {
                (this.app)
                    .update(cx, |app, cx| app.select(selection.clone(), window, cx))
                    .ok();
            }
            None => this.select_other_branch(Some(ref_name.clone()), cx),
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::sum_up;
    use crate::backend::CommitId;
    use crate::backend::branches::{OtherBranch, OwnCommit};
    use crate::detail::upstream::noon;

    const TODAY: i64 = 20_000;

    /// A branch with `people`, whose own commits are `(days ago, person)`, newest first.
    fn branch(name: &str, people: &[&str], commits: &[(i64, usize)]) -> OtherBranch {
        let commits: Vec<_> = (commits.iter())
            .map(|&(ago, person)| OwnCommit {
                day: TODAY - ago,
                person,
            })
            .collect();
        OtherBranch {
            ref_name: format!("refs/heads/{name}"),
            name: name.to_owned(),
            local: true,
            applied: false,
            author: None,
            people: people.iter().map(|p| p.to_string()).collect(),
            email: None,
            agent: None,
            time: commits.first().map(|c| noon(c.day)),
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
        let kept: Vec<&OtherBranch> = list.iter().collect();
        let lanes = |summary: &super::Summary| -> Vec<(String, usize)> {
            (summary.lanes.iter())
                .map(|l| (l.branch.name.clone(), l.n))
                .collect()
        };

        let all = sum_up(&kept, None, TODAY - 29, TODAY);
        assert_eq!(
            lanes(&all),
            [("a".into(), 3), ("b".into(), 2)],
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

        let bob = sum_up(&kept, Some("Bob"), TODAY - 29, TODAY);
        assert_eq!(
            lanes(&bob),
            [("b".into(), 2), ("a".into(), 1)],
            "only Bob's commits count"
        );
        assert_eq!(bob.lanes[1].latest, TODAY - 3, "his last on a, not Ann's");

        let picked = sum_up(&kept, None, TODAY - 1, TODAY);
        assert_eq!(
            lanes(&picked),
            [("a".into(), 2)],
            "b had nothing on those two days"
        );
        assert_eq!(
            picked.lanes[0].daily.iter().sum::<u32>(),
            3,
            "a's lane keeps Bob's commit of three days ago, on a day not picked"
        );
    }
}

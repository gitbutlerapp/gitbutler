//! A glyph and a short value in a row's meta, as but-ui-react's `MetaCount`, and where something
//! that runs has got to, as its `MetaStatus`.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Div, Hsla, SharedString, Transformation, canvas, div,
    percentage, prelude::*,
};

use crate::theme::Theme;
use crate::*;

/// What a [`meta_count`] counts, which sets its glyph and its colour, as React's `MetaCountType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaCountType {
    /// Commits on the target the branch doesn't have: waiting on you, in amber.
    Behind,
    /// How many of a branch's commits are conflicted: waiting on you, in amber.
    Conflicts,
    /// Commits to send, in blue.
    Unpushed,
    /// Pushed commits rewritten since, the unpushed arrow in grey.
    Rebased,
    Commits,
    /// A branch with no commits yet, quietest of all.
    Empty,
    /// What a worktree with nothing to list is on, quietest of all.
    Head,
    /// Uncommitted changes: waiting on you, in amber.
    Uncommitted,
    Clean,
    Age,
    Branch,
    Worktrees,
    Repos,
    /// An agent at work: it needs nothing from you, so quiet.
    AgentWorking,
    /// An agent waiting for you, in amber.
    AgentWaiting,
    /// Any other program at work in a terminal, as the terminal's dots spinner.
    Running,
    /// Where a branch sits in its stack, "1/3 in stack".
    Stack,
    /// A process's CPU use while it stays low, in green.
    Cpu,
    /// The same use once it runs high, in red.
    CpuDanger,
}

impl MetaCountType {
    fn glyph(self) -> Icon {
        match self {
            Self::Behind => Icon::ArrowDown,
            Self::Conflicts => Icon::Warning,
            Self::Unpushed | Self::Rebased => Icon::ArrowUp,
            Self::Commits | Self::Empty => Icon::Commit,
            Self::Head => Icon::Anchor,
            Self::Branch => Icon::Branch,
            Self::Uncommitted | Self::Clean => Icon::Diff,
            Self::Age => Icon::Clock,
            Self::Worktrees => Icon::Folder,
            Self::Repos => Icon::Repo,
            Self::AgentWorking | Self::AgentWaiting => Icon::Robot,
            Self::Running => Icon::Spinner,
            Self::Stack => Icon::Stack,
            Self::Cpu => Icon::Cpu,
            Self::CpuDanger => Icon::CpuDanger,
        }
    }

    /// Its colour on a row selected on the solid fill: its token's other scheme's value, as
    /// React's flips the scheme there.
    fn color_on_solid(self, theme: &Theme) -> Hsla {
        let flipped = &theme.on_solid;
        match self {
            Self::Behind | Self::Uncommitted | Self::Conflicts | Self::AgentWaiting => {
                flipped.warning_text
            }
            Self::AgentWorking | Self::Empty | Self::Head => flipped.subtle_foreground,
            Self::Unpushed => flipped.info_text,
            Self::Cpu => flipped.success_text,
            Self::CpuDanger => flipped.danger_text,
            Self::Rebased
            | Self::Commits
            | Self::Clean
            | Self::Age
            | Self::Branch
            | Self::Worktrees
            | Self::Repos
            | Self::Running
            | Self::Stack => flipped.muted_foreground,
        }
    }

    fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Behind | Self::Uncommitted | Self::Conflicts | Self::AgentWaiting => {
                theme.warning_text
            }
            Self::AgentWorking | Self::Empty | Self::Head => theme.subtle_foreground,
            Self::Unpushed => theme.info_text,
            Self::Cpu => theme.success_text,
            Self::CpuDanger => theme.danger_text,
            Self::Rebased
            | Self::Commits
            | Self::Clean
            | Self::Age
            | Self::Branch
            | Self::Worktrees
            | Self::Repos
            | Self::Running
            | Self::Stack => theme.muted_foreground,
        }
    }
}

/// A count's box: 20px tall with 3px at its sides, so counts sit flush in a row, 6px apart from
/// glyph to glyph, and a host puts no gap between them. The value gives way first.
fn count_box(color: Hsla, glyph: AnyElement, value: Option<SharedString>) -> Div {
    h_flex()
        .flex_none()
        .min_w(dp(20.))
        .min_h(dp(20.))
        .px(dp(3.))
        .py(dp(2.))
        .gap(dp(4.))
        .text_13()
        .line_height(dp(16.))
        .text_color(color)
        .child(glyph)
        .children(value.map(|value| div().min_w_0().truncate().child(value)))
}

/// A glyph and a short value in a row's meta, as React's `MetaCount`: a count, an age, a branch's
/// name, the type setting both glyph and colour. The value is the host's to format (`12`, `3d`).
/// The glyph says nothing to a screen reader: give the count an id and a tooltip saying what it
/// counts. A count is never pressed; the step that acts on it is a small button beside it.
///
/// `AgentWorking` draws the robot still, where React's waves its eyes: a gap until the robot's
/// motion is drawn here.
pub fn meta_count(kind: MetaCountType, value: Option<SharedString>, theme: &Theme) -> Div {
    meta_count_on(kind, value, false, theme)
}

/// [`meta_count`] in a row selected on the solid fill, with `solid`: in its token's other
/// scheme's value, as React's flips its scheme there, so it stays legible and keeps its hue.
pub fn meta_count_on(
    kind: MetaCountType,
    value: Option<SharedString>,
    solid: bool,
    theme: &Theme,
) -> Div {
    counted(kind, value, solid, true, theme)
}

/// A program at work, as [`MetaCountType::Running`] draws it, its dots stepping round only
/// while `turning`, as while its window shows, so a hidden one costs nothing.
pub fn running_count(turning: bool, solid: bool, theme: &Theme) -> Div {
    counted(MetaCountType::Running, None, solid, turning, theme)
}

fn counted(
    kind: MetaCountType,
    value: Option<SharedString>,
    solid: bool,
    turning: bool,
    theme: &Theme,
) -> Div {
    let color = match solid {
        true => kind.color_on_solid(theme),
        false => kind.color(theme),
    };
    let glyph = match kind {
        // The terminal's dots, in a glyph's 14px box.
        MetaCountType::Running => div()
            .flex_none()
            .size(dp(14.))
            .flex()
            .items_center()
            .justify_center()
            .child(spinning(color, turning))
            .into_any_element(),
        // A step quieter than its value, as 🍑 But UI draws it, so the value reads first.
        _ => icon_at(kind.glyph(), 14.)
            .flex_none()
            .text_color(color)
            .opacity(0.7)
            .into_any_element(),
    };
    count_box(color, glyph, value).when(
        matches!(
            kind,
            MetaCountType::AgentWorking | MetaCountType::AgentWaiting
        ),
        // The robot fills its 14px edge to edge where the other glyphs leave air around them, so
        // it keeps 1px more from the count before it.
        |el| el.ml(dp(1.)),
    )
}

/// Where the thing a [`meta_status`] names has got to, as React's `MetaStatusVariant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaStatusVariant {
    /// Under way, as checks that are running: the spinner, in amber.
    Working,
    /// Done and fine: a tick, in green.
    Passed,
    /// Done and broken: a cross, in red.
    Failed,
    /// Stopped until you act: a warning, in amber.
    Warning,
    /// No result yet: the dashed ring, in grey.
    Neutral,
}

/// Where something that runs has got to, as React's `MetaStatus`: a glyph and an optional short
/// label naming the thing ("CI"), in its state's colour. Without a label it's the glyph alone;
/// give it an id and a tooltip that names both then, "Checks running".
pub fn meta_status(variant: MetaStatusVariant, label: Option<SharedString>, theme: &Theme) -> Div {
    let (glyph, color) = match variant {
        MetaStatusVariant::Working => (Icon::Spinner, theme.warning_text),
        MetaStatusVariant::Passed => (Icon::Tick, theme.success_text),
        MetaStatusVariant::Failed => (Icon::Cross, theme.danger_text),
        MetaStatusVariant::Warning => (Icon::Warning, theme.warning_text),
        MetaStatusVariant::Neutral => (Icon::Pending, theme.muted_foreground),
    };
    // A step quieter than its label, as a MetaCount's glyph is.
    let glyph = match variant {
        MetaStatusVariant::Working => div()
            .flex_none()
            .opacity(0.7)
            .child(spinner(14., color))
            .into_any_element(),
        _ => icon_at(glyph, 14.)
            .flex_none()
            .text_color(color)
            .opacity(0.7)
            .into_any_element(),
    };
    count_box(color, glyph, label)
}

/// The spinner glyph at `px`, turning once every 0.8s, as React's `Icon name="spinner"` does: an
/// action of the user's own under way, or a check that's running.
pub fn spinner(px: f32, color: Hsla) -> impl IntoElement {
    icon_at(Icon::Spinner, px)
        .flex_none()
        .text_color(color)
        .with_animation(
            "spinner",
            Animation::new(Duration::from_millis(800)).repeat(),
            |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
        )
}

const SPIN_STEP: Duration = Duration::from_millis(100);
/// The dots each step of [`spinning`] shows, as braille's: the low three bits are the left
/// column's, top down, the next three the right's. The terminal's dots spinner, ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏.
const SPIN_FRAMES: [u8; 10] = [0x0b, 0x19, 0x39, 0x38, 0x3c, 0x34, 0x26, 0x27, 0x07, 0x0f];
/// A dot of [`spinning`], and how far apart their centres are, in a `small_icon`'s box.
const SPIN_DOT: f32 = 2.;
const SPIN_PITCH: f32 = 3.5;

/// Dots running round, as a terminal's spinner does, while what it stands for is under way,
/// `turning` only while it shows. It steps 10 times a second, each step drawing the view it's
/// in, and with it the whole window, again: an agent's spinner turns for minutes, and drawing
/// the window any more often kept it from going idle and WindowServer busy all that while.
/// Dots read as stepping on purpose at that rate, where a turning glyph would look choppy.
pub fn spinning(color: Hsla, turning: bool) -> impl IntoElement {
    static START: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(crate::clock::now);
    let since = crate::clock::now().saturating_duration_since(*START);
    let steps = since.as_nanos() / SPIN_STEP.as_nanos();
    #[expect(clippy::indexing_slicing, reason = "taken modulo the length")]
    let frame = SPIN_FRAMES[(steps % SPIN_FRAMES.len() as u128) as usize];
    let due = SPIN_STEP - Duration::from_nanos((since.as_nanos() % SPIN_STEP.as_nanos()) as u64);
    let left = (12. - SPIN_PITCH - SPIN_DOT) / 2.;
    let top = (12. - 2. * SPIN_PITCH - SPIN_DOT) / 2.;
    let dots = (0..6)
        .filter(|bit| frame & (1 << bit) != 0)
        .map(move |bit| {
            let (column, row) = ((bit / 3) as f32, (bit % 3) as f32);
            div()
                .absolute()
                .left(dp(left + column * SPIN_PITCH))
                .top(dp(top + row * SPIN_PITCH))
                .size(dp(SPIN_DOT))
                .rounded_full()
                .bg(color)
        });
    div()
        .relative()
        .flex_none()
        .size(dp(12.))
        .children(dots)
        // The next step asked for as this one is drawn, so a spinner that's gone, or one in a
        // view that isn't drawn, asks for no more.
        .when(turning, |el| {
            el.child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| {
                        let view = window.current_view();
                        (cx.spawn(async move |cx| {
                            cx.background_executor().timer(due).await;
                            cx.update(|cx| cx.notify(view));
                        }))
                        .detach();
                    },
                )
                .absolute(),
            )
        })
}

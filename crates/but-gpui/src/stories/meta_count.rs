use gpui::{AnyElement, Context, Div, SharedString, Window, prelude::*};

use super::{Example, example, page};
use crate::theme::Theme;
use crate::ui::{MetaCountType, MetaStatusVariant, dp, h_flex, meta_count, meta_status};

/// Every kind of count, with the value React's story gives it.
const KINDS: [(MetaCountType, &str); 19] = [
    (MetaCountType::Behind, "3"),
    (MetaCountType::Conflicts, "2"),
    (MetaCountType::Unpushed, "4"),
    (MetaCountType::Rebased, "2"),
    (MetaCountType::Commits, "5"),
    (MetaCountType::Empty, "empty"),
    (MetaCountType::Head, "detached"),
    (MetaCountType::Uncommitted, "12"),
    (MetaCountType::Clean, "0"),
    (MetaCountType::Age, "3d"),
    (MetaCountType::Branch, "retry-transfers"),
    (MetaCountType::Worktrees, "2"),
    (MetaCountType::Repos, "4"),
    (MetaCountType::AgentWorking, "12m"),
    (MetaCountType::AgentWaiting, "2m"),
    (MetaCountType::Running, "cargo build"),
    (MetaCountType::Stack, "1/3"),
    (MetaCountType::Cpu, "14%"),
    (MetaCountType::CpuDanger, "84%"),
];

const VARIANTS: [MetaStatusVariant; 5] = [
    MetaStatusVariant::Working,
    MetaStatusVariant::Passed,
    MetaStatusVariant::Failed,
    MetaStatusVariant::Warning,
    MetaStatusVariant::Neutral,
];

fn wrap(items: impl IntoIterator<Item = Div>) -> AnyElement {
    h_flex()
        .flex_wrap()
        .max_w(dp(570.))
        .gap(dp(12.))
        .children(items)
        .into_any_element()
}

fn label(text: &'static str) -> Option<SharedString> {
    Some(text.into())
}

/// The stories of `MetaCount.stories.tsx` this client can draw. `WorkUnderWay` and
/// `WithNextStep` show other components beside the counts, so they aren't here.
pub(super) const COUNT_EXAMPLES: [Example; 2] = [
    Example {
        id: "components-metacount--default",
        name: "Default",
        draw: |theme| meta_count(MetaCountType::Unpushed, label("4"), theme).into_any_element(),
    },
    Example {
        id: "components-metacount--kinds",
        name: "Kinds",
        draw: |theme| wrap(KINDS.map(|(kind, value)| meta_count(kind, label(value), theme))),
    },
];

/// The stories of `MetaStatus.stories.tsx` this client can draw. `InRowMeta` needs the
/// sidebar's rows, so it isn't here yet.
pub(super) const STATUS_EXAMPLES: [Example; 5] = [
    Example {
        id: "components-metastatus--default",
        name: "Default",
        draw: |theme| meta_status(MetaStatusVariant::Passed, label("CI"), theme).into_any_element(),
    },
    Example {
        id: "components-metastatus--variants",
        name: "Variants",
        draw: |theme| wrap(VARIANTS.map(|variant| meta_status(variant, label("CI"), theme))),
    },
    Example {
        id: "components-metastatus--labels",
        name: "Labels",
        draw: |theme| {
            wrap([
                meta_status(MetaStatusVariant::Passed, label("Deploy"), theme),
                meta_status(MetaStatusVariant::Working, label("Build"), theme),
                meta_status(MetaStatusVariant::Warning, label("Sync"), theme),
            ])
        },
    },
    Example {
        id: "components-metastatus--without-label",
        name: "Without Label",
        draw: |theme| wrap(VARIANTS.map(|variant| meta_status(variant, None, theme))),
    },
    Example {
        id: "components-metastatus--with-progress",
        name: "With Progress",
        draw: |theme| {
            wrap([
                meta_status(MetaStatusVariant::Working, label("CI 2/14"), theme),
                meta_status(MetaStatusVariant::Failed, label("CI 2/14"), theme),
                meta_status(MetaStatusVariant::Passed, label("CI"), theme),
            ])
        },
    },
];

pub struct MetaCountStory;

impl Render for MetaCountStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(COUNT_EXAMPLES.into_iter().map(|e| example(e, theme)))
    }
}

pub struct MetaStatusStory;

impl Render for MetaStatusStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        page(STATUS_EXAMPLES.into_iter().map(|e| example(e, theme)))
    }
}

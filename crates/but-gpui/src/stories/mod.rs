//! A window of the shared controls in `ui`, each in its states side by side, in light or dark
//! and at a few interface scales, as ui-react's Storybook shows its components. Built only
//! with `--features stories`, as `gitbutler-gpui-stories`. `GITBUTLER_GPUI_STORY=<name>`
//! opens it on one story, and with `dev-script` too a script drives it as it does the app
//! (`scripts/drive.sh --stories`).
//!
//! A story is a view of its own, which keeps whatever state trying its control needs. Add one
//! as a module here and to [`Gallery::new`].

mod activity;
mod button;
mod checkbox;
mod counts;
mod dropdown;
mod empty_state;
mod icons;
mod keycap;
mod popover;
mod review;
mod rows;
mod segmented;
mod switch;
mod toggle_group;

use gpui::{
    AnyElement, AnyView, App, Bounds, Context, Div, KeyBinding, Menu, MenuItem, QuitMode,
    SharedString, TitlebarOptions, Window, WindowAppearance, WindowBounds, WindowOptions, actions,
    div, point, prelude::*, size,
};

use crate::theme::{CodeSize, Theme, UiScale};
use crate::ui::{self, dp, faint, fixed, h_flex, list_row, section_label, segmented, v_flex};

actions!(stories, [Quit]);

/// Open the stories' window on this thread, the process's main one, and run until it closes.
pub fn run() {
    gpui_platform::application()
        .with_assets(ui::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            if let Err(err) = cx.text_system().add_fonts(ui::fonts()) {
                tracing::error!("couldn't load the bundled fonts: {err:#}");
            }
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
            cx.set_menus([Menu::new("Stories").items([MenuItem::action("Quit", Quit)])]);
            // As the app's: a script's run stays out of the Dock and off the screen.
            let scripted = crate::app::scripted();
            if scripted {
                cx.set_activation_policy(gpui::ActivationPolicy::Accessory);
            }
            let default_size = size(fixed(1100.), fixed(720.));
            let bounds = match scripted {
                true => Bounds::new(point(fixed(-10000.), fixed(-10000.)), default_size),
                false => Bounds::centered(None, default_size, cx),
            };
            let window = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("GitButler Stories".into()),
                        ..Default::default()
                    }),
                    focus: !scripted,
                    show: !scripted,
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Gallery::new(window, cx)),
            );
            match window {
                #[cfg(feature = "dev-script")]
                Ok(window) => crate::dev::run_from_env(window.into(), cx),
                #[cfg(not(feature = "dev-script"))]
                Ok(_) => {}
                Err(err) => {
                    tracing::error!("couldn't open the window: {err:#}");
                    cx.spawn(async |cx| cx.update(|cx| cx.quit())).detach();
                    return;
                }
            }
            if !scripted {
                cx.activate(true);
            }
        });
}

struct Story {
    name: &'static str,
    view: AnyView,
}

/// The list of stories beside the one picked, and how the theme draws them.
struct Gallery {
    stories: Vec<Story>,
    shown: usize,
    dark: bool,
    scale: UiScale,
}

/// The interface scales the toolbar offers.
const SCALES: [u16; 3] = [100, 125, 150];

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        fn story<V: Render>(name: &'static str, view: V, cx: &mut App) -> Story {
            let view = cx.new(|_| view).into();
            Story { name, view }
        }
        let stories = vec![
            story("Activity", activity::ActivityStory::default(), cx),
            story("Button", button::ButtonStory, cx),
            story("Checkbox", checkbox::CheckboxStory::default(), cx),
            story("Counts and marks", counts::CountsStory, cx),
            story("Dropdown", dropdown::DropdownStory, cx),
            story("Empty state", empty_state::EmptyStateStory, cx),
            story("Icons", icons::IconsStory, cx),
            story("Keycap", keycap::KeycapStory, cx),
            story("List rows", rows::RowsStory::default(), cx),
            story("Popover and modal", popover::PopoverStory, cx),
            story("Pull request state", review::ReviewStory, cx),
            story("Segmented", segmented::SegmentedStory::default(), cx),
            story("Switch", switch::SwitchStory::default(), cx),
            story(
                "Toggle group",
                toggle_group::ToggleGroupStory::default(),
                cx,
            ),
        ];
        // `list-rows` or `List rows`, as a shell's quoting allows.
        let asked = std::env::var("GITBUTLER_GPUI_STORY").ok();
        let shown = (asked.as_deref())
            .and_then(|asked| {
                let asked = asked.replace('-', " ");
                (stories.iter()).position(|s| s.name.eq_ignore_ascii_case(&asked))
            })
            .unwrap_or_default();
        // As the app's theme: the variable first, handy for screenshots, then the system's.
        let dark = match std::env::var("GITBUTLER_GPUI_APPEARANCE").as_deref() {
            Ok("dark") => true,
            Ok("light") => false,
            _ => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        let gallery = Self {
            stories,
            shown,
            dark,
            scale: UiScale::default(),
        };
        gallery.set_theme(cx);
        gallery
    }

    fn set_theme(&self, cx: &mut App) {
        cx.set_global(Theme::gitbutler(self.dark, CodeSize::default()).with_scale(self.scale));
    }

    fn sidebar(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        v_flex()
            .flex_none()
            .w(dp(200.))
            .h_full()
            .py(dp(12.))
            .gap(dp(2.))
            .bg(theme.bg_2)
            .border_r_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .px(dp(14.))
                    .pb(dp(6.))
                    .child(section_label("Components", theme)),
            )
            .children(self.stories.iter().enumerate().map(|(ix, story)| {
                list_row(("story", ix), ix == self.shown, theme)
                    .child(story.name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shown = ix;
                        cx.notify();
                    }))
            }))
    }

    fn toolbar(&self, name: &'static str, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let appearance = [("Light", false), ("Dark", true)].map(|(label, dark)| {
            let segment =
                div()
                    .id(label)
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dark = dark;
                        this.set_theme(cx);
                        cx.notify();
                    }));
            (segment, self.dark == dark)
        });
        let scales = (UiScale::STEPS.into_iter())
            .filter(|scale| SCALES.contains(&scale.percent()))
            .map(|scale| {
                let label = SharedString::from(format!("{}%", scale.percent()));
                let segment = div()
                    .id(("scale", usize::from(scale.percent())))
                    .child(label);
                let segment = segment.on_click(cx.listener(move |this, _, _, cx| {
                    this.scale = scale;
                    this.set_theme(cx);
                    cx.notify();
                }));
                (segment, self.scale == scale)
            });
        h_flex()
            .flex_none()
            .h(dp(48.))
            .px(dp(24.))
            .gap(dp(12.))
            .border_b_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .flex_1()
                    .text_size(dp(15.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(segmented(true, theme, scales))
            .child(segmented(true, theme, appearance))
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Every `dp` size is in rems, so this scales the whole window, as the app's does.
        window.set_rem_size(Theme::get(cx).scaled(ui::BASE_REM));
        let theme = Theme::get(cx).clone();
        let story = self.stories.get(self.shown);
        let name = story.map_or("", |story| story.name);
        h_flex()
            .size_full()
            .items_start()
            .font_family(theme.ui_font.clone())
            .text_size(dp(13.))
            .text_color(theme.text_1)
            .bg(theme.bg_1)
            .child(self.sidebar(&theme, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.toolbar(name, &theme, cx))
                    .child(
                        div()
                            .id("canvas")
                            .flex_1()
                            .overflow_y_scroll()
                            .p(dp(24.))
                            .children(story.map(|story| story.view.clone())),
                    ),
            )
    }
}

/// A story's sections, top to bottom.
fn page(sections: impl IntoIterator<Item = Div>) -> Div {
    v_flex().gap(dp(32.)).children(sections)
}

/// A titled group of a control's variants, in a row that wraps.
fn section(
    title: &'static str,
    theme: &Theme,
    variants: impl IntoIterator<Item = AnyElement>,
) -> Div {
    v_flex()
        .gap(dp(12.))
        .child(section_label(title, theme))
        .child(
            h_flex()
                .flex_wrap()
                .items_start()
                .gap_x(dp(24.))
                .gap_y(dp(16.))
                .children(variants),
        )
}

/// One variant: the control, in a row as tall as a button so captions line up, over a caption
/// naming it.
fn variant(caption: impl Into<SharedString>, theme: &Theme, el: impl IntoElement) -> AnyElement {
    framed(caption, theme, h_flex().h(dp(28.)).child(el))
}

/// One variant of something taller than a button, as a list or a card, over its caption.
fn framed(caption: impl Into<SharedString>, theme: &Theme, el: impl IntoElement) -> AnyElement {
    v_flex()
        .gap(dp(6.))
        .child(el)
        .child(faint(caption.into(), theme).text_size(dp(11.)))
        .into_any_element()
}

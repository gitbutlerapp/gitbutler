//! A window of the shared controls in `ui`, each in its states side by side, in light or dark
//! and at a few interface scales, as but-ui-react's docs site shows its components. Built only
//! with `--features stories`, as `gitbutler-gpui-pantry`. `GITBUTLER_GPUI_STORY=<name>`
//! opens it on one story, and with `dev-script` too a script drives it as it does the app
//! (`scripts/drive.sh --stories`).
//!
//! A story is a view of its own, which keeps whatever state trying its control needs. Add one
//! as a module here and to [`Gallery::new`].
//!
//! A story draws but-ui-react's stories of its component as [`Example`]s, under the docs site's
//! ids. `GITBUTLER_GPUI_STORY=<id>` opens one of them alone, the window fitted to it, which is
//! how `apps/but-ui-pantry/scripts/gpui-shots.mjs` shoots it for the pantry.

mod activity;
mod avatar;
mod badge;
mod button;
mod checkbox;
mod counts;
mod diff_stats;
mod drawer;
mod dropdown_button;
mod empty_state;
mod file_list;
mod file_status_badge;
mod icons;
mod keycap;
mod machine_status;
mod meta_count;
mod popover;
mod radio;
mod review;
mod rows;
mod segmented;
mod select;
mod sidebar_rows;
mod snackbar;
mod split_preview;
mod switch;
mod switch_button;
mod toggle_group;

use gpui::{
    AnyElement, AnyView, App, Bounds, Context, Div, Entity, Focusable as _, Hsla, KeyBinding, Menu,
    MenuItem, QuitMode, SharedString, Subscription, Task, TitlebarOptions, Window,
    WindowAppearance, WindowBounds, WindowOptions, actions, div, point, prelude::*, size,
};

use crate::backend::{
    self,
    rebuild::{Printed, Rebuild},
};
use crate::theme::{CodeSize, Density, Theme, UiScale};
use crate::ui::Text;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    self, ButtonStyle, button, dp, faint, field, fixed, h_flex, list_row, section_label, segmented,
    v_flex,
};

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
            // Escape and the rest of a field's keys, for the stories that take typing.
            ui::text_input::bind_keys(cx);
            cx.set_menus([Menu::new("Pantry").items([MenuItem::action("Quit", Quit)])]);
            // Its own icon, so the Dock tells it from the app.
            #[cfg(target_os = "macos")]
            crate::set_dock_icon(include_bytes!("../../assets/stories-icon.png"));
            // Started by `cargo run`, the toolbar offers to build the stories again.
            if let Some(rebuild) = Rebuild::detect() {
                cx.set_global(rebuild);
            }
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
                        title: Some("GPUI pantry".into()),
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

/// One of but-ui-react's stories, as this client draws it.
#[derive(Clone, Copy)]
struct Example {
    /// The story's id on the docs site: `components-button--icon-only`.
    id: &'static str,
    /// The story's name there, its export's words: `Icon Only`.
    name: &'static str,
    draw: fn(&Theme) -> AnyElement,
}

/// Every story's examples.
fn examples() -> impl Iterator<Item = Example> {
    (avatar::EXAMPLES.into_iter())
        .chain(badge::EXAMPLES)
        .chain(button::EXAMPLES)
        .chain(diff_stats::EXAMPLES)
        .chain(drawer::ITEM_EXAMPLES)
        .chain(drawer::EXAMPLES)
        .chain(file_list::EXAMPLES)
        .chain(file_status_badge::EXAMPLES)
        .chain(machine_status::EXAMPLES)
        .chain(meta_count::COUNT_EXAMPLES)
        .chain(meta_count::STATUS_EXAMPLES)
        .chain(sidebar_rows::EXAMPLES)
        .chain(sidebar_rows::TERMINAL_EXAMPLES)
        .chain(split_preview::EXAMPLES)
}

/// An example under its name, as the docs site titles it, at its own width at the start of the
/// column, as the docs site's ground places a story, rather than stretched across it.
fn example(example: Example, theme: &Theme) -> Div {
    v_flex()
        .items_start()
        .gap(dp(12.))
        .child(section_label(example.name, theme))
        .child((example.draw)(theme))
}

/// The list of stories beside the one picked, and how the theme draws them.
struct Gallery {
    stories: Vec<Story>,
    shown: usize,
    /// Narrows the list of stories to those whose name has what's typed in it.
    filter: Entity<TextInput>,
    _filter: Subscription,
    /// An example drawn alone, in place of the gallery, to be shot.
    solo: Option<Example>,
    dark: bool,
    scale: UiScale,
    density: Density,
    build: Build,
}

/// Where the toolbar's Rebuild stands.
#[derive(Default)]
enum Build {
    #[default]
    Idle,
    /// Cargo runs, its output going to the terminal that started the stories.
    Running(
        #[expect(
            dead_code,
            reason = "held so the build runs: dropping it would stop it"
        )]
        Task<()>,
    ),
    Failed,
}

const SCALES: [u16; 3] = [100, 125, 150];

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        fn story<V: Render>(name: &'static str, view: V, cx: &mut App) -> Story {
            let view = cx.new(|_| view).into();
            Story { name, view }
        }
        let stories = vec![
            story("Activity", activity::ActivityStory::default(), cx),
            story("Avatar", avatar::AvatarStory, cx),
            story("Badge", badge::BadgeStory, cx),
            story("Button", button::ButtonStory, cx),
            story("Checkbox", checkbox::CheckboxStory::default(), cx),
            story("Counts and marks", counts::CountsStory, cx),
            story("Diff stats", diff_stats::DiffStatsStory, cx),
            story("Drawer", drawer::DrawerStory, cx),
            story("Dropdown button", dropdown_button::DropdownButtonStory, cx),
            story("Empty state", empty_state::EmptyStateStory, cx),
            story("File list", file_list::FileListStory::default(), cx),
            story("File status", file_status_badge::FileStatusBadgeStory, cx),
            story("Icons", icons::IconsStory::default(), cx),
            story("Keycap", keycap::KeycapStory, cx),
            story("List rows", rows::RowsStory::default(), cx),
            story("MachineStatus", machine_status::MachineStatusStory, cx),
            story("MetaCount", meta_count::MetaCountStory, cx),
            story("MetaStatus", meta_count::MetaStatusStory, cx),
            story("Popover and modal", popover::PopoverStory, cx),
            story("Pull request state", review::ReviewStory, cx),
            story("Radio", radio::RadioStory::default(), cx),
            story("Segmented", segmented::SegmentedStory::default(), cx),
            story("Select", select::SelectStory, cx),
            story("Sidebar rows", sidebar_rows::SidebarRowsStory, cx),
            story("Snackbar", snackbar::SnackbarStory, cx),
            story("Splits", split_preview::SplitPreviewStory, cx),
            story("Switch", switch::SwitchStory::default(), cx),
            story(
                "Switch button",
                switch_button::SwitchButtonStory::default(),
                cx,
            ),
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
        let solo = (asked.as_deref()).and_then(|asked| examples().find(|e| e.id == asked));
        // As the app's theme: the variable first, handy for screenshots, then the system's.
        let dark = match std::env::var("GITBUTLER_GPUI_APPEARANCE").as_deref() {
            Ok("dark") => true,
            Ok("light") => false,
            _ => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        let filter = cx.new(|cx| TextInput::new("Filter", false, cx));
        let filter_events = cx.subscribe(&filter, |this: &mut Self, input, event, cx| {
            match event {
                InputEvent::Changed => {}
                // Esc empties it.
                InputEvent::Cancel => input.update(cx, |input, cx| input.set_text("", cx)),
                // Enter opens the first story it finds.
                InputEvent::Submit => {
                    let first = this.found(cx).next();
                    if let Some(ix) = first {
                        this.shown = ix;
                    }
                }
            }
            cx.notify();
        });
        let gallery = Self {
            build: Build::default(),
            stories,
            shown,
            filter,
            _filter: filter_events,
            solo,
            dark,
            scale: UiScale::default(),
            density: Density::default(),
        };
        gallery.set_theme(cx);
        gallery
    }

    /// Build the stories again from the checkout that built these, and reopen on the new build at
    /// the same story and appearance. Cargo's output goes to the terminal, as `cargo run`'s did; a
    /// failed build leaves this window as it was.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let Some(rebuild) = cx.try_global::<Rebuild>().cloned() else {
            return;
        };
        if matches!(self.build, Build::Running(_)) {
            return;
        }
        let story = (self.stories.get(self.shown)).map_or("", |story| story.name);
        let appearance = if self.dark { "dark" } else { "light" };
        self.build = Build::Running(cx.spawn(async move |this, cx| {
            let built = backend::run(move || {
                let build = rebuild.start(rebuild.checkout())?;
                build.finish(|printed| {
                    if let Printed::Line(line) = printed {
                        eprintln!("{line}");
                    }
                })
            })
            .await;
            match built {
                #[cfg(unix)]
                Ok(built) => cx.update(|cx| {
                    let env = [
                        ("GITBUTLER_GPUI_STORY", story),
                        ("GITBUTLER_GPUI_APPEARANCE", appearance),
                    ];
                    let err = built.restart::<(), ()>(Vec::new(), &(), &env);
                    tracing::error!("couldn't restart on the new build: {err:#}");
                    cx.quit();
                }),
                #[cfg(not(unix))]
                Ok(_) => {}
                Err(err) => {
                    tracing::error!("couldn't build the stories: {err:#}");
                    this.update(cx, |this, cx| {
                        this.build = Build::Failed;
                        cx.notify();
                    })
                    .ok();
                }
            }
        }));
        cx.notify();
    }

    /// Rebuild, when `cargo run` started the stories.
    fn rebuild_button(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        cx.try_global::<Rebuild>()?;
        let (label, tooltip) = match self.build {
            Build::Idle => (
                "Rebuild",
                "Build the stories with cargo, and reopen on them",
            ),
            Build::Running(_) => ("Building…", "Cargo's output is in the terminal"),
            Build::Failed => (
                "Build failed",
                "Cargo's output is in the terminal; click to try again",
            ),
        };
        Some(
            button("rebuild", label)
                .small()
                .style(ButtonStyle::Gray)
                .tooltip(tooltip)
                .on_click(cx.listener(|this, _, _, cx| this.rebuild(cx))),
        )
    }

    fn set_theme(&self, cx: &mut App) {
        cx.set_global(
            (Theme::gitbutler(self.dark, CodeSize::default()))
                .with_density(self.density)
                .with_scale(self.scale),
        );
    }

    /// Where the stories whose name has the filter's text in it are, in their order.
    fn found<'a>(&'a self, cx: &App) -> impl Iterator<Item = usize> + 'a {
        let typed = self.filter.read(cx).text().trim().to_lowercase();
        (self.stories.iter().enumerate())
            .filter(move |(_, story)| story.name.to_lowercase().contains(&typed))
            .map(|(ix, _)| ix)
    }

    fn sidebar(&self, window: &Window, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let focused = self.filter.focus_handle(cx).is_focused(window);
        let found: Vec<usize> = self.found(cx).collect();
        v_flex()
            .flex_none()
            .w(dp(200.))
            .h_full()
            .py(dp(12.))
            .gap(dp(2.))
            .bg(theme.canvas)
            .border_r_1()
            .border_color(theme.border_subtle)
            .child(
                div()
                    .px(dp(14.))
                    .pb(dp(6.))
                    .child(section_label("Components", theme)),
            )
            .child(
                div()
                    .px(dp(10.))
                    .pb(dp(6.))
                    .child(field(&self.filter, focused, theme)),
            )
            // The stories scroll under their label once the window is shorter than the list.
            .child(
                v_flex()
                    .id("story-list")
                    .flex_1()
                    .min_h_0()
                    .gap(dp(2.))
                    .overflow_y_scroll()
                    .children(found.into_iter().filter_map(|ix| {
                        let story = self.stories.get(ix)?;
                        let row = list_row(("story", ix), ix == self.shown, theme)
                            .child(story.name)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.shown = ix;
                                cx.notify();
                            }));
                        Some(row)
                    })),
            )
    }

    fn toolbar(&self, name: &'static str, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let rebuild = self.rebuild_button(cx);
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
        // As the docs site's Density: how tall controls are.
        let densities = [
            ("Compact", Density::Compact),
            ("Balanced", Density::Balanced),
            ("Spacious", Density::Spacious),
        ]
        .map(|(label, density)| {
            let segment =
                (div().id(label).child(label)).on_click(cx.listener(move |this, _, _, cx| {
                    this.density = density;
                    this.set_theme(cx);
                    cx.notify();
                }));
            (segment, self.density == density)
        });
        h_flex()
            .flex_none()
            .h(dp(48.))
            .px(dp(24.))
            .gap(dp(12.))
            .border_b_1()
            .border_color(theme.border_subtle)
            .child(div().flex_1().text_15().semibold().child(name))
            .children(rebuild)
            .child(segmented(true, theme, densities))
            .child(segmented(true, theme, scales))
            .child(segmented(true, theme, appearance))
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Every `dp` size is in rems, so this scales the whole window, as the app's does.
        window.set_rem_size(Theme::get(cx).scaled(ui::BASE_REM));
        let theme = Theme::get(cx).clone();
        if let Some(example) = self.solo {
            return solo(example, &theme, self.dark).into_any_element();
        }
        let story = self.stories.get(self.shown);
        let name = story.map_or("", |story| story.name);
        h_flex()
            .size_full()
            .items_start()
            .font_family(theme.ui_font.clone())
            .text_13()
            .text_color(theme.foreground)
            .bg(theme.background)
            .child(self.sidebar(window, &theme, cx))
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
            .into_any_element()
    }
}

/// An example alone on the docs site's ground, with its 24px of room around it, and the window
/// resized to just that, so a shot of the whole window is the example as the site would show it.
fn solo(example: Example, theme: &Theme, dark: bool) -> Div {
    h_flex()
        .size_full()
        .items_start()
        .font_family(theme.ui_font.clone())
        .text_13()
        .text_color(theme.foreground)
        .bg(docs_ground(dark))
        .child(div().flex_none().p(dp(24.)).child((example.draw)(theme)))
        .on_children_prepainted(|bounds, window, _| {
            let Some(fits) = bounds.first().map(|bounds| bounds.size) else {
                return;
            };
            let size = window.viewport_size();
            let off = (size.width - fits.width).abs() + (size.height - fits.height).abs();
            // Resized once laid out, and again only if the new width wraps it differently.
            if off > fixed(0.5) {
                window.on_next_frame(move |window, _| window.resize(fits));
            }
        })
}

/// The docs site's ground under an example, `.docs-page-canvas` in but-ui-react's
/// `docs/site.css`: `--but-disabled`, and in dark 65% `--but-background` mixed into it.
fn docs_ground(dark: bool) -> Hsla {
    use crate::theme::tokens::{BACKGROUND, DISABLED};
    // One of a token's red, green and blue, in this appearance; tokens are RGBA.
    let channel = |[light, dark_value]: [u32; 2], shift: u32| {
        let rgba = if dark { dark_value } else { light };
        ((rgba >> (shift + 8)) & 0xff) as f32 / 255.
    };
    let share = if dark { 0.65 } else { 0. };
    let [r, g, b] = [16, 8, 0].map(|shift| {
        let (from, to) = (channel(DISABLED, shift), channel(BACKGROUND, shift));
        from + (to - from) * share
    });
    gpui::Rgba { r, g, b, a: 1. }.into()
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
        .child(faint(caption.into(), theme).text_11())
        .into_any_element()
}

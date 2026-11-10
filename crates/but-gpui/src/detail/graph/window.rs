//! A project's Graph page in a window of its own, which stays on that project whatever the main
//! window selects. It closes alone, and goes when the main window quits the app or the project
//! is removed.

use gpui::{
    App, Context, DisplayId, Entity, FocusHandle, SharedString, StyleRefinement, Subscription,
    Window, WindowBounds, div, prelude::*, size,
};

use super::{GraphState, GraphView};
use crate::app::{CloseWindow, Minimize, Zoom};
use crate::state::{ProjectId, Store};
use crate::theme::Theme;
use crate::ui::{self, HEADER_HEIGHT, dp, fixed, h_flex, v_flex};

/// How big it opens, and how small it can be made, in logical pixels.
const SIZE: (f32, f32) = (1000., 720.);
const LEAST: (f32, f32) = (480., 320.);

pub(crate) struct GraphWindow {
    pub(crate) store: Entity<Store>,
    project: ProjectId,
    name: SharedString,
    graph: Entity<GraphView>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// Bring `project`'s window in front, or open one on `display` looking at it as `left`.
pub(super) fn open(
    store: Entity<Store>,
    project: ProjectId,
    left: GraphState,
    display: Option<DisplayId>,
    cx: &mut App,
) {
    let open = (cx.windows().into_iter())
        .filter_map(|window| window.downcast::<GraphWindow>())
        .find(|window| window.read(cx).is_ok_and(|open| open.project == project));
    if let Some(window) = open {
        window.update(cx, |_, window, _| ui::activate(window)).ok();
        return;
    }
    let Some(name) = name_of(&store, &project, cx) else {
        return;
    };
    let default_size = size(fixed(SIZE.0), fixed(SIZE.1));
    let (display, bounds) = match crate::app::scripted() {
        true => (None, crate::off_screen(default_size)),
        false => (
            display,
            WindowBounds::Windowed(gpui::Bounds::centered(display, default_size, cx)),
        ),
    };
    let options = crate::window_options(
        title(&name),
        bounds,
        display,
        size(fixed(LEAST.0), fixed(LEAST.1)),
    );
    let window = cx.open_window(options, |window, cx| {
        cx.new(|cx| GraphWindow::new(store, project, name, left, window, cx))
    });
    if let Err(err) = window {
        tracing::error!("couldn't open the graph's window: {err:#}");
    }
}

fn name_of(store: &Entity<Store>, project: &ProjectId, cx: &App) -> Option<SharedString> {
    let entry = store.read(cx).project(project)?;
    Some(entry.project.name.clone().into())
}

fn title(name: &str) -> SharedString {
    format!("{name} – Graph").into()
}

impl GraphWindow {
    fn new(
        store: Entity<Store>,
        project: ProjectId,
        name: SharedString,
        left: GraphState,
        window: &mut Window,
        cx: &mut Context<GraphWindow>,
    ) -> GraphWindow {
        let graph = cx.new(|cx| {
            let mut graph = GraphView::new(store.clone(), cx);
            graph.own_window = true;
            graph.show_project(Some(project.clone()), cx);
            graph.restore(left, cx);
            graph
        });
        let id = window.window_handle().window_id();
        let subscriptions = Vec::from([
            cx.observe_in(&store, window, GraphWindow::follow_project),
            cx.observe_window_visibility(window, move |this, visibility, _, cx| {
                if !crate::app::scripted() {
                    this.store.update(cx, |store, cx| {
                        store.set_visible(id, visibility.is_visible(), cx)
                    });
                }
            }),
            cx.on_release(move |this, cx| {
                this.store
                    .update(cx, |store, cx| store.set_visible(id, false, cx))
            }),
            #[cfg(target_os = "macos")]
            cx.observe_global_in::<Theme>(window, |_, window, cx| {
                crate::app::place_traffic_lights(false, window, cx)
            }),
        ]);
        #[cfg(target_os = "macos")]
        crate::app::place_traffic_lights(false, window, cx);
        // Visibility arrives only as it changes, so it starts as the window opens.
        let visible = crate::app::scripted() || window.is_visible();
        store.update(cx, |store, cx| store.set_visible(id, visible, cx));
        // With nothing focused, its keys would reach no listener here.
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        GraphWindow {
            store,
            project,
            name,
            graph,
            focus,
            _subscriptions: subscriptions,
        }
    }

    /// Keep to the project's name, and close once it's removed.
    fn follow_project(
        &mut self,
        store: Entity<Store>,
        window: &mut Window,
        cx: &mut Context<GraphWindow>,
    ) {
        match name_of(&store, &self.project, cx) {
            Some(name) if name != self.name => {
                window.set_window_title(&title(&name));
                self.name = name;
                cx.notify();
            }
            Some(_) => {}
            None => window.remove_window(),
        }
    }
}

impl Render for GraphWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<GraphWindow>) -> impl IntoElement {
        // Every `dp` size is in rems, so this scales the whole window.
        window.set_rem_size(Theme::get(cx).scaled(ui::BASE_REM));
        let theme = Theme::get(cx);
        let header = ui::drags_window(
            h_flex()
                .h(dp(HEADER_HEIGHT))
                .flex_none()
                .justify_center()
                .gap(dp(6.))
                .bg(theme.bg_1)
                .border_b_1()
                .border_color(theme.border_2)
                .text_size(dp(13.))
                .child(self.name.clone())
                .child(div().text_color(theme.text_3).child("Graph")),
        );
        let root = v_flex()
            .id("graph-window")
            .key_context("GraphWindow")
            .track_focus(&self.focus)
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .relative()
            .size_full()
            .bg(theme.bg_2)
            .text_color(theme.text_1)
            .font_family(theme.ui_font.clone())
            .text_size(dp(13.))
            .line_height(dp(18.))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child((self.graph.clone()).cached(StyleRefinement::default().size_full())),
            )
            .when(ui::draws_window_controls(window), |el| {
                el.child(ui::window_controls::window_controls(window, cx))
            })
            .when(cfg!(target_os = "macos"), |el| {
                el.child(ui::holds_back_first_click())
            });
        ui::window_frame::window_frame(root, window, cx)
    }
}

//! The menu bar the app draws on Windows and Linux, where GPUI shows none, as Zed draws its: a
//! button in the window's top left corner, which opens the first of `menus.json`'s menus, and
//! while one of them is open, a row of every menu's name over the header. Each menu is a
//! right-click menu, so it takes the keys as one does, and ← and → go to the menus beside it.
//! Settings' Developer page draws it on macOS too, to try it there.

use gpui::{
    AnyElement, App, Context, MouseButton, MouseDownEvent, Pixels, Point, Role, Window, actions,
    deferred, div, point, prelude::*,
};

use crate::app::AppView;
use crate::backend;
use crate::keymap::{self, menus::BarItem, menus::BarMenu};
use crate::overlays::{MenuItem, OVERLAY, overlay};
use crate::state::Prefs;
use crate::theme::Theme;
use crate::ui::{HEADER_HEIGHT, Icon, dp, fixed, h_flex, icon_button};

actions!(menu_bar, [OpenMenuBar, PreviousMenu, NextMenu]);

/// How wide the button is, and the room the headers leave after it.
const BUTTON: f32 = 28.;
const GAP: f32 = 6.;
/// How far the headers start from the window's left edge to clear macOS's traffic lights.
const TRAFFIC_LIGHTS: f32 = 84.;

/// Whether the app draws its menu bar: where the platform has none, and on macOS while
/// Settings' Developer page asks for it.
pub fn shown(prefs: &Prefs) -> bool {
    !cfg!(target_os = "macos") || (prefs.menu_bar_everywhere && backend::settings::dev_build())
}

/// Where the button is from the window's left edge: past macOS's traffic lights, else where the
/// headers' own start.
fn left(window: &Window, theme: &Theme) -> Pixels {
    match crate::ui::has_traffic_lights(window) {
        true => fixed(TRAFFIC_LIGHTS),
        false => theme.scaled(8.),
    }
}

/// How far the headers start from the window's left edge to clear what's in its top left
/// corner: macOS's traffic lights, and the menu bar's button where the app draws one. None
/// while neither is there.
pub fn top_left_room(window: &Window, prefs: &Prefs, theme: &Theme) -> Option<Pixels> {
    match shown(prefs) {
        true => Some(left(window, theme) + theme.scaled(BUTTON + GAP)),
        false => crate::ui::has_traffic_lights(window).then(|| fixed(TRAFFIC_LIGHTS)),
    }
}

/// What only macOS's app menu does: elsewhere there's no hiding apps.
const MACOS_ONLY: [&str; 3] = ["app::Hide", "app::HideOthers", "app::ShowAll"];

impl AppView {
    /// `menu`'s items as a right-click menu has them: the actions this build offers, and of the
    /// commands those offered here, worded as the bar words them, with the keys it shows beside
    /// them and a line where it divides them.
    fn bar_menu_items(&self, menu: &BarMenu, window: &Window, cx: &App) -> Vec<MenuItem> {
        let mut separated = false;
        let mut items = Vec::new();
        for item in &menu.items {
            let action = match item {
                BarItem::Action { action, .. } => action,
                BarItem::Divider => {
                    separated = true;
                    continue;
                }
                // macOS fills it; there's nothing to fill it with here.
                BarItem::System(_) => continue,
            };
            if !keymap::menus::offered(action, cx)
                || (!cfg!(target_os = "macos") && MACOS_ONLY.contains(&action.as_str()))
            {
                continue;
            }
            let command = (crate::commands::COMMANDS.iter()).find(|c| c.name() == action);
            if command.is_some_and(|command| !self.offers(*command, window, cx)) {
                continue;
            }
            let Ok(built) = cx.build_action(action, None) else {
                continue;
            };
            items.push(MenuItem {
                key: keymap::menus::shortcut(action, cx),
                separated: std::mem::take(&mut separated),
                ..MenuItem::action(item.title(), built.as_ref(), cx)
            });
        }
        items
    }

    /// The menu bar's menu that's open or about to be, by its place in the bar.
    fn bar_menu_open(&self) -> Option<usize> {
        self.menu_bar
            .filter(|_| self.menu.is_some() || self.pending_menu.is_some())
    }

    /// Open the menu bar's menu `ix`, in place of any menu open.
    fn open_bar_menu(&mut self, ix: usize, window: &Window, cx: &mut Context<Self>) {
        let Some(menu) = keymap::bar_menus(cx).into_iter().nth(ix) else {
            return;
        };
        let items = self.bar_menu_items(&menu, window, cx);
        if items.is_empty() {
            return;
        }
        // Under the header, until the row of names is drawn for it to hang from its name.
        let theme = Theme::get(cx);
        let position = point(left(window, theme), theme.scaled(HEADER_HEIGHT));
        self.open_menu(position, items, cx);
        self.menu_bar = Some(ix);
    }

    /// Where the open menu hangs from, when it's one of the menu bar's: under its name, below
    /// the row, which is drawn over it.
    pub(crate) fn bar_menu_position(&self, theme: &Theme) -> Option<Point<Pixels>> {
        let ix = self.bar_menu_open()?;
        let names = self.menu_bar_names.borrow();
        let name = names.get(ix)?;
        Some(point(name.left(), theme.scaled(HEADER_HEIGHT)))
    }

    /// F10: the menu bar's first menu, or none if one of its menus is open.
    pub(crate) fn open_menu_bar(
        &mut self,
        _: &OpenMenuBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !shown(&self.store.read(cx).prefs) {
            cx.propagate();
            return;
        }
        match self.bar_menu_open() {
            Some(_) => self.close_menu(window, cx),
            None => self.open_bar_menu(0, window, cx),
        }
    }

    pub(crate) fn previous_menu(
        &mut self,
        _: &PreviousMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_bar_menu(false, window, cx);
    }

    pub(crate) fn next_menu(&mut self, _: &NextMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.step_bar_menu(true, window, cx);
    }

    /// The menu bar's menu beside the open one, round from either end. Elsewhere ← and → go
    /// on to what has the keys.
    fn step_bar_menu(&mut self, forward: bool, window: &Window, cx: &mut Context<Self>) {
        let Some(open) = self.bar_menu_open() else {
            cx.propagate();
            return;
        };
        let count = keymap::bar_menus(cx).len();
        if count == 0 {
            return;
        }
        let next = match forward {
            true => (open + 1) % count,
            false => (open + count - 1) % count,
        };
        self.open_bar_menu(next, window, cx);
    }

    /// The button, or while one of its menus is open, the row of the menus' names over the
    /// header, above the open menu's backdrop so the pointer can go along it.
    pub(crate) fn render_menu_bar(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !shown(&self.store.read(cx).prefs) {
            return None;
        }
        let theme = Theme::get(cx);
        let left = left(window, theme);
        let Some(open) = self.bar_menu_open() else {
            let button = icon_button("menu-bar", Icon::Menu)
                .tooltip_for("Menu", &OpenMenuBar)
                .dispatch(&OpenMenuBar);
            return Some(overlay(
                h_flex()
                    .absolute()
                    .top_0()
                    .left(left)
                    .h(theme.scaled(HEADER_HEIGHT))
                    // The header's drag area is under it.
                    .occlude()
                    .child(button),
            ));
        };
        let names = (keymap::bar_menus(cx).into_iter().enumerate()).map(|(ix, menu)| {
            div()
                .id(("menu-bar-name", ix))
                .role(Role::MenuItem)
                .aria_label(menu.name.clone())
                .flex()
                .items_center()
                .h(dp(BUTTON))
                .px(dp(8.))
                .rounded(dp(theme.shape.card))
                .text_size(dp(13.))
                .text_color(theme.text_1)
                .when(ix == open, |el| el.bg(theme.hover()))
                .hover(|el| el.bg(theme.hover()))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        // The open menu closed as the button went down outside it, so a click
                        // on its own name leaves it closed.
                        match this.menu_bar == Some(ix) {
                            true => this.menu_bar = None,
                            false => this.open_bar_menu(ix, window, cx),
                        }
                    }),
                )
                .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                    if *hovered && this.bar_menu_open().is_some_and(|open| open != ix) {
                        this.open_bar_menu(ix, window, cx);
                    }
                }))
                .child(menu.name)
        });
        let drawn = self.menu_bar_names.clone();
        let row = h_flex()
            .on_children_prepainted(move |bounds, _, _| *drawn.borrow_mut() = bounds)
            .id("menu-bar")
            .role(Role::MenuBar)
            .absolute()
            .top_0()
            .left_0()
            .h(theme.scaled(HEADER_HEIGHT) - fixed(1.))
            .pl(left)
            .pr(dp(8.))
            .gap(dp(2.))
            .bg(theme.bg_2)
            .occlude()
            .children(names);
        Some(deferred(row).with_priority(OVERLAY + 1).into_any_element())
    }
}

//! The command editor's Menu bar tab: the menus as macOS shows them, each item with the keys it
//! shows, and every command to drag into them. A change is saved to `menus.json` as it's made,
//! and the menu bar takes it at once (`keymap::edit`).

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, FontWeight, ListAlignment, ListState, Role,
    SharedString, Subscription, Window, div, list,
};

use super::rows::Command;
use crate::keymap::menus::{self, BarItem, BarMenu};
use crate::keymap::{self, KeymapFile, Loaded};
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    self, ButtonStyle, Icon, Tooltip, button, dp, field, h_flex, icon, icon_button, list_row,
    v_flex,
};

const SOURCE_WIDTH: f32 = 300.;
const MENU_WIDTH: f32 = 230.;
const ITEM_HEIGHT: f32 = 24.;
const COMMAND_HEIGHT: f32 = 44.;
const SMALL_TEXT: f32 = 13.;

/// What a new menu is called until it's named.
const NEW_MENU: &str = "New Menu";

/// Where an item is in the bar.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Spot {
    menu: usize,
    ix: usize,
}

/// An item on its way somewhere in the bar.
#[derive(Clone)]
struct Dragged {
    item: BarItem,
    title: SharedString,
    /// Where it is now, when it's moved rather than added.
    from: Option<Spot>,
}

/// A menu on its way to another place in the bar, by its name.
#[derive(Clone)]
struct DraggedMenu {
    ix: usize,
    name: SharedString,
}

/// What an item shows, worked out as the menus are read.
struct Shown {
    title: SharedString,
    keys: Option<SharedString>,
    /// Why it's not in the menu bar, when it isn't.
    left_out: Option<&'static str>,
}

struct Renaming {
    menu: usize,
    /// Its name as renaming began, so a menu that's moved meanwhile isn't the one renamed.
    name: String,
    input: Entity<TextInput>,
    _events: Subscription,
}

pub struct MenuBarEditor {
    menus: Vec<BarMenu>,
    /// What each item of `menus` shows, menu by menu.
    shown: Vec<Vec<Shown>>,
    commands: Vec<Command>,
    /// The commands the search finds, by their place in `commands`.
    found: Vec<usize>,
    search: Entity<TextInput>,
    list: ListState,
    renaming: Option<Renaming>,
    /// What went wrong saving the last change.
    pub warning: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl MenuBarEditor {
    pub fn new(
        commands: Vec<Command>,
        window: &mut Window,
        cx: &mut Context<MenuBarEditor>,
    ) -> MenuBarEditor {
        let search = cx.new(|cx| TextInput::new("Find a command", false, cx));
        let subscriptions = Vec::from([
            cx.subscribe_in(&search, window, |this, search, event, _, cx| match event {
                InputEvent::Changed => this.find(cx),
                InputEvent::Cancel => search.update(cx, |search, cx| search.set_text("", cx)),
                InputEvent::Submit => {}
            }),
            cx.observe_global::<Loaded>(|this, cx| this.load(cx)),
        ]);
        let mut this = MenuBarEditor {
            menus: Vec::new(),
            shown: Vec::new(),
            commands,
            found: Vec::new(),
            search,
            list: ListState::new(0, ListAlignment::Top, ui::fixed(400.)),
            renaming: None,
            warning: None,
            _subscriptions: subscriptions,
        };
        this.commands
            .sort_by_cached_key(|command| menus::title_case(&command.title).to_lowercase());
        this.load(cx);
        this.find(cx);
        this
    }

    /// Read the menus as the file says them now, with the keys each item shows.
    fn load(&mut self, cx: &mut Context<MenuBarEditor>) {
        self.menus = keymap::bar_menus(cx);
        self.shown = (self.menus.iter())
            .map(|menu| (menu.items.iter()).map(|item| shown(item, cx)).collect())
            .collect();
        // The commands' rows say which menu has them.
        self.list.reset(self.found.len());
        cx.notify();
    }

    /// Show the commands the search finds.
    fn find(&mut self, cx: &mut Context<MenuBarEditor>) {
        let query = self.search.read(cx).text().to_lowercase();
        self.found = (self.commands.iter().enumerate())
            .filter(|(_, command)| found_by(command, &query))
            .map(|(ix, _)| ix)
            .collect();
        self.list.reset(self.found.len());
        cx.notify();
    }

    /// Save `menus` as the bar, unless they're what it is already.
    fn save(&mut self, menus: Vec<BarMenu>, cx: &mut Context<MenuBarEditor>) {
        if menus == self.menus {
            return;
        }
        let saved = keymap::edit(KeymapFile::Menus, |_| menus::written(&menus), cx);
        self.warning = saved.err().map(|err| format!("Couldn't save: {err:#}"));
        self.load(cx);
    }

    fn drop_at(&mut self, dragged: &Dragged, to: Spot, cx: &mut Context<MenuBarEditor>) {
        match with_dropped(&self.menus, dragged, to) {
            Some(menus) => self.save(menus, cx),
            // The file changed under the drag.
            None => self.load(cx),
        }
    }

    fn remove_item(&mut self, at: Spot, item: &BarItem, cx: &mut Context<MenuBarEditor>) {
        match without(&self.menus, at, item) {
            Some(menus) => self.save(menus, cx),
            None => self.load(cx),
        }
    }

    fn move_menu(&mut self, dragged: &DraggedMenu, to: usize, cx: &mut Context<MenuBarEditor>) {
        match with_menu_moved(&self.menus, dragged, to) {
            Some(menus) => self.save(menus, cx),
            None => self.load(cx),
        }
    }

    fn add_menu(&mut self, window: &mut Window, cx: &mut Context<MenuBarEditor>) {
        let mut menus = self.menus.clone();
        menus.push(BarMenu {
            name: NEW_MENU.to_owned(),
            items: Vec::new(),
        });
        let added = menus.len() - 1;
        self.save(menus, cx);
        if self.menus.len() > added {
            self.start_renaming(added, window, cx);
        }
    }

    fn remove_menu(&mut self, menu: usize, name: &str, cx: &mut Context<MenuBarEditor>) {
        let mut menus = self.menus.clone();
        if menus.get(menu).is_none_or(|it| it.name != name) {
            return self.load(cx);
        }
        menus.remove(menu);
        self.renaming = None;
        self.save(menus, cx);
    }

    fn start_renaming(
        &mut self,
        menu: usize,
        window: &mut Window,
        cx: &mut Context<MenuBarEditor>,
    ) {
        let Some(name) = self.menus.get(menu).map(|menu| menu.name.clone()) else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Menu name", false, cx);
            input.set_text(name.clone(), cx);
            input.select_all_text(cx);
            input
        });
        let events = cx.subscribe_in(&input, window, |this, input, event, _, cx| match event {
            InputEvent::Submit => {
                let name = input.read(cx).text().trim().to_owned();
                this.finish_renaming(name, cx);
            }
            InputEvent::Cancel => {
                this.renaming = None;
                cx.notify();
            }
            InputEvent::Changed => {}
        });
        window.focus(&input.read(cx).focus_handle(cx), cx);
        self.renaming = Some(Renaming {
            menu,
            name,
            input,
            _events: events,
        });
        cx.notify();
    }

    fn finish_renaming(&mut self, name: String, cx: &mut Context<MenuBarEditor>) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        cx.notify();
        if name.is_empty() {
            return;
        }
        let mut menus = self.menus.clone();
        match menus.get_mut(renaming.menu) {
            Some(menu) if menu.name == renaming.name => menu.name = name,
            Some(_) | None => return,
        }
        self.save(menus, cx);
    }

    fn render_source(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<MenuBarEditor>,
    ) -> impl IntoElement {
        let focused = self.search.read(cx).focus_handle(cx).is_focused(window);
        let divider = Dragged {
            item: BarItem::Divider,
            title: "Divider".into(),
            from: None,
        };
        let commands = list(
            self.list.clone(),
            cx.processor(|this, ix, _, cx| this.render_command(ix, cx)),
        )
        .size_full();
        v_flex()
            .flex_none()
            .w(dp(SOURCE_WIDTH))
            .h_full()
            .border_r_1()
            .border_color(theme.border_2)
            .child(div().flex_none().px(dp(12.)).py(dp(10.)).child(field(
                &self.search,
                focused,
                theme,
            )))
            .child(
                list_row("divider-source", false, theme)
                    .cursor_grab()
                    .text_size(dp(SMALL_TEXT))
                    .child(div().flex_1().h(ui::fixed(1.)).bg(theme.border_2))
                    .child("Divider")
                    .child(div().flex_1().h(ui::fixed(1.)).bg(theme.border_2))
                    .on_drag(divider, drag_preview),
            )
            .child(scrolled(
                commands,
                "menu-commands-scrollbar",
                Scroll::List(self.list.clone()),
                theme,
            ))
    }

    fn render_command(&self, ix: usize, cx: &mut Context<MenuBarEditor>) -> AnyElement {
        let Some(command) = self.found.get(ix).and_then(|ix| self.commands.get(*ix)) else {
            return div().into_any_element();
        };
        let theme = Theme::get(cx);
        let item = BarItem::Action {
            action: command.name.clone(),
            title: None,
        };
        let title: SharedString = item.title().into();
        let menu = (self.menus.iter())
            .find(|menu| {
                menu.items
                    .iter()
                    .any(|it| it.action() == Some(&command.name))
            })
            .map(|menu| format!("In {}", menu.name));
        let dragged = Dragged {
            item,
            title: title.clone(),
            from: None,
        };
        let row = list_row(("command", ix), false, theme)
            .flex_grow(1.)
            .h(dp(COMMAND_HEIGHT))
            .cursor_grab()
            .on_drag(dragged, drag_preview)
            .child(
                v_flex()
                    .flex_grow(1.)
                    .min_w_0()
                    .child(div().truncate().child(title))
                    .child(
                        div()
                            .truncate()
                            .font_family(theme.mono_font.clone())
                            .text_size(dp(12.))
                            .text_color(theme.text_2)
                            .child(command.name.clone()),
                    ),
            )
            .children(menu.map(|menu| {
                div()
                    .flex_shrink(1000.)
                    .truncate()
                    .text_size(dp(12.))
                    .text_color(theme.text_2)
                    .child(menu)
            }));
        h_flex().w_full().child(row).into_any_element()
    }

    fn render_menu(
        &self,
        ix: usize,
        menu: &BarMenu,
        theme: &Theme,
        cx: &mut Context<MenuBarEditor>,
    ) -> impl IntoElement + use<> {
        let shown = self.shown.get(ix);
        let items: Vec<AnyElement> = (menu.items.iter().enumerate())
            .map(|(at, item)| {
                let spot = Spot { menu: ix, ix: at };
                let shown = shown.and_then(|shown| shown.get(at));
                self.render_item(spot, item, shown, theme, cx)
            })
            .collect();
        let end = Spot {
            menu: ix,
            ix: menu.items.len(),
        };
        let empty = menu.items.is_empty();
        let pop = theme.pop;
        // What's dropped below the items goes last.
        let tail = div()
            .id("tail")
            .flex_grow(1.)
            .min_h(dp(if empty { 60. } else { ITEM_HEIGHT }))
            .border_t_2()
            .border_color(gpui::transparent_black())
            .drag_over::<Dragged>(move |style, _, _, _| style.border_color(pop))
            .on_drop(
                cx.listener(move |this, dragged: &Dragged, _, cx| this.drop_at(dragged, end, cx)),
            )
            .when(empty, |el| {
                el.flex()
                    .items_center()
                    .justify_center()
                    .text_size(dp(SMALL_TEXT))
                    .text_color(theme.text_2)
                    .child("Drag commands here")
            });
        v_flex()
            .id(("menu", ix))
            .flex_none()
            .w(dp(MENU_WIDTH))
            .gap(dp(6.))
            // A line beside it shows a menu dropped on it goes before it.
            .pl(dp(6.))
            .border_l_2()
            .border_color(gpui::transparent_black())
            .drag_over::<DraggedMenu>(move |style, _, _, _| style.border_color(pop))
            .on_drop(cx.listener(move |this, dragged: &DraggedMenu, _, cx| {
                this.move_menu(dragged, ix, cx)
            }))
            .child(self.render_menu_header(ix, menu, theme, cx))
            .child(
                v_flex()
                    .min_h(dp(120.))
                    .py(dp(5.))
                    .rounded(dp(theme.shape.card))
                    .border_1()
                    .border_color(theme.border_2)
                    .bg(theme.bg_2)
                    .text_size(dp(SMALL_TEXT))
                    .children(items)
                    .child(tail),
            )
    }

    fn render_menu_header(
        &self,
        ix: usize,
        menu: &BarMenu,
        theme: &Theme,
        cx: &mut Context<MenuBarEditor>,
    ) -> AnyElement {
        if let Some(renaming) = self.renaming.as_ref().filter(|it| it.menu == ix) {
            return div()
                .h(dp(30.))
                .child(field(&renaming.input, true, theme))
                .into_any_element();
        }
        let name = menu.name.clone();
        let removed = menu.name.clone();
        let note = (ix == 0).then_some("Named after the app");
        let dragged = DraggedMenu {
            ix,
            name: menu.name.clone().into(),
        };
        h_flex()
            .id("header")
            .group("menu-header")
            .cursor_grab()
            .on_drag(dragged, menu_drag_preview)
            .h(dp(30.))
            .gap(dp(4.))
            .child(
                h_flex()
                    .id("name")
                    .role(Role::Button)
                    .aria_label(format!("Rename {name}"))
                    .min_w_0()
                    .gap(dp(6.))
                    .px(dp(4.))
                    .rounded(dp(theme.shape.control))
                    .hover(|el| el.bg(theme.hover()))
                    .tooltip(Tooltip::text("Rename"))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.start_renaming(ix, window, cx)),
                    )
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    ),
            )
            .children(note.map(|note| {
                div()
                    .flex_shrink(1000.)
                    .truncate()
                    .text_size(dp(12.))
                    .text_color(theme.text_2)
                    .child(note)
            }))
            .child(div().flex_grow(1.))
            .child(
                div()
                    .flex_none()
                    .invisible()
                    .group_hover("menu-header", |el| el.visible())
                    .child(
                        icon_button("remove-menu", Icon::Cross)
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip("Remove menu")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_menu(ix, &removed, cx)
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_item(
        &self,
        spot: Spot,
        item: &BarItem,
        shown: Option<&Shown>,
        theme: &Theme,
        cx: &mut Context<MenuBarEditor>,
    ) -> AnyElement {
        let Some(shown) = shown else {
            return div().into_any_element();
        };
        let dragged = Dragged {
            item: item.clone(),
            title: match item {
                BarItem::Divider => "Divider".into(),
                BarItem::Action { .. } | BarItem::System(_) => shown.title.clone(),
            },
            from: Some(spot),
        };
        let removed = item.clone();
        let remove = div()
            .id("remove")
            .role(Role::Button)
            .aria_label(format!("Remove {}", dragged.title))
            .flex_none()
            .invisible()
            .group_hover("item", |el| el.visible())
            .tooltip(Tooltip::text("Remove from the menu"))
            .on_click(cx.listener(move |this, _, _, cx| this.remove_item(spot, &removed, cx)))
            .child(icon(Icon::Cross).size(dp(12.)).text_color(theme.text_1));
        let pop = theme.pop;
        let row = h_flex()
            .id(("item", spot.ix))
            .group("item")
            .mx(dp(5.))
            .px(dp(8.))
            .gap(dp(8.))
            .rounded(dp(theme.shape.control))
            .cursor_grab()
            // A line above it shows a drop goes before it.
            .border_t_2()
            .border_color(gpui::transparent_black())
            .drag_over::<Dragged>(move |style, _, _, _| style.border_color(pop))
            .on_drop(
                cx.listener(move |this, dragged: &Dragged, _, cx| this.drop_at(dragged, spot, cx)),
            )
            .on_drag(dragged, drag_preview);
        match item {
            BarItem::Divider => row
                .h(dp(11.))
                .child(div().flex_1().h(ui::fixed(1.)).bg(theme.border_2))
                .child(remove),
            BarItem::Action { .. } | BarItem::System(_) => {
                let text_color = match shown.left_out {
                    Some(_) => theme.text_2,
                    None => theme.text_1,
                };
                row.h(dp(ITEM_HEIGHT))
                    .hover(|el| el.bg(theme.hover()))
                    .when_some(shown.left_out, |el, why| el.tooltip(Tooltip::text(why)))
                    .child(
                        div()
                            .flex_grow(1.)
                            .min_w_0()
                            .truncate()
                            .text_color(text_color)
                            .when(shown.left_out.is_some(), |el| el.italic())
                            .child(shown.title.clone()),
                    )
                    .children(shown.keys.as_ref().map(|keys| {
                        div()
                            .flex_shrink(1000.)
                            .text_color(theme.text_2)
                            .child(ui::keys_text(keys))
                    }))
                    .when(matches!(item, BarItem::System(_)), |el| {
                        el.child(
                            icon(Icon::ChevronRight)
                                .size(dp(12.))
                                .flex_none()
                                .text_color(theme.text_2),
                        )
                    })
                    .child(remove)
            }
        }
        .into_any_element()
    }
}

impl Render for MenuBarEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<MenuBarEditor>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let menus: Vec<AnyElement> = (self.menus.iter().enumerate())
            .map(|(ix, menu)| self.render_menu(ix, menu, &theme, cx).into_any_element())
            .collect();
        let (pop, last) = (theme.pop, self.menus.len());
        let add = button("add-menu", "Add menu")
            .icon(Icon::Plus)
            .style(ButtonStyle::Outline)
            .small()
            .on_click(cx.listener(|this, _, window, cx| this.add_menu(window, cx)));
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .child(self.render_source(&theme, window, cx))
            .child(
                // A row, so the menus' row is as wide as they are rather than as it is.
                h_flex()
                    .id("menus")
                    .items_start()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_x_scroll()
                    .overflow_y_scroll()
                    .child(
                        h_flex()
                            .flex_none()
                            .items_start()
                            .gap(dp(10.))
                            .p(dp(16.))
                            .children(menus)
                            .child(
                                // A menu dropped here goes last.
                                div()
                                    .id("menus-end")
                                    .flex_none()
                                    .min_h(dp(120.))
                                    .pl(dp(6.))
                                    .pt(dp(2.))
                                    .border_l_2()
                                    .border_color(gpui::transparent_black())
                                    .drag_over::<DraggedMenu>(move |style, _, _, _| {
                                        style.border_color(pop)
                                    })
                                    .on_drop(cx.listener(
                                        move |this, dragged: &DraggedMenu, _, cx| {
                                            this.move_menu(dragged, last, cx)
                                        },
                                    ))
                                    .child(add),
                            ),
                    ),
            )
    }
}

/// What `item` shows in the menu.
fn shown(item: &BarItem, cx: &App) -> Shown {
    let title = item.title().into();
    let BarItem::Action { action, .. } = item else {
        return Shown {
            title,
            keys: None,
            left_out: None,
        };
    };
    let left_out = match cx.build_action(action, None) {
        Err(_) => Some("Not an action, so it's left out"),
        Ok(_) if !menus::offered(action, cx) => Some("Only where the build looks for updates"),
        Ok(_) => None,
    };
    Shown {
        title,
        keys: menus::shortcut(action, cx),
        left_out,
    }
}

/// Whether every word of `query`, in lowercase, is in the command's title, name or aliases.
fn found_by(command: &Command, query: &str) -> bool {
    let words = format!(
        "{} {} {}",
        command.title,
        command.name,
        command.aliases.join(" ")
    )
    .to_lowercase();
    (query.split_whitespace()).all(|word| words.contains(word))
}

/// `menus` with `dragged` put before the item at `to`, or `None` when the bar has changed under
/// the drag, and what it moves isn't where it was.
fn with_dropped(menus: &[BarMenu], dragged: &Dragged, to: Spot) -> Option<Vec<BarMenu>> {
    let mut menus = menus.to_vec();
    let mut at = to.ix;
    if let Some(from) = dragged.from {
        let items = &mut menus.get_mut(from.menu)?.items;
        if items.get(from.ix) != Some(&dragged.item) {
            return None;
        }
        items.remove(from.ix);
        if from.menu == to.menu && from.ix < at {
            at = at.saturating_sub(1);
        }
    }
    let items = &mut menus.get_mut(to.menu)?.items;
    items.insert(at.min(items.len()), dragged.item.clone());
    Some(menus)
}

/// `menus` with the menu `dragged` put before the one at `to`, or last past them, or `None` when
/// it's moved in the file meanwhile.
fn with_menu_moved(menus: &[BarMenu], dragged: &DraggedMenu, to: usize) -> Option<Vec<BarMenu>> {
    let mut menus = menus.to_vec();
    if menus.get(dragged.ix)?.name != dragged.name.as_ref() {
        return None;
    }
    let menu = menus.remove(dragged.ix);
    let to = match dragged.ix < to {
        true => to.saturating_sub(1),
        false => to,
    };
    menus.insert(to.min(menus.len()), menu);
    Some(menus)
}

/// `menus` without `item` at `at`, or `None` when it isn't there now.
fn without(menus: &[BarMenu], at: Spot, item: &BarItem) -> Option<Vec<BarMenu>> {
    let mut menus = menus.to_vec();
    let items = &mut menus.get_mut(at.menu)?.items;
    if items.get(at.ix) != Some(item) {
        return None;
    }
    items.remove(at.ix);
    Some(menus)
}

/// The item following the pointer while it's dragged.
struct DragPreview {
    title: SharedString,
}

fn drag_preview(
    dragged: &Dragged,
    _: gpui::Point<gpui::Pixels>,
    _: &mut Window,
    cx: &mut App,
) -> Entity<DragPreview> {
    let title = dragged.title.clone();
    cx.new(|_| DragPreview { title })
}

fn menu_drag_preview(
    dragged: &DraggedMenu,
    _: gpui::Point<gpui::Pixels>,
    _: &mut Window,
    cx: &mut App,
) -> Entity<DragPreview> {
    let title = dragged.name.clone();
    cx.new(|_| DragPreview { title })
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<DragPreview>) -> impl IntoElement {
        let theme = Theme::get(cx);
        div()
            .px(dp(10.))
            .py(dp(4.))
            .rounded(dp(theme.shape.control))
            .border_1()
            .border_color(theme.border_2)
            .bg(theme.bg_1)
            .shadow_md()
            .font_family(theme.ui_font.clone())
            .text_size(dp(SMALL_TEXT))
            .text_color(theme.text_1)
            .child(self.title.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BarItem, BarMenu, Dragged, DraggedMenu, Spot, with_dropped, with_menu_moved, without,
    };

    fn action(name: &str) -> BarItem {
        BarItem::Action {
            action: name.to_owned(),
            title: None,
        }
    }

    fn bar() -> Vec<BarMenu> {
        Vec::from([
            BarMenu {
                name: "File".to_owned(),
                items: Vec::from([action("a::One"), BarItem::Divider, action("a::Two")]),
            },
            BarMenu {
                name: "View".to_owned(),
                items: Vec::from([action("a::Three")]),
            },
        ])
    }

    /// Each menu's items, by their action's name, `-` for a divider.
    fn names(menus: &[BarMenu]) -> Vec<Vec<String>> {
        (menus.iter())
            .map(|menu| {
                (menu.items.iter())
                    .map(|item| match item {
                        BarItem::Action { action, .. } => action.clone(),
                        BarItem::Divider | BarItem::System(_) => "-".to_owned(),
                    })
                    .collect()
            })
            .collect()
    }

    fn moved(item: BarItem, from: Spot) -> Dragged {
        Dragged {
            item,
            title: "".into(),
            from: Some(from),
        }
    }

    fn added(item: BarItem) -> Dragged {
        Dragged {
            item,
            title: "".into(),
            from: None,
        }
    }

    #[test]
    fn a_command_dropped_on_an_item_goes_before_it() {
        let menus = with_dropped(&bar(), &added(action("a::New")), Spot { menu: 0, ix: 2 });
        assert_eq!(
            names(&menus.expect("the menu is there")),
            [
                Vec::from(["a::One", "-", "a::New", "a::Two"]),
                Vec::from(["a::Three"])
            ]
        );
    }

    #[test]
    fn a_command_dropped_below_the_items_goes_last() {
        let menus = with_dropped(&bar(), &added(action("a::New")), Spot { menu: 1, ix: 1 });
        assert_eq!(
            names(&menus.expect("the menu is there")),
            [
                Vec::from(["a::One", "-", "a::Two"]),
                Vec::from(["a::Three", "a::New"])
            ]
        );
    }

    #[test]
    fn an_item_moved_down_its_menu_goes_before_the_one_dropped_on() {
        let dragged = moved(action("a::One"), Spot { menu: 0, ix: 0 });
        let menus = with_dropped(&bar(), &dragged, Spot { menu: 0, ix: 2 });
        assert_eq!(
            names(&menus.expect("the item is there")),
            [
                Vec::from(["-", "a::One", "a::Two"]),
                Vec::from(["a::Three"])
            ]
        );
    }

    #[test]
    fn an_item_moved_up_its_menu_goes_before_the_one_dropped_on() {
        let dragged = moved(action("a::Two"), Spot { menu: 0, ix: 2 });
        let menus = with_dropped(&bar(), &dragged, Spot { menu: 0, ix: 0 });
        assert_eq!(
            names(&menus.expect("the item is there")),
            [
                Vec::from(["a::Two", "a::One", "-"]),
                Vec::from(["a::Three"])
            ]
        );
    }

    #[test]
    fn an_item_moved_to_the_end_of_its_menu_goes_last() {
        let dragged = moved(action("a::One"), Spot { menu: 0, ix: 0 });
        let menus = with_dropped(&bar(), &dragged, Spot { menu: 0, ix: 3 });
        assert_eq!(
            names(&menus.expect("the item is there")),
            [
                Vec::from(["-", "a::Two", "a::One"]),
                Vec::from(["a::Three"])
            ]
        );
    }

    #[test]
    fn an_item_moved_to_another_menu_leaves_its_own() {
        let dragged = moved(BarItem::Divider, Spot { menu: 0, ix: 1 });
        let menus = with_dropped(&bar(), &dragged, Spot { menu: 1, ix: 0 });
        assert_eq!(
            names(&menus.expect("the item is there")),
            [
                Vec::from(["a::One", "a::Two"]),
                Vec::from(["-", "a::Three"])
            ]
        );
    }

    #[test]
    fn an_item_that_moved_in_the_file_meanwhile_isnt_moved() {
        let dragged = moved(action("a::Two"), Spot { menu: 0, ix: 0 });
        assert_eq!(
            with_dropped(&bar(), &dragged, Spot { menu: 1, ix: 0 }),
            None,
            "the item at its spot is another, so the drag is stale"
        );
    }

    #[test]
    fn a_drop_on_a_menu_thats_gone_does_nothing() {
        assert_eq!(
            with_dropped(&bar(), &added(action("a::New")), Spot { menu: 5, ix: 0 }),
            None,
            "there's no sixth menu"
        );
    }

    fn menu_names(menus: &[BarMenu]) -> Vec<&str> {
        menus.iter().map(|menu| menu.name.as_str()).collect()
    }

    fn three_menus() -> Vec<BarMenu> {
        ["File", "View", "Help"]
            .map(|name| BarMenu {
                name: name.to_owned(),
                items: Vec::new(),
            })
            .into()
    }

    fn dragged_menu(ix: usize, name: &str) -> DraggedMenu {
        DraggedMenu {
            ix,
            name: name.to_owned().into(),
        }
    }

    #[test]
    fn a_menu_dropped_on_an_earlier_one_goes_before_it() {
        let menus = with_menu_moved(&three_menus(), &dragged_menu(2, "Help"), 0);
        assert_eq!(
            menu_names(&menus.expect("the menu is there")),
            ["Help", "File", "View"]
        );
    }

    #[test]
    fn a_menu_dropped_on_a_later_one_goes_before_it() {
        let menus = with_menu_moved(&three_menus(), &dragged_menu(0, "File"), 2);
        assert_eq!(
            menu_names(&menus.expect("the menu is there")),
            ["View", "File", "Help"]
        );
    }

    #[test]
    fn a_menu_dropped_past_the_others_goes_last() {
        let menus = with_menu_moved(&three_menus(), &dragged_menu(0, "File"), 3);
        assert_eq!(
            menu_names(&menus.expect("the menu is there")),
            ["View", "Help", "File"]
        );
    }

    #[test]
    fn a_menu_that_moved_in_the_file_meanwhile_isnt_moved() {
        assert!(
            with_menu_moved(&three_menus(), &dragged_menu(0, "Help"), 2).is_none(),
            "the first menu is File, so the drag is stale"
        );
    }

    #[test]
    fn an_item_is_removed_from_its_menu() {
        let menus = without(&bar(), Spot { menu: 0, ix: 1 }, &BarItem::Divider);
        assert_eq!(
            names(&menus.expect("the item is there")),
            [Vec::from(["a::One", "a::Two"]), Vec::from(["a::Three"])]
        );
    }

    #[test]
    fn an_item_that_moved_meanwhile_isnt_removed() {
        assert_eq!(
            without(&bar(), Spot { menu: 0, ix: 0 }, &BarItem::Divider),
            None,
            "the first item is an action, not the divider"
        );
    }
}

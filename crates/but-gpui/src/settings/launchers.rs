//! The commands "New terminal with command" offers, on General: a field for each, dragged by
//! its grip into another order as the navigator's rows are, the others sliding to make room.

use gpui::{
    App, Context, Div, DragMoveEvent, Entity, EntityId, Focusable, Pixels, Point, SharedString,
    Stateful, Subscription, Window, div, prelude::*,
};

use crate::ui::fold::{Reorder, reordered};

use super::{Row, SettingsView};
use crate::state::Store;
use crate::theme::Theme;
use crate::transfer::{drag_pill, pill_label};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, button, dp, field, h_flex, icon_button, small_icon, v_flex};

/// How tall each command's row is, so where the pointer is says which place it's over.
const ROW_HEIGHT: f32 = 36.;

/// The commands as typed until saved, in the order they're offered.
#[derive(Default)]
pub(super) struct Launchers {
    rows: Vec<Launcher>,
    /// The field whose row is being dragged, and the order before, which it goes back to unless
    /// it's let go over the list.
    dragged: Option<(EntityId, Vec<EntityId>)>,
    /// Let go over the list, so the order it shows stays.
    dropped: bool,
    /// The row drawn outlined over the others: the one dragged, until it's done sliding.
    held: Option<EntityId>,
    reorder: Reorder<EntityId>,
}

struct Launcher {
    input: Entity<TextInput>,
    _subscriptions: [Subscription; 2],
}

/// A command's row dragged by its grip, known by its field.
struct LauncherDrag(EntityId);

/// The pill that follows the pointer while a command's row is dragged.
struct LauncherPreview {
    command: SharedString,
    /// Where the pointer pressed the grip, which GPUI draws the pill from.
    offset: Point<Pixels>,
}

impl Render for LauncherPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let mark = small_icon(Icon::TextCode).text_color(theme.pop);
        let command = pill_label(self.command.clone()).font_family(theme.mono_font.clone());
        drag_pill(self.offset, mark, command, Some("Move"), 1, theme)
    }
}

impl SettingsView {
    /// A field for each saved command, in place of what's typed and not saved.
    pub(super) fn launchers(
        store: &Entity<Store>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Launchers {
        let commands = store.read(cx).prefs.terminal_launchers();
        Launchers {
            rows: (commands.into_iter())
                .map(|command| Self::launcher(command, window, cx))
                .collect(),
            ..Launchers::default()
        }
    }

    fn launcher(command: String, window: &mut Window, cx: &mut Context<Self>) -> Launcher {
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Command", false, cx);
            input.load(command, cx);
            input
        });
        let events = cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            InputEvent::Changed => {}
            InputEvent::Submit => {
                this.save_terminal_launchers(cx);
                this.back_to_search(window, cx);
            }
            // What the others hold was saved as they were left.
            InputEvent::Cancel => {
                this.launchers = Self::launchers(&this.store.clone(), window, cx);
                this.back_to_search(window, cx);
            }
        });
        // Each saves as it's left, and all as Settings closes.
        let blur = cx.on_blur(&input.focus_handle(cx), window, |this, _, cx| {
            this.save_terminal_launchers(cx)
        });
        Launcher {
            input,
            _subscriptions: [events, blur],
        }
    }

    /// Save the commands as typed, leaving out empty ones, unless they're what's saved already.
    pub(super) fn save_terminal_launchers(&self, cx: &mut App) {
        let launchers: Vec<String> = (self.launchers.rows.iter())
            .map(|row| row.input.read(cx).text().trim().to_owned())
            .filter(|command| !command.is_empty())
            .collect();
        if launchers == self.store.read(cx).prefs.terminal_launchers() {
            return;
        }
        (self.store).update(cx, |store, cx| {
            store.set_prefs(|p| p.terminal_launchers = Some(launchers), cx)
        });
    }

    /// Focus a new field at the end, or the empty one already there.
    fn add_launcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let empty = (self.launchers.rows.last())
            .filter(|row| row.input.read(cx).text().trim().is_empty())
            .map(|row| row.input.clone());
        let input = empty.unwrap_or_else(|| {
            let row = Self::launcher(String::new(), window, cx);
            let input = row.input.clone();
            self.launchers.rows.push(row);
            input
        });
        window.focus(&input.focus_handle(cx), cx);
        cx.notify();
    }

    fn remove_launcher(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        (self.launchers.rows).retain(|row| row.input.entity_id() != id);
        self.save_terminal_launchers(cx);
        // The button pressed had the keys, and it's gone with its row.
        self.back_to_search(window, cx);
        cx.notify();
    }

    /// Put the rows in `order`, those it moves sliding there from where they were.
    fn order_launchers(&mut self, order: &[EntityId], cx: &mut Context<Self>) {
        let launchers = &mut self.launchers;
        let laid = |rows: &[Launcher]| -> Vec<(Option<EntityId>, f32)> {
            (rows.iter())
                .map(|row| (Some(row.input.entity_id()), ROW_HEIGHT))
                .collect()
        };
        let was = laid(&launchers.rows);
        let at = |row: &Launcher| order.iter().position(|id| *id == row.input.entity_id());
        launchers
            .rows
            .sort_by_key(|row| at(row).unwrap_or(usize::MAX));
        let offsets = reordered(&was, &laid(&launchers.rows));
        launchers.reorder.start(offsets, cx);
        cx.notify();
    }

    /// Show the row dragged at the place the pointer's over, the others making room.
    fn follow_launcher_drag(
        &mut self,
        event: &DragMoveEvent<LauncherDrag>,
        cx: &mut Context<Self>,
    ) {
        let at = event.event.position;
        if !event.bounds.contains(&at) {
            return;
        }
        let dragged = event.drag(cx).0;
        let over = (at.y - event.bounds.top()) / Theme::get(cx).scaled(ROW_HEIGHT);
        let mut order: Vec<EntityId> = (self.launchers.rows.iter())
            .map(|row| row.input.entity_id())
            .filter(|id| *id != dragged)
            .collect();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "floored, at least 0"
        )]
        let place = (over.max(0.) as usize).min(order.len());
        order.insert(place, dragged);
        let shown = (self.launchers.rows.iter()).map(|row| row.input.entity_id());
        if !shown.eq(order.iter().copied()) {
            self.order_launchers(&order, cx);
        }
    }

    /// The drag let go: over the list the order shown stays and saves, elsewhere it goes back.
    fn end_launcher_drag(&mut self, cx: &mut Context<Self>) {
        let dropped = std::mem::take(&mut self.launchers.dropped);
        let Some((_, before)) = self.launchers.dragged.take() else {
            return;
        };
        match dropped {
            true => self.save_terminal_launchers(cx),
            false => self.order_launchers(&before, cx),
        }
        cx.notify();
    }

    /// Go on to the frame being drawn, asking for the next while rows slide.
    pub(super) fn launchers_frame(&mut self, window: &mut Window) {
        self.launchers.reorder.frame();
        if self.launchers.dragged.is_none() && !self.launchers.reorder.any() {
            self.launchers.held = None;
        }
        if self.launchers.reorder.any() {
            window.request_animation_frame();
        }
    }

    /// The commands "New terminal with command" offers.
    pub(super) fn terminal_launchers_row(&self, window: &Window, cx: &Context<Self>) -> Row {
        let mut rows = Vec::new();
        let mut held = None;
        for (ix, row) in self.launchers.rows.iter().enumerate() {
            let id = row.input.entity_id();
            let slid = self.launchers.reorder.offset(&id);
            let el = self.launcher_row(row, window, cx);
            if self.launchers.held == Some(id) {
                // Drawn over the rest, as what moves in the navigator is, its place kept.
                let top = ROW_HEIGHT * ix as f32 + slid;
                held = Some(el.absolute().left_0().right_0().top(dp(top)));
                rows.push(div().h(dp(ROW_HEIGHT)).into_any_element());
            } else {
                rows.push(el.relative().top(dp(slid)).into_any_element());
            }
        }
        let add = button("launcher-add", "Add command")
            .icon(Icon::Plus)
            .style(ButtonStyle::Ghost)
            .small()
            .on_click(cx.listener(|this, _, window, cx| this.add_launcher(window, cx)));
        Row::new(
            "New terminal with command",
            "The commands it offers. Each runs through your shell.",
            div(),
        )
        .with_keywords("claude codex agent command launcher")
        .with_below(
            v_flex()
                .child(
                    v_flex()
                        .relative()
                        .children(rows)
                        .children(held)
                        .on_drag_move(cx.listener(
                            |this, event: &DragMoveEvent<LauncherDrag>, _, cx| {
                                this.follow_launcher_drag(event, cx)
                            },
                        ))
                        .on_drop(cx.listener(|this, _: &LauncherDrag, _, _| {
                            this.launchers.dropped = true
                        })),
                )
                // Under the fields, past the grips.
                .child(div().pl(dp(18.)).pt(dp(3.)).child(add)),
        )
    }

    /// A command's field, its grip before it and its remove button after.
    fn launcher_row(&self, row: &Launcher, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let theme = Theme::get(cx);
        let id = row.input.entity_id();
        let focused = row.input.focus_handle(cx).is_focused(window);
        let view = cx.weak_entity();
        let input = row.input.clone();
        let grip = div()
            .id("grip")
            .flex_none()
            .py(dp(6.))
            .child(small_icon(Icon::DragVertical).text_color(theme.text_3))
            .on_drag(LauncherDrag(id), move |_, offset, _, cx| {
                let command = SharedString::from(input.read(cx).text().to_owned());
                let preview = cx.new(|_| LauncherPreview { command, offset });
                view.update(cx, |this, cx| {
                    let order = (this.launchers.rows.iter())
                        .map(|row| row.input.entity_id())
                        .collect();
                    this.launchers.dragged = Some((id, order));
                    this.launchers.held = Some(id);
                    this.launchers.dropped = false;
                    cx.notify();
                })
                .ok();
                // GPUI drops the pill as the drag ends, wherever it's let go, after the drop.
                let view = view.clone();
                cx.observe_release(&preview, move |_, cx| {
                    view.update(cx, |this, cx| this.end_launcher_drag(cx)).ok();
                })
                .detach();
                preview
            });
        let held = self.launchers.held == Some(id);
        h_flex()
            .id(("launcher", id))
            .h(dp(ROW_HEIGHT))
            .gap(dp(6.))
            // On the card's ground, as it slides over the rows making room.
            .when(held, |el| el.bg(theme.bg_1))
            .child(grip)
            .child(
                field(&row.input, focused, theme)
                    .flex_1()
                    .font_family(theme.mono_font.clone())
                    // Outlined where letting go puts it, as the navigator's rows are while they
                    // move.
                    .when(held, |el| el.border_color(theme.pop).bg(theme.pop_soft)),
            )
            .child(
                icon_button("remove", Icon::Minus)
                    .small()
                    .tooltip("Remove")
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.remove_launcher(id, window, cx)
                        }),
                    ),
            )
    }
}

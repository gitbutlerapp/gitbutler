//! A window of every command and its keys, for a dev build. A key changed here is
//! written to the keymap's file in the checkout and bound at once, so the files stay the one place
//! keys are kept (`keymap::source_dir`). Its Menu bar tab lays out the menu bar the same way.

mod menu_bar;
mod rows;

use std::cell::Cell;
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, Context, CursorStyle, Decorations, Entity, FocusHandle,
    Focusable as _, FontWeight, HighlightStyle, KeyDownEvent, ListAlignment, ListState,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Role, SharedString, StyledText,
    Subscription, Task, TextLayout, TitlebarOptions, Toggled, Window, WindowBounds, WindowHandle,
    WindowOptions, actions, canvas, div, list, point, size,
};

use crate::app::CloseWindow;
use crate::clipboard;
use crate::commands::{COMMANDS, title_of};
use crate::keymap::edit::{self, Bound};
use crate::keymap::{self, Binding, Keymap, KeymapFile, Loaded};
use crate::state::Store;
use crate::theme::{Highlight, Theme};
use crate::ui::modal::{ModalSize, modal};
use crate::ui::scrollbar::{Scroll, scrolled};
use crate::ui::text_input::{self, InputEvent, TextInput};
use crate::ui::{
    self, ButtonStyle, Icon, Tooltip, dp, field, fixed, h_flex, icon, icon_button, list_row,
    segmented, v_flex,
};
use menu_bar::MenuBarEditor;
use rows::{AuditFilter, Command, Filter, Key, Place, Row, Sources};

actions!(command_editor, [OpenCommandEditor]);

/// How long a key waits for a second one, which makes it a chord, before it's saved.
const CHORD_PAUSE: Duration = Duration::from_millis(800);

/// Actions that aren't anyone's to bind, unless a keymap does.
const UNLISTED: [&str; 3] = ["text_input::", "zed::", "stories::"];

const ROW_HEIGHT: f32 = 34.;
const AUDIT_WIDTH: f32 = 26.;
const COMMAND_WIDTH: f32 = 270.;
const ACTION_WIDTH: f32 = 270.;
const PLACE_WIDTH: f32 = 270.;
const SIDE_PAD: f32 = 12.;
/// What the keys' column keeps as the window narrows: the columns of text give way first.
const KEYS_MIN_WIDTH: f32 = 190.;
/// The table's small text: what's mono, and the notes beside a key.
const SMALL_TEXT: f32 = 13.;

/// The window, once it's been opened.
struct EditorWindow(WindowHandle<CommandEditor>);

impl gpui::Global for EditorWindow {}

/// Open the window, or bring it in front if it's open.
pub fn open(store: Entity<Store>, cx: &mut App) {
    let scripted = crate::app::scripted();
    if let Some(handle) = cx.try_global::<EditorWindow>().map(|editor| editor.0) {
        let shown = handle.update(cx, |_, window, _| {
            if !scripted {
                window.activate_window();
            }
        });
        if shown.is_ok() {
            return;
        }
    }
    let default_size = size(fixed(1360.), fixed(860.));
    // As the main window's: out of the way of a script's run.
    let bounds = match scripted {
        true => Bounds::new(point(fixed(-10000.), fixed(-10000.)), default_size),
        false => Bounds::centered(None, default_size, cx),
    };
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Commands".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(fixed(1100.), fixed(500.))),
            focus: !scripted,
            show: !scripted,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| CommandEditor::new(store, window, cx)),
    );
    match window {
        Ok(handle) => cx.set_global(EditorWindow(handle)),
        Err(err) => tracing::error!("couldn't open the command editor: {err:#}"),
    }
}

/// What keys being recorded will go to.
#[derive(Clone, PartialEq)]
enum Target {
    /// In place of these.
    Change(Binding),
    /// Beside any others, in the keymap's own file.
    Add {
        action: String,
        context: Option<String>,
    },
}

struct Recording {
    target: Target,
    strokes: Vec<gpui::Keystroke>,
    /// Which of `keymap::PLATFORMS` a new key is for. A key being changed stays on its own.
    platforms: [bool; 3],
    /// Takes the keys from everything else while it lasts.
    _intercept: Subscription,
    _pause: Task<()>,
}

/// Which place a command is getting, and whether to go on to record keys for it.
struct Picking {
    action: String,
    title: String,
    record: bool,
}

enum Notice {
    Done(String),
    Warning(String),
}

/// The part of an action's name that's selected in the table.
struct ActionSelection {
    /// Whose name, by `Row::audit_key`.
    row: String,
    text: SharedString,
    /// The text as it's laid out, for where the pointer goes while it's held.
    layout: TextLayout,
    /// Where it started and where it's got to, in bytes.
    anchor: usize,
    head: usize,
    /// The button that started it is still down.
    held: bool,
}

/// What's between two points of a selection, whichever way round they are.
fn between(anchor: usize, head: usize) -> Range<usize> {
    anchor.min(head)..anchor.max(head)
}

/// What the window shows.
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Keys,
    MenuBar,
}

impl Tab {
    const ALL: [Tab; 2] = [Tab::Keys, Tab::MenuBar];

    fn label(self) -> &'static str {
        match self {
            Tab::Keys => "Keys",
            Tab::MenuBar => "Menu bar",
        }
    }
}

/// What the table shows, in order.
enum Line {
    Heading(SharedString),
    /// An index into the rows, whether it goes on the command of the line before, and whether
    /// it has the ground of every other command.
    Row {
        row: usize,
        repeats: bool,
        striped: bool,
    },
}

pub struct CommandEditor {
    store: Entity<Store>,
    focus: FocusHandle,
    tab: Tab,
    menu_bar: Entity<MenuBarEditor>,
    search: Entity<TextInput>,
    /// The key pressed to find what's bound to it, as keys are written.
    found_keys: Option<SharedString>,
    /// Waiting for that key: takes the keys from everything else while it lasts.
    finding: Option<Subscription>,
    filter: Filter,
    audit_filter: AuditFilter,
    rows: Vec<Row>,
    lines: Vec<Line>,
    list: ListState,
    /// Places given to commands with [`Picking`], until keys are bound there.
    places: Vec<(String, Option<String>)>,
    recording: Option<Recording>,
    picking: Option<Picking>,
    notice: Option<Notice>,
    selection: Option<ActionSelection>,
    /// Where the platforms to record a new key for are drawn, where a press doesn't end it.
    platforms_at: Rc<Cell<Option<Bounds<Pixels>>>>,
    _subscriptions: Vec<Subscription>,
}

impl CommandEditor {
    fn new(
        store: Entity<Store>,
        window: &mut Window,
        cx: &mut Context<CommandEditor>,
    ) -> CommandEditor {
        let search = cx.new(|cx| TextInput::new("Find by title, action, place or keys", false, cx));
        let subscriptions = Vec::from([
            cx.subscribe_in(&search, window, |this, search, event, _, cx| match event {
                InputEvent::Changed => this.show_rows(false, cx),
                InputEvent::Cancel if this.picking.is_some() => {
                    this.picking = None;
                    cx.notify();
                }
                InputEvent::Cancel => search.update(cx, |search, cx| search.set_text("", cx)),
                InputEvent::Submit => {}
            }),
            cx.observe_global::<Theme>(|_, cx| cx.notify()),
            cx.observe_global::<Loaded>(|this, cx| this.load_rows(cx)),
            // A key pressed in another app isn't a key for this.
            cx.observe_window_activation(window, |this, window, cx| {
                match window.is_window_active() {
                    true => keymap::reload(cx),
                    false => this.stop_recording(cx),
                }
            }),
        ]);
        window.focus(&search.read(cx).focus_handle(cx), cx);
        let commands = commands(&keymap::bindings(cx), cx);
        let menu_bar = cx.new(|cx| MenuBarEditor::new(commands, window, cx));
        let mut this = CommandEditor {
            store,
            focus: cx.focus_handle(),
            tab: Tab::Keys,
            menu_bar,
            search,
            found_keys: None,
            finding: None,
            filter: Filter::All,
            audit_filter: AuditFilter::Any,
            rows: Vec::new(),
            lines: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, fixed(400.)),
            places: Vec::new(),
            recording: None,
            picking: None,
            notice: None,
            selection: None,
            platforms_at: Rc::default(),
            _subscriptions: subscriptions,
        };
        this.load_rows(cx);
        this
    }

    /// Make the rows from what the files say now, and stay where the table is.
    fn load_rows(&mut self, cx: &mut Context<CommandEditor>) {
        self.selection = None;
        let bindings = keymap::bindings(cx);
        let commands = commands(&bindings, cx);
        let text_keys = text_input::bound_keys(cx);
        self.rows = rows::rows(&Sources {
            commands: &commands,
            bindings: &bindings,
            text_keys: &text_keys,
            places: &self.places,
            audited: &self.store.read(cx).prefs.audited_commands,
            os: std::env::consts::OS,
        });
        self.show_rows(true, cx);
    }

    /// Show the rows the filter and the search leave, from the top unless `keep_place`.
    fn show_rows(&mut self, keep_place: bool, cx: &mut Context<CommandEditor>) {
        let query = self.search.read(cx).text().to_lowercase();
        let mut lines = Vec::new();
        let mut last: Option<&Row> = None;
        let mut striped = true;
        for (ix, row) in self.rows.iter().enumerate() {
            let kept = self.filter.keeps(row)
                && self.audit_filter.keeps(row)
                && (self.found_keys.as_ref()).is_none_or(|keys| row.bound_to(keys));
            if !kept || !row.found_by(&query) {
                continue;
            }
            if last.is_none_or(|last| last.group != row.group) {
                lines.push(Line::Heading(row.group.clone().into()));
                striped = true;
            }
            let repeats =
                last.is_some_and(|last| last.action == row.action && last.title == row.title);
            striped ^= !repeats;
            lines.push(Line::Row {
                row: ix,
                repeats,
                striped,
            });
            last = Some(row);
        }
        self.lines = lines;
        let place = self.list.logical_scroll_top();
        self.list.reset(self.lines.len());
        if keep_place {
            self.list.scroll_to(place);
        }
        cx.notify();
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<CommandEditor>) {
        self.stop_recording(cx);
        self.picking = None;
        self.tab = tab;
        cx.notify();
    }

    fn set_filter(&mut self, filter: Filter, cx: &mut Context<CommandEditor>) {
        self.filter = filter;
        self.show_rows(false, cx);
    }

    fn set_audit_filter(&mut self, filter: AuditFilter, cx: &mut Context<CommandEditor>) {
        self.audit_filter = filter;
        self.show_rows(false, cx);
    }

    /// Check or uncheck a command as audited. The filter can take it off the table at once.
    fn toggle_audited(&mut self, key: &str, cx: &mut Context<CommandEditor>) {
        let audited = !(self.rows.iter()).any(|row| row.audit_key == key && row.audited);
        self.store.update(cx, |store, cx| {
            store.set_prefs(
                |prefs| match audited {
                    true => {
                        prefs.audited_commands.insert(key.to_owned());
                    }
                    false => {
                        prefs.audited_commands.remove(key);
                    }
                },
                cx,
            )
        });
        for row in self.rows.iter_mut().filter(|row| row.audit_key == key) {
            row.audited = audited;
        }
        self.show_rows(true, cx);
    }

    fn start_recording(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<CommandEditor>,
    ) {
        let intercept = intercept_keys(CommandEditor::take_key, window, cx);
        self.notice = None;
        self.recording = Some(Recording {
            target,
            strokes: Vec::new(),
            platforms: [true; 3],
            _intercept: intercept,
            _pause: Task::ready(()),
        });
        cx.notify();
    }

    /// Have a new key bind on the platform `ix` of `keymap::PLATFORMS` or not.
    fn toggle_platform(&mut self, ix: usize, cx: &mut Context<CommandEditor>) {
        if let Some(recording) = self.recording.as_mut() {
            recording.platforms = toggled(recording.platforms, ix);
            cx.notify();
        }
    }

    /// A press anywhere but on the platforms ends recording.
    fn press_while_recording(&mut self, event: &MouseDownEvent, cx: &mut Context<CommandEditor>) {
        let on_platforms =
            (self.platforms_at.get()).is_some_and(|bounds| bounds.contains(&event.position));
        let adding = (self.recording.as_ref())
            .is_some_and(|recording| matches!(recording.target, Target::Add { .. }));
        if !(adding && on_platforms) {
            self.stop_recording(cx);
        }
    }

    fn stop_recording(&mut self, cx: &mut Context<CommandEditor>) {
        let finding = self.finding.take();
        if self.recording.take().is_some() || finding.is_some() {
            cx.notify();
        }
    }

    /// Wait for a key to find what's bound to it.
    fn start_finding(&mut self, window: &mut Window, cx: &mut Context<CommandEditor>) {
        self.finding = Some(intercept_keys(CommandEditor::find_key, window, cx));
        cx.notify();
    }

    fn find_key(&mut self, keystroke: gpui::Keystroke, cx: &mut Context<CommandEditor>) {
        self.finding = None;
        let keys = edit::written(&keystroke, None, std::env::consts::OS);
        self.found_keys = Some(ui::keys_written(&keys));
        self.show_rows(false, cx);
    }

    fn clear_found_keys(&mut self, cx: &mut Context<CommandEditor>) {
        self.found_keys = None;
        self.show_rows(false, cx);
    }

    /// A key while recording: the first waits for a second, which ends it. After the key has
    /// gone, as saving binds all the keys again.
    fn take_key(&mut self, keystroke: gpui::Keystroke, cx: &mut Context<CommandEditor>) {
        let Some(recording) = &mut self.recording else {
            return;
        };
        recording.strokes.push(keystroke);
        let chord = recording.strokes.len() > 1;
        recording._pause = cx.spawn(async move |this, cx| {
            if !chord {
                cx.background_executor().timer(CHORD_PAUSE).await;
            }
            this.update(cx, |this, cx| this.save_recording(cx)).ok();
        });
        cx.notify();
    }

    fn save_recording(&mut self, cx: &mut Context<CommandEditor>) {
        let Some(Recording {
            target,
            strokes,
            platforms,
            ..
        }) = self.recording.take()
        else {
            return;
        };
        cx.notify();
        let os = match &target {
            Target::Change(binding) => binding.os.clone(),
            Target::Add { .. } => chosen_platforms(platforms),
        };
        let keys: Vec<String> = (strokes.iter())
            .map(|stroke| edit::written(stroke, os.as_deref(), std::env::consts::OS))
            .collect();
        let keys = keys.join(" ");
        let label = ui::keys_written(&keys);
        if let Some(platform) = keymap::kept_by_platform(&keys, os.as_deref()) {
            let name = keymap::platform_name(platform);
            self.notice = Some(Notice::Warning(format!("{label} belongs to {name}")));
            return;
        }
        let (action, context, file) = match &target {
            Target::Change(binding) => (&binding.action, &binding.context, binding.file),
            Target::Add { action, context } => (action, context, Keymap::default().own_file()),
        };
        let bindings = keymap::bindings(cx);
        if already_bound(&bindings, file, action, context, &keys, os.as_deref()) {
            if !matches!(&target, Target::Change(binding) if binding.keys == keys) {
                self.notice = Some(Notice::Warning(format!("{label} already does this here")));
            }
            return;
        }
        let saved = match &target {
            Target::Change(binding) => keymap::edit(
                binding.file,
                |text| {
                    let bound = Bound {
                        section: binding.section,
                        keys: &binding.keys,
                        action: &binding.action,
                        context: binding.context.as_deref(),
                        os: binding.os.as_deref(),
                    };
                    edit::rekey(text, &bound, &keys)
                },
                cx,
            ),
            Target::Add { action, context } => keymap::edit(
                Keymap::default().own_file(),
                |text| edit::add(text, &keys, action, context.as_deref(), os.as_deref()),
                cx,
            ),
        };
        let action = action.clone();
        self.load_rows(cx);
        self.notice = Some(match saved {
            Err(err) => Notice::Warning(format!("Couldn't save: {err:#}")),
            Ok(()) => self.saved_notice(&action, &keys, &label),
        });
    }

    /// What's been bound, or what else its keys do where it's bound.
    fn saved_notice(&self, action: &str, keys: &str, label: &str) -> Notice {
        let row = (self.rows.iter()).find(|row| row.action == action);
        let conflicts: Vec<&str> = (self.rows.iter())
            .filter(|row| row.action == action)
            .flat_map(|row| row.keys.iter())
            .filter(|key| (key.binding.as_ref()).is_some_and(|binding| binding.keys == keys))
            .flat_map(|key| key.conflicts.iter().map(String::as_str))
            .collect();
        match conflicts.is_empty() {
            true => Notice::Done(format!(
                "{label} saved for {}",
                row.map_or(action, |row| row.title.as_str())
            )),
            false => Notice::Warning(format!("{label} also runs {}", conflicts.join(", "))),
        }
    }

    fn remove(&mut self, binding: Binding, cx: &mut Context<CommandEditor>) {
        let saved = keymap::edit(
            binding.file,
            |text| edit::remove(text, binding.section, &binding.keys),
            cx,
        );
        self.load_rows(cx);
        let label = ui::keys_written(&binding.keys);
        self.notice = Some(match saved {
            Ok(()) => Notice::Done(format!("{label} removed")),
            Err(err) => Notice::Warning(format!("Couldn't save: {err:#}")),
        });
        cx.notify();
    }

    /// Give the command being picked for `context`, and go on to record its keys if asked to.
    fn pick_place(
        &mut self,
        context: Option<String>,
        window: &mut Window,
        cx: &mut Context<CommandEditor>,
    ) {
        let Some(picking) = self.picking.take() else {
            return;
        };
        let place = (picking.action.clone(), context.clone());
        if !self.places.contains(&place) {
            self.places.push(place);
        }
        self.load_rows(cx);
        if picking.record {
            let target = Target::Add {
                action: picking.action,
                context,
            };
            self.start_recording(target, window, cx);
        }
    }

    fn render_toolbar(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<CommandEditor>,
    ) -> impl IntoElement {
        let focused = self.search.read(cx).focus_handle(cx).is_focused(window);
        let filters = Filter::ALL.map(|filter| {
            let label = format!("{} {}", filter.label(), filter.count(&self.rows));
            let segment = div()
                .id(filter.label())
                .aria_label(label.clone())
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.set_filter(filter, cx)));
            (segment, self.filter == filter)
        });
        let audit_filters = AuditFilter::ALL.map(|filter| {
            let label = match filter {
                AuditFilter::Any => filter.label().to_owned(),
                AuditFilter::Audited | AuditFilter::NotAudited => {
                    format!("{} {}", filter.label(), filter.count(&self.rows))
                }
            };
            let segment = div()
                .id(filter.label())
                .aria_label(label.clone())
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.set_audit_filter(filter, cx)));
            (segment, self.audit_filter == filter)
        });
        let tabs = Tab::ALL.map(|tab| {
            let segment = div()
                .id(tab.label())
                .aria_label(tab.label())
                .child(tab.label())
                .on_click(cx.listener(move |this, _, _, cx| this.set_tab(tab, cx)));
            (segment, self.tab == tab)
        });
        let bar = h_flex()
            .flex_none()
            .gap(dp(12.))
            .px(dp(SIDE_PAD))
            .py(dp(10.))
            .child(segmented(true, theme, tabs));
        match self.tab {
            Tab::MenuBar => bar.border_b_1().border_color(theme.border_2),
            Tab::Keys => bar
                .child(div().flex_1().max_w(dp(300.)).min_w_0().child(field(
                    &self.search,
                    focused,
                    theme,
                )))
                .child(self.render_key_finder(theme, cx))
                .child(segmented(true, theme, filters))
                .child(segmented(true, theme, audit_filters))
                .child(div().flex_1())
                .child(
                    h_flex()
                        .gap(dp(5.))
                        .text_size(dp(SMALL_TEXT))
                        .child(ui::dot(theme.pop))
                        .child("Shown in tooltips and right-click menus"),
                ),
        }
    }

    /// Where a key is pressed to find what's bound to it, with that key once it has been.
    fn render_key_finder(
        &self,
        theme: &Theme,
        cx: &mut Context<CommandEditor>,
    ) -> impl IntoElement {
        let finding = self.finding.is_some();
        let found = self.found_keys.as_ref().filter(|_| !finding);
        let press = ui::tab_stop(div().id("find-keys"), theme)
            .role(Role::Button)
            .aria_label("Find by pressing a key")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .px(dp(8.))
            .on_click(cx.listener(|this, _, window, cx| this.start_finding(window, cx)));
        let press = match (finding, found) {
            (true, _) => press.child("Press a key…"),
            (false, Some(keys)) => press.child(ui::keycap_large(keys, theme)),
            (false, None) => press
                .text_color(theme.text_3)
                .child("Find by pressing a key"),
        };
        h_flex()
            .flex_none()
            .w(dp(190.))
            .h(dp(30.))
            .pr(dp(3.))
            .rounded(dp(theme.shape.control))
            .border_1()
            .border_color(if finding { theme.pop } else { theme.border_2 })
            .bg(theme.bg_1)
            .text_size(dp(13.))
            .child(press)
            .children(found.map(|_| {
                icon_button("clear-found-keys", Icon::Cross)
                    .small()
                    .style(ButtonStyle::Ghost)
                    .tooltip("Show every key")
                    .on_click(cx.listener(|this, _, _, cx| this.clear_found_keys(cx)))
            }))
    }

    fn render_header(&self, theme: &Theme) -> impl IntoElement {
        h_flex()
            .flex_none()
            .h(dp(30.))
            .px(dp(SIDE_PAD))
            .border_b_1()
            .border_color(theme.border_2)
            .child(div().flex_none().w(dp(AUDIT_WIDTH)))
            .child(text_column(COMMAND_WIDTH).child(table_heading("Command")))
            .child(text_column(ACTION_WIDTH).child(table_heading("Action")))
            .child(text_column(PLACE_WIDTH).child(table_heading("Where")))
            .child(keys_column().child(table_heading("Keys")))
    }

    fn render_line(&self, ix: usize, cx: &mut Context<CommandEditor>) -> AnyElement {
        let theme = Theme::get(cx).clone();
        match self.lines.get(ix) {
            Some(Line::Heading(group)) => h_flex()
                .h(dp(ROW_HEIGHT + 6.))
                .items_end()
                .pb(dp(4.))
                .px(dp(SIDE_PAD))
                .child(table_heading(group.clone()))
                .into_any_element(),
            Some(Line::Row {
                row,
                repeats,
                striped,
            }) => match self.rows.get(*row) {
                Some(row_data) => self.render_row(*row, row_data, *repeats, *striped, &theme, cx),
                None => div().into_any_element(),
            },
            None => div().into_any_element(),
        }
    }

    /// Start selecting `row`'s action's name, or take all of it on a double click.
    fn press_action(
        &mut self,
        row: String,
        text: SharedString,
        layout: TextLayout,
        event: &MouseDownEvent,
        cx: &mut Context<CommandEditor>,
    ) {
        let at = ui::text_index(&layout, &text, event.position);
        let (anchor, head, held) = match event.click_count {
            1 => (at, at, true),
            _ => (0, text.len(), false),
        };
        self.selection = Some(ActionSelection {
            row,
            text,
            layout,
            anchor,
            head,
            held,
        });
        cx.notify();
    }

    /// Follow the pointer while the button's down, and stop once it's not.
    fn drag_action(&mut self, event: &MouseMoveEvent, cx: &mut Context<CommandEditor>) {
        let Some(selection) = self.selection.as_mut().filter(|selection| selection.held) else {
            return;
        };
        match event.pressed_button {
            Some(MouseButton::Left) => {
                selection.head = ui::text_index(&selection.layout, &selection.text, event.position);
            }
            Some(_) | None => selection.held = false,
        }
        cx.notify();
    }

    fn release_action(&mut self, cx: &mut Context<CommandEditor>) {
        if let Some(selection) = self.selection.as_mut() {
            selection.held = false;
            cx.notify();
        }
    }

    /// Copy what's selected, on ⌘C. A text field that has the keys takes its own ⌘C first, but a
    /// press on a name takes the keys from it.
    fn copy_action(&mut self, event: &KeyDownEvent, cx: &mut Context<CommandEditor>) {
        let keystroke = &event.keystroke;
        if keystroke.key != "c" || !keystroke.modifiers.secondary() {
            return;
        }
        let selected = (self.selection.as_ref()).and_then(|selection| {
            selection
                .text
                .get(between(selection.anchor, selection.head))
                .filter(|text| !text.is_empty())
        });
        if let Some(selected) = selected {
            // Said in this window, which has no snackbar.
            clipboard::copy_silently(selected.to_owned(), cx);
            self.notice = Some(Notice::Done(clipboard::copied(selected)));
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// A row's action's name, as text that can be selected.
    fn action_text(
        &self,
        ix: usize,
        row: &Row,
        theme: &Theme,
        cx: &mut Context<CommandEditor>,
    ) -> impl IntoElement + use<> {
        let text: SharedString = row.action.clone().into();
        let selected = (self.selection.as_ref())
            .filter(|selection| selection.row == row.audit_key)
            .map(|selection| between(selection.anchor, selection.head))
            .filter(|range| !range.is_empty());
        let highlight = HighlightStyle {
            background_color: Some(theme.highlight(Highlight::Selected)),
            ..HighlightStyle::default()
        };
        let styled = StyledText::new(text.clone())
            .with_highlights(selected.into_iter().map(|range| (range, highlight)));
        let layout = styled.layout().clone();
        let key = row.audit_key.clone();
        div()
            .id(("action-name", ix))
            .min_w_0()
            .truncate()
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.press_action(key.clone(), text.clone(), layout.clone(), event, cx)
                }),
            )
            .child(styled)
    }

    fn render_row(
        &self,
        ix: usize,
        row: &Row,
        repeats: bool,
        striped: bool,
        theme: &Theme,
        cx: &mut Context<CommandEditor>,
    ) -> AnyElement {
        let place = match &row.place {
            Place::In(context) => {
                let text = context.clone().unwrap_or_else(|| "Anywhere".to_owned());
                text_column(PLACE_WIDTH)
                    .flex()
                    .items_center()
                    .gap(dp(4.))
                    .font_family(theme.mono_font.clone())
                    .text_size(dp(SMALL_TEXT))
                    .when(row.locked(), |el| {
                        el.child(
                            icon(Icon::Lock)
                                .size(dp(12.))
                                .flex_none()
                                .text_color(theme.text_1),
                        )
                    })
                    .child(div().min_w_0().truncate().child(text))
            }
            Place::Unbound => text_column(PLACE_WIDTH)
                .text_size(dp(SMALL_TEXT))
                .child("Nowhere"),
        };
        let add_place = (!repeats && !row.locked() && row.place != Place::Unbound).then(|| {
            let (action, title) = (row.action.clone(), row.title.clone());
            div()
                .flex_none()
                .invisible()
                .group_hover("row", |el| el.visible())
                .child(
                    icon_button("add-place", Icon::Plus)
                        .small()
                        .style(ButtonStyle::Ghost)
                        .tooltip("Bind it in another place")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.picking = Some(Picking {
                                action: action.clone(),
                                title: title.clone(),
                                record: false,
                            });
                            cx.notify();
                        })),
                )
        });
        let audit_key = row.audit_key.clone();
        let check = match row.audited {
            true => Toggled::True,
            false => Toggled::False,
        };
        // On a command's first row: it's the command that's audited, not each place.
        let audit = h_flex()
            .flex_none()
            .w(dp(AUDIT_WIDTH))
            .when(!repeats, |el| {
                el.child(
                    ui::checkbox("audited", check, theme)
                        .aria_label(format!("{} audited", row.title))
                        .tooltip(Tooltip::text("Audited"))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_audited(&audit_key, cx)),
                        ),
                )
            });
        let command = text_column(COMMAND_WIDTH)
            .flex()
            .items_center()
            .gap(dp(4.))
            .when(!repeats, |el| {
                el.child(div().min_w_0().truncate().child(row.title.clone()))
            })
            .children(add_place);
        let action = text_column(ACTION_WIDTH)
            .truncate()
            .font_family(theme.mono_font.clone())
            .text_size(dp(SMALL_TEXT))
            .when(!repeats, |el| {
                el.child(self.action_text(ix, row, theme, cx))
            });
        h_flex()
            .id(("row", ix))
            .group("row")
            .w_full()
            .min_h(dp(ROW_HEIGHT))
            .px(dp(SIDE_PAD))
            .py(dp(3.))
            .when(striped, |el| el.bg(theme.hover))
            .hover(|el| el.bg(theme.selected_unfocused))
            .child(audit)
            .child(command)
            .child(action)
            .child(place)
            .child(self.render_cell(row, theme, cx))
            .into_any_element()
    }

    fn render_cell(
        &self,
        row: &Row,
        theme: &Theme,
        cx: &mut Context<CommandEditor>,
    ) -> impl IntoElement + use<> {
        let keys: Vec<AnyElement> = (row.keys.iter().enumerate())
            .map(|(ix, key)| self.render_key(key, ix, theme, cx))
            .collect();
        let empty = keys.is_empty();
        let adding = match &row.place {
            Place::In(context) => self.recording.as_ref().filter(|recording| {
                recording.target
                    == Target::Add {
                        action: row.action.clone(),
                        context: context.clone(),
                    }
            }),
            Place::Unbound => None,
        };
        let add = (!row.locked() && adding.is_none()).then(|| {
            let (action, title, place) = (row.action.clone(), row.title.clone(), row.place.clone());
            div()
                .flex_none()
                .when(!empty, |el| {
                    el.invisible().group_hover("row", |el| el.visible())
                })
                .child(
                    icon_button("add-key", Icon::Plus)
                        .small()
                        .style(ButtonStyle::Ghost)
                        .tooltip("Add a key")
                        .on_click(cx.listener(move |this, _, window, cx| match &place {
                            Place::In(context) => this.start_recording(
                                Target::Add {
                                    action: action.clone(),
                                    context: context.clone(),
                                },
                                window,
                                cx,
                            ),
                            Place::Unbound => {
                                this.picking = Some(Picking {
                                    action: action.clone(),
                                    title: title.clone(),
                                    record: true,
                                });
                                cx.notify();
                            }
                        })),
                )
        });
        // The add button stays out of the wrap: hidden, it would still take a line of its own.
        let wrapped = (!empty || adding.is_some()).then(|| {
            h_flex()
                .min_w_0()
                .flex_wrap()
                .gap(dp(4.))
                .children(keys)
                .children(adding.map(|recording| recording_cap(recording, theme)))
        });
        keys_column()
            .flex()
            .items_center()
            .gap(dp(4.))
            .overflow_hidden()
            .children(wrapped)
            .children(add)
    }

    fn render_key(
        &self,
        key: &Key,
        ix: usize,
        theme: &Theme,
        cx: &mut Context<CommandEditor>,
    ) -> AnyElement {
        let Some(binding) = key.binding.clone() else {
            return h_flex()
                .flex_none()
                .id(("locked-key", ix))
                .mr(dp(8.))
                .tooltip(Tooltip::text("Bound in ui::text_input"))
                .child(ui::keycap_large(&key.label, theme))
                .into_any_element();
        };
        let target = Target::Change(binding.clone());
        if let Some(recording) =
            (self.recording.as_ref()).filter(|recording| recording.target == target)
        {
            return recording_cap(recording, theme).into_any_element();
        }
        let conflicted = !key.conflicts.is_empty();
        let tooltip = match conflicted {
            true => format!("Also runs {}", key.conflicts.join(", ")),
            false => "Change these keys".to_owned(),
        };
        let cap = h_flex()
            .id(("key", ix))
            .role(Role::Button)
            .aria_label(format!("Change {}", key.label))
            .h(dp(28.))
            .px(dp(3.))
            .gap(dp(4.))
            .rounded(dp(theme.shape.control))
            .border_1()
            .border_color(match conflicted {
                true => theme.warn,
                false => gpui::transparent_black(),
            })
            .hover(|el| el.bg(theme.selected_unfocused))
            .text_size(dp(SMALL_TEXT))
            .tooltip(Tooltip::text(tooltip))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.start_recording(target.clone(), window, cx)
            }))
            .child(ui::keycap_large(&key.label, theme))
            .when(key.shown, |el| el.child(ui::dot(theme.pop)))
            .when(key.shared(), |el| el.child("shared"))
            .when_some(key.platforms.clone(), |el, platforms| el.child(platforms));
        let remove = div()
            .id(("remove", ix))
            .role(Role::Button)
            .aria_label(format!("Remove {}", key.label))
            .flex_none()
            .invisible()
            .group_hover("key", |el| el.visible())
            .tooltip(Tooltip::text("Remove these keys"))
            .on_click(cx.listener(move |this, _, _, cx| this.remove(binding.clone(), cx)))
            .child(icon(Icon::Cross).size(dp(12.)).text_color(theme.text_1));
        h_flex()
            .group("key")
            .flex_none()
            .gap(dp(1.))
            .child(cap)
            .child(remove)
            .into_any_element()
    }

    /// The platforms a key being added is for, to tick while it's recorded.
    fn render_platforms(
        &self,
        theme: &Theme,
        cx: &mut Context<CommandEditor>,
    ) -> Option<impl IntoElement + use<>> {
        let recording = self.recording.as_ref()?;
        let Target::Add { .. } = recording.target else {
            return None;
        };
        let at = self.platforms_at.clone();
        let boxes = (keymap::PLATFORMS
            .iter()
            .zip(recording.platforms)
            .enumerate())
        .map(|(ix, (name, on))| {
            let check = match on {
                true => Toggled::True,
                false => Toggled::False,
            };
            let name = keymap::platform_name(name);
            h_flex()
                .id(("platform", ix))
                .role(Role::CheckBox)
                .aria_label(format!("Bind on {name}"))
                .gap(dp(5.))
                .child(ui::checkbox(("platform-box", ix), check, theme))
                .child(name.to_owned())
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_platform(ix, cx)))
        })
        .collect::<Vec<_>>();
        Some(
            h_flex()
                .relative()
                .flex_none()
                .gap(dp(12.))
                .child(
                    canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .children(boxes),
        )
    }

    fn render_footer(&self, theme: &Theme, cx: &mut Context<CommandEditor>) -> impl IntoElement {
        let errors = (cx.try_global::<Loaded>()).map_or(Vec::new(), |loaded| loaded.errors.clone());
        let notice = match self.tab {
            Tab::Keys => self.notice.as_ref(),
            Tab::MenuBar => None,
        };
        let menu_warning = (self.tab == Tab::MenuBar)
            .then(|| self.menu_bar.read(cx).warning.clone())
            .flatten();
        let (text, color) = match (&self.recording, errors.first(), notice) {
            _ if self.finding.is_some() => (
                "Press a key to find what's bound to it. Click elsewhere to cancel.".to_owned(),
                theme.text_1,
            ),
            (Some(_), _, _) => (
                "Press the keys, two for a chord. Click elsewhere to cancel.".to_owned(),
                theme.text_1,
            ),
            (None, Some(error), _) => (error.clone(), theme.danger),
            (None, None, Some(Notice::Warning(text))) => (text.clone(), theme.warn),
            (None, None, Some(Notice::Done(text))) => (text.clone(), theme.text_1),
            (None, None, None) => match (self.tab, menu_warning) {
                (Tab::MenuBar, Some(warning)) => (warning, theme.warn),
                (Tab::MenuBar, None) => (
                    "Drag commands into menus and around them. Changes are saved as you make them."
                        .to_owned(),
                    theme.text_1,
                ),
                (Tab::Keys, _) => (
                    "Click keys to change them. Changes are saved as you make them.".to_owned(),
                    theme.text_1,
                ),
            },
        };
        let platforms = self.render_platforms(theme, cx);
        // They take its place, as they're only there for the moment.
        let folder = (keymap::source_dir())
            .filter(|_| platforms.is_none())
            .map(|dir| dir.display().to_string());
        h_flex()
            .flex_none()
            .h(dp(30.))
            .px(dp(SIDE_PAD))
            .gap(dp(12.))
            .border_t_1()
            .border_color(theme.border_2)
            .text_size(dp(SMALL_TEXT))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(color)
                    .child(text),
            )
            .children(platforms)
            .children(folder.map(|folder| div().flex_none().child(folder)))
    }

    fn render_picker(
        &self,
        theme: &Theme,
        cx: &mut Context<CommandEditor>,
    ) -> Option<impl IntoElement + use<>> {
        let picking = self.picking.as_ref()?;
        let mut contexts: Vec<Option<String>> = (keymap::bindings(cx).into_iter())
            .map(|binding| binding.context)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        contexts.sort();
        let rows = contexts.into_iter().enumerate().map(|(ix, context)| {
            let label = context.clone().unwrap_or_else(|| "Anywhere".to_owned());
            list_row(("place", ix), false, theme)
                .h(dp(26.))
                .px(dp(12.))
                .font_family(theme.mono_font.clone())
                .text_size(dp(SMALL_TEXT))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.pick_place(context.clone(), window, cx)
                }))
        });
        let card = modal(ModalSize::Medium, theme)
            .max_h(dp(560.))
            .child(
                div()
                    .flex_none()
                    .p(dp(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("Where should {} work?", picking.title)),
            )
            .child(
                v_flex()
                    .id("places")
                    .flex_1()
                    .min_h_0()
                    .pb(dp(8.))
                    .overflow_y_scroll()
                    .children(rows),
            );
        Some(
            div()
                .id("picker")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(dp(24.))
                .bg(theme.overlay)
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.picking = None;
                    cx.notify();
                }))
                .child(
                    card.id("picker-card")
                        .on_click(|_, _, cx| cx.stop_propagation()),
                ),
        )
    }
}

/// Have `take` get every key pressed in `window`, and nothing else, while what's returned lasts.
fn intercept_keys(
    take: fn(&mut CommandEditor, gpui::Keystroke, &mut Context<CommandEditor>),
    window: &Window,
    cx: &mut Context<CommandEditor>,
) -> Subscription {
    let this = cx.entity().downgrade();
    let here = window.window_handle().window_id();
    cx.intercept_keystrokes(move |event, window, cx| {
        if window.window_handle().window_id() == here {
            cx.stop_propagation();
            let keystroke = event.keystroke.clone();
            this.update(cx, |this, cx| take(this, keystroke, cx)).ok();
        }
    })
}

/// The cap of keys being recorded, with those pressed so far.
fn recording_cap(recording: &Recording, theme: &Theme) -> impl IntoElement + use<> {
    let keys: Vec<String> = (recording.strokes.iter())
        .map(|stroke| edit::written(stroke, None, std::env::consts::OS))
        .collect();
    let label = match keys.is_empty() {
        true => "Press keys…".to_owned(),
        false => format!("{}…", ui::keys_written(&keys.join(" "))),
    };
    h_flex()
        .flex_none()
        .h(dp(28.))
        .px(dp(6.))
        .rounded(dp(theme.shape.control))
        .border_1()
        .border_color(theme.pop)
        .text_size(dp(SMALL_TEXT))
        .text_color(theme.text_1)
        .child(label)
}

/// A column of text, of the header or a row.
fn text_column(width: f32) -> gpui::Div {
    div().w(dp(width)).min_w_0().pr(dp(8.))
}

/// The keys' column, of the header or a row.
fn keys_column() -> gpui::Div {
    div().flex_1().min_w(dp(KEYS_MIN_WIDTH))
}

/// A column's or a group's name.
fn table_heading(text: impl IntoElement) -> gpui::Div {
    div()
        .text_size(dp(SMALL_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

/// `on` with platform `ix` switched, unless that'd leave none for the key to bind on.
fn toggled(mut on: [bool; 3], ix: usize) -> [bool; 3] {
    let others = (on.iter().enumerate()).any(|(other, on)| other != ix && *on);
    if let Some(platform) = on.get_mut(ix).filter(|_| others) {
        *platform = !*platform;
    }
    on
}

/// What a new key's `on` platforms come to in a section: nothing to name when it's all of them.
fn chosen_platforms(on: [bool; 3]) -> Option<Vec<String>> {
    if on.iter().all(|on| *on) {
        return None;
    }
    let names = (keymap::PLATFORMS.iter().zip(on))
        .filter(|(_, on)| *on)
        .map(|(name, _)| (*name).to_owned());
    Some(names.collect())
}

/// Whether the platforms two sections name have one in common, as `None` is all of them.
fn share_a_platform(a: Option<&[String]>, b: Option<&[String]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.iter().any(|name| b.contains(name)),
        (Some(_), None) | (None, Some(_)) | (None, None) => true,
    }
}

/// Whether `keys` already run `action` in `context` on one of the platforms `os`, in a keymap
/// with `file` in it: a second binding would never show.
fn already_bound(
    bindings: &[Binding],
    file: KeymapFile,
    action: &str,
    context: &Option<String>,
    keys: &str,
    os: Option<&[String]>,
) -> bool {
    bindings.iter().any(|binding| {
        let one_keymap =
            binding.file == file || binding.file == KeymapFile::Base || file == KeymapFile::Base;
        one_keymap
            && binding.action == action
            && binding.context == *context
            && binding.keys == keys
            && share_a_platform(binding.os.as_deref(), os)
    })
}

/// Every action there is: the palette's, then the rest, named after their names.
fn commands(bindings: &[Binding], cx: &App) -> Vec<Command> {
    let mut commands: Vec<Command> = (COMMANDS.iter())
        .map(|command| Command {
            name: command.name().to_owned(),
            title: command.title().to_owned(),
            aliases: (command.aliases().iter())
                .map(|alias| (*alias).to_owned())
                .collect(),
        })
        .collect();
    let mut names: HashSet<String> = (commands.iter())
        .map(|command| command.name.clone())
        .collect();
    let registered = (cx.all_action_names().iter())
        .filter(|name| !UNLISTED.iter().any(|prefix| name.starts_with(prefix)))
        .map(|name| (*name).to_owned());
    let bound = bindings.iter().map(|binding| binding.action.clone());
    for name in registered.chain(bound) {
        if names.insert(name.clone()) {
            commands.push(Command {
                title: title_of(&name),
                name,
                aliases: Vec::new(),
            });
        }
    }
    commands
}

impl Render for CommandEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<CommandEditor>) -> impl IntoElement {
        // Every `dp` size is in rems, so this scales the whole window, as the app's does.
        window.set_rem_size(Theme::get(cx).scaled(ui::BASE_REM));
        ui::Covered::note(self.picking.is_some(), window, cx);
        let theme = Theme::get(cx).clone();
        let table = list(
            self.list.clone(),
            cx.processor(|this, ix, _, cx| this.render_line(ix, cx)),
        )
        .size_full();
        let client_frame = matches!(window.window_decorations(), Decorations::Client { .. });
        let root = v_flex()
            .id("command-editor")
            .key_context("CommandEditor")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .font_family(theme.ui_font.clone())
            .text_size(dp(14.))
            .text_color(theme.text_1)
            .bg(theme.bg_1)
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            // Before what's pressed takes it, so pressing other keys to change starts afresh.
            .capture_any_mouse_down(cx.listener(|this, event, _, cx| {
                this.selection = None;
                this.press_while_recording(event, cx)
            }))
            .on_mouse_move(cx.listener(|this, event, _, cx| this.drag_action(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release_action(cx)),
            )
            .on_key_down(cx.listener(|this, event, _, cx| this.copy_action(event, cx)))
            .child(self.render_toolbar(&theme, window, cx))
            .map(|el| match self.tab {
                Tab::Keys => el.child(self.render_header(&theme)).child(scrolled(
                    table,
                    "command-editor-scrollbar",
                    Scroll::List(self.list.clone()),
                    &theme,
                )),
                Tab::MenuBar => el.child(self.menu_bar.clone()),
            })
            .child(self.render_footer(&theme, cx))
            .children(self.render_picker(&theme, cx))
            .when(client_frame, |el| {
                el.child(ui::window_controls::window_controls(window, cx))
            })
            .when(cfg!(target_os = "macos"), |el| {
                el.child(ui::holds_back_first_click())
            });
        ui::window_frame::window_frame(root, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Binding, KeymapFile, already_bound, between, chosen_platforms, share_a_platform, toggled,
    };

    fn names(os: &[&str]) -> Vec<String> {
        os.iter().map(|name| (*name).to_owned()).collect()
    }

    fn bound_on(os: Option<&[&str]>) -> Vec<Binding> {
        Vec::from([Binding {
            file: KeymapFile::Standard,
            section: 0,
            keys: "f6".to_owned(),
            action: "a::Next".to_owned(),
            context: None,
            os: os.map(names),
        }])
    }

    #[test]
    fn a_platform_can_be_unticked() {
        assert_eq!(toggled([true, true, true], 0), [false, true, true]);
    }

    #[test]
    fn a_platform_can_be_ticked_again() {
        assert_eq!(toggled([false, true, true], 0), [true, true, true]);
    }

    #[test]
    fn the_last_platform_stays_ticked() {
        assert_eq!(toggled([false, false, true], 2), [false, false, true]);
    }

    #[test]
    fn every_platform_chosen_names_none() {
        assert_eq!(chosen_platforms([true, true, true]), None);
    }

    #[test]
    fn some_platforms_chosen_are_named_in_order() {
        assert_eq!(
            chosen_platforms([false, true, true]),
            Some(names(&["windows", "linux"]))
        );
    }

    #[test]
    fn one_platform_chosen_is_named() {
        assert_eq!(
            chosen_platforms([true, false, false]),
            Some(names(&["macos"]))
        );
    }

    #[test]
    fn sections_for_every_platform_share_one_with_any() {
        assert!(share_a_platform(None, Some(&names(&["linux"]))));
    }

    #[test]
    fn sections_for_other_platforms_share_none() {
        assert!(!share_a_platform(
            Some(&names(&["macos"])),
            Some(&names(&["linux"]))
        ));
    }

    #[test]
    fn a_key_bound_on_a_platform_is_already_bound_there() {
        let os = names(&["linux"]);
        let bindings = bound_on(Some(&["windows", "linux"]));
        assert!(already_bound(
            &bindings,
            KeymapFile::Standard,
            "a::Next",
            &None,
            "f6",
            Some(&os)
        ));
    }

    #[test]
    fn a_key_bound_on_another_platform_can_be_added_here() {
        let os = names(&["macos"]);
        let bindings = bound_on(Some(&["windows", "linux"]));
        assert!(!already_bound(
            &bindings,
            KeymapFile::Standard,
            "a::Next",
            &None,
            "f6",
            Some(&os)
        ));
    }

    #[test]
    fn a_selection_dragged_to_the_right_runs_from_where_it_started() {
        assert_eq!(between(2, 7), 2..7);
    }

    #[test]
    fn a_selection_dragged_to_the_left_runs_to_where_it_started() {
        assert_eq!(between(7, 2), 2..7);
    }
}

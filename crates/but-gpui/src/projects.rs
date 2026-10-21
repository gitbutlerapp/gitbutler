//! Getting projects into shape: cloning a repository to start one, finding a project whose
//! folder moved, and picking the target branch the workspace integrates into.

use std::path::PathBuf;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, Focusable as _, PathPromptOptions, Pixels, Point,
    Subscription, Window, actions, anchored, deferred, div, prelude::*, px,
};

use crate::app::AppView;
use crate::backend::{self, settings};
use crate::palette::Choice;
use crate::state::ProjectId;
use crate::theme::Theme;
use crate::ui::emoji::{self, ProjectIcon};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{Tooltip, faint, field, popover, section_label};

actions!(projects, [CloneRepository]);

/// Ask for one folder with the system's picker, `prompt` on its button.
pub fn pick_folder(
    prompt: &'static str,
    cx: &App,
) -> impl Future<Output = Option<PathBuf>> + use<> {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(prompt.into()),
    });
    async move { paths.await.ok()?.ok()??.into_iter().next() }
}

/// The folder a clone of `url` goes in: the repository's name.
fn clone_name(url: &str) -> Option<String> {
    let name = url.trim().trim_end_matches('/').rsplit(['/', ':']).next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    (!name.is_empty()).then(|| name.to_owned())
}

impl AppView {
    /// Ask for a repository's URL and where to put it, clone it there, and add it.
    pub fn clone_repository(
        &mut self,
        _: &CloneRepository,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| TextInput::new("https://github.com/owner/repo.git", false, cx));
        let check = |_: &AppView, url: &str, _: &gpui::App| match clone_name(url) {
            Some(_) => None,
            None => Some("Enter the repository's URL".into()),
        };
        self.open_prompt(
            "Clone a repository",
            Some("Next, pick the folder to clone it into.".into()),
            "Choose folder…",
            input,
            None,
            Some(Box::new(check)),
            window,
            cx,
            |this, url, _, cx| {
                let Some(name) = clone_name(&url) else {
                    return;
                };
                let folder = pick_folder("Clone here", cx);
                let store = this.store.clone();
                cx.spawn(async move |_, cx| {
                    let Some(into) = folder.await.map(|dir| dir.join(&name)) else {
                        return;
                    };
                    store.update(cx, |store, cx| store.snack(format!("Cloning {name}…"), cx));
                    let target = into.clone();
                    let cloned = backend::run(move || backend::clone(url.trim(), &target)).await;
                    store.update(cx, |store, cx| match cloned {
                        Ok(()) => store.add_project(into, cx),
                        Err(err) => {
                            store.error(format!("Couldn't clone {name}"), format!("{err:#}"), cx)
                        }
                    });
                })
                .detach();
            },
        );
    }

    /// Offer the project's remote branches to pick its target from.
    pub fn pick_target(&mut self, project: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let listed = {
                let path = path.clone();
                backend::run(move || settings::remote_branches(&path)).await
            };
            let names = match listed {
                Ok(names) if !names.is_empty() => names,
                Ok(_) => {
                    let fetched = project.clone();
                    store.update(cx, |store, cx| {
                        store.error_with_retry(
                            "No remote branches to pick from",
                            "Fetch to list the remote's branches, then choose again. A project \
                             without a remote needs one added first.",
                            "Fetch",
                            move |store, cx| store.fetch(&fetched, cx),
                            cx,
                        )
                    });
                    return;
                }
                Err(err) => {
                    store.update(cx, |store, cx| {
                        store.error("Couldn't list the remote branches", format!("{err:#}"), cx)
                    });
                    return;
                }
            };
            let choices = names
                .into_iter()
                .map(|name| {
                    let short = name
                        .strip_prefix("refs/remotes/")
                        .unwrap_or(&name)
                        .to_owned();
                    let (path, project) = (path.clone(), project.clone());
                    Choice::new(short.clone(), move |app, _, cx| {
                        let store = app.store.clone();
                        cx.spawn(async move |_, cx| {
                            let set =
                                backend::run(move || settings::set_target(&path, &name)).await;
                            store.update(cx, |store, cx| match set {
                                Ok(()) => {
                                    store.reload(&project, cx);
                                    store.snack(format!("The target is now {short}"), cx)
                                }
                                Err(err) => store.error(
                                    "Couldn't change the target",
                                    format!("{err:#}"),
                                    cx,
                                ),
                            });
                        })
                        .detach();
                    })
                })
                .collect();
            this.update_in(cx, |app, window, cx| {
                app.open_picker("Pick the target branch", "Pick", choices, window, cx)
            })
            .ok();
        })
        .detach();
    }
}

/// The picker open on a project's icon, of the glyph's colours and emoji, as but.dev's
/// repository page has one.
pub struct IconPicker {
    project: ProjectId,
    position: Point<Pixels>,
    search: Entity<TextInput>,
    _subscription: Subscription,
}

/// A choice's cell in the picker's grid, seven to a row, as ui-react's picker lays them out.
const CELL: f32 = 28.;

impl AppView {
    /// Open the icon picker for `project`'s icon at `position`, under its button.
    pub fn open_icon_picker(
        &mut self,
        project: ProjectId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let search = cx.new(|cx| TextInput::new("Search emoji…", false, cx));
        let subscription =
            cx.subscribe_in(
                &search,
                window,
                |this, input, event, window, cx| match event {
                    InputEvent::Changed => cx.notify(),
                    // Return takes the first match, a colour before an emoji as they're listed.
                    InputEvent::Submit => {
                        let query = input.read(cx).text();
                        let first = match emoji::search_colours(query).first() {
                            Some(colour) => Some(colour.stored()),
                            None => {
                                (emoji::search(query).first()).map(|e| Some(e.emoji.to_owned()))
                            }
                        };
                        if let Some(first) = first {
                            this.set_project_icon(first, window, cx)
                        }
                    }
                    InputEvent::Cancel => this.set_icon_picker_closed(window, cx),
                },
            );
        window.focus(&search.focus_handle(cx), cx);
        self.icon_picker = Some(IconPicker {
            project,
            position,
            search,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Give the picker's project `icon`, as but.dev stores it, or none, and close the picker.
    fn set_project_icon(
        &mut self,
        icon: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(picker) = self.icon_picker.as_ref() else {
            return;
        };
        let project = picker.project.clone();
        (self.store).update(cx, |store, cx| store.set_project_icon(&project, icon, cx));
        self.set_icon_picker_closed(window, cx);
    }

    fn set_icon_picker_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.icon_picker.take().is_some() {
            self.focus_main(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn render_icon_picker(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self.icon_picker.as_ref()?;
        let theme = Theme::get(cx);
        let current = (self.store.read(cx).project_icon(&picker.project))
            .filter(|icon| !icon.is_empty())
            .map(str::to_owned);
        let query = picker.search.read(cx).text();
        let colours = emoji::search_colours(query);
        let found = emoji::search(query);
        // A choice's cell: what it draws, highlighted when it's the project's.
        let cell = |id: (&'static str, usize),
                    name: &'static str,
                    stored: Option<String>,
                    mark: AnyElement| {
            div()
                .id(id)
                .size(px(CELL))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .text_size(px(18.))
                .cursor_pointer()
                .hover(|el| el.bg(theme.hover()))
                .when(current == stored, |el| el.bg(theme.selected_unfocused()))
                .tooltip(Tooltip::text(name))
                .child(mark)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.set_project_icon(stored.clone(), window, cx)
                }))
        };
        let row = |cells: Vec<_>| {
            div()
                .w(px(7. * CELL + 6. * 4.))
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .children(cells)
        };
        let colour_cells: Vec<_> = (colours.iter().enumerate())
            .map(|(ix, colour)| {
                let mark = ProjectIcon::Glyph(colour).render(px(18.), theme);
                cell(("colour", ix), colour.name, colour.stored(), mark)
            })
            .collect();
        let emoji_cells: Vec<_> = (found.iter().enumerate())
            .map(|(ix, choice)| {
                let mark = div().child(choice.emoji).into_any_element();
                cell(
                    ("emoji", ix),
                    choice.name,
                    Some(choice.emoji.to_owned()),
                    mark,
                )
            })
            .collect();
        let grid = div()
            .id("emoji-grid")
            // Five rows, then it scrolls.
            .max_h(px(5. * CELL + 4. * 4.))
            .overflow_y_scroll()
            .child(row(emoji_cells));
        let focused = picker.search.focus_handle(cx).is_focused(window);
        Some(
            deferred(
                div().absolute().inset_0().occlude().child(
                    anchored()
                        .position(picker.position)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            popover(theme)
                                .id("icon-picker")
                                .p(px(8.))
                                .gap(px(8.))
                                .child(field(&picker.search, focused, theme))
                                .when(!colours.is_empty(), |el| {
                                    el.child(section_label("Colours", theme))
                                        .child(row(colour_cells))
                                })
                                .when(!found.is_empty(), |el| {
                                    el.child(section_label("Emoji", theme)).child(grid)
                                })
                                .when(colours.is_empty() && found.is_empty(), |el| {
                                    el.child(faint("Nothing found", theme))
                                })
                                .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                                    this.set_icon_picker_closed(window, cx)
                                })),
                        ),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::clone_name;

    #[test]
    fn names_the_clone_after_the_repository() {
        assert_eq!(
            clone_name("https://github.com/gitbutlerapp/gitbutler.git").as_deref(),
            Some("gitbutler"),
            "https, .git dropped"
        );
        assert_eq!(
            clone_name("git@github.com:owner/repo").as_deref(),
            Some("repo"),
            "ssh form"
        );
        assert_eq!(clone_name("  "), None, "nothing to name");
    }
}

//! Getting projects into shape: cloning a repository to start one, picking the target branch
//! the workspace integrates into, asking for the author Git records commits with, and picking a
//! project's icon.

use std::path::PathBuf;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, Focusable as _, PathPromptOptions, Pixels, Point,
    Subscription, Task, Window, actions, div, prelude::*,
};

use crate::app::AppView;
use crate::backend;
use crate::backend::settings::{self, MissingAuthor};
use crate::overlays::floating;
use crate::palette::Choice;
use crate::state::{ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::emoji::{self, ProjectIcon};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{Tooltip, dp, faint, field, popover, section_label};

actions!(projects, [CloneRepository]);

/// Ask for one folder with the system's picker, `prompt` on its button. When the picker can't
/// open, as on Linux without a desktop portal, a card says why.
pub fn pick_folder(prompt: &'static str, store: Entity<Store>, cx: &App) -> Task<Option<PathBuf>> {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(prompt.into()),
    });
    cx.spawn(async move |cx| match paths.await.ok()? {
        Ok(paths) => paths?.into_iter().next(),
        Err(err) => {
            store.update(cx, |store, cx| {
                store.failed("Couldn't open the folder picker", &err, cx)
            });
            None
        }
    })
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
            None,
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
                let store = this.store.clone();
                let folder = pick_folder("Clone here", store.clone(), cx);
                cx.spawn(async move |_, cx| {
                    let Some(into) = folder.await.map(|dir| dir.join(&name)) else {
                        return;
                    };
                    store.update(cx, |store, cx| store.snack(format!("Cloning {name}…"), cx));
                    let target = into.clone();
                    let cloned = backend::run(move || backend::clone(url.trim(), &target)).await;
                    store.update(cx, |store, cx| match cloned {
                        Ok(()) => store.add_project(into, cx),
                        Err(err) => store.failed(format!("Couldn't clone {name}"), &err, cx),
                    });
                })
                .detach();
            },
        );
    }

    /// Make the branch `name`, as refs/heads/main, `project`'s target, or for a remote branch
    /// as refs/remotes/origin/main, the local branch tracking it, then read the project again
    /// and say so.
    pub fn set_target(
        &mut self,
        project: ProjectId,
        name: backend::FullName,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        let store = self.store.clone();
        let short = name.shorten().to_string();
        cx.spawn(async move |_, cx| {
            let set = backend::run(move || settings::set_target(&path, &name)).await;
            store.update(cx, |store, cx| match set {
                Ok(()) => {
                    store.reload_or_open(&project, cx);
                    store.snack(format!("The target is now {short}"), cx)
                }
                Err(err) => store.failed("Couldn't change the target", &err, cx),
            });
        })
        .detach();
    }

    /// Offer the project's local branches to pick its target from; each one's upstream is what
    /// pulls bring in.
    pub fn pick_target(&mut self, project: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        let store = self.store.clone();
        cx.spawn_in(window, async move |this, cx| {
            let listed = {
                let path = path.clone();
                backend::run(move || settings::target_branches(&path)).await
            };
            let names = match listed {
                Ok(names) if !names.is_empty() => names,
                Ok(_) => {
                    store.update(cx, |store, cx| {
                        store.error(
                            "No branches to pick from",
                            "The target is a local branch, like main, tracking the remote's. \
                             Create one, then choose again.",
                            cx,
                        )
                    });
                    return;
                }
                Err(err) => {
                    store.update(cx, |store, cx| {
                        store.failed("Couldn't list the branches", &err, cx)
                    });
                    return;
                }
            };
            let choices = names
                .into_iter()
                .map(|name| {
                    let short = backend::short_ref(&name).into_owned();
                    let project = project.clone();
                    Choice::new(short, move |app, _, cx| app.set_target(project, name, cx))
                })
                .collect();
            this.update_in(cx, |app, window, cx| {
                let place = app
                    .store
                    .read(cx)
                    .place(&Selection::Project(project.clone()));
                app.open_picker(
                    Some(place),
                    "Pick the target branch",
                    "Pick",
                    choices,
                    window,
                    cx,
                )
            })
            .ok();
        })
        .detach();
    }
}

impl AppView {
    /// Ask for the name, then the email, Git records commits with, of those it lacks, and save
    /// them to the user's global Git config, as the desktop app asks when Git has none.
    pub fn ask_author(&mut self, project: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let missing = backend::run(move || settings::author_missing(&path)).await;
            let Ok(Some(missing)) = missing else {
                return;
            };
            this.update_in(cx, |this, window, cx| match missing.name {
                true => this.ask_author_name(project, missing, window, cx),
                false => this.ask_author_email(project, String::new(), missing, window, cx),
            })
            .ok();
        })
        .detach();
    }

    fn ask_author_name(
        &mut self,
        project: ProjectId,
        missing: MissingAuthor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| TextInput::new("Your full name", false, cx));
        let check = |_: &AppView, name: &str, _: &App| {
            name.trim().is_empty().then(|| "Enter your name".into())
        };
        let goes = match (missing.here, missing.email) {
            (true, true) => format!(
                "Commits record who made them. Your name goes in your global Git config, and \
                 your email next in {}.",
                saved_in(missing)
            ),
            (true, false) => {
                "Commits record who made them. Your name goes in your global Git config.".into()
            }
            (false, _) => format!(
                "Commits record who made them. What you enter goes in {}.",
                saved_in(missing)
            ),
        };
        self.open_prompt(
            None,
            "Set up your Git author",
            Some(goes),
            if missing.email { "Next" } else { "Save" },
            input,
            None,
            Some(Box::new(check)),
            window,
            cx,
            // After this prompt gives the keys back, so the next one keeps them.
            move |this, name, window, cx| match missing.email {
                true => cx.defer_in(window, move |this, window, cx| {
                    this.ask_author_email(project, name, missing, window, cx)
                }),
                false => this.save_author(project, name, String::new(), missing, cx),
            },
        );
    }

    fn ask_author_email(
        &mut self,
        project: ProjectId,
        name: String,
        missing: MissingAuthor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| TextInput::new("your.email@example.com", false, cx));
        let check = |_: &AppView, email: &str, _: &App| {
            let email = email.trim();
            let looks_right = email.split_once('@').is_some_and(|(user, host)| {
                !user.is_empty()
                    && host.contains('.')
                    && !host.contains('@')
                    && !email.contains(char::is_whitespace)
            });
            (!looks_right).then(|| "Enter an email address".into())
        };
        let goes = match missing.name {
            true => "Next, the email address commits record with your name.".to_owned(),
            false => format!(
                "Commits record who made them. Git has your name, and your email goes in {}.",
                saved_in(missing)
            ),
        };
        self.open_prompt(
            None,
            "Set up your Git author",
            Some(goes),
            "Save",
            input,
            None,
            Some(Box::new(check)),
            window,
            cx,
            move |this, email, _, cx| this.save_author(project, name, email, missing, cx),
        );
    }

    fn save_author(
        &mut self,
        project: ProjectId,
        name: String,
        email: String,
        missing: MissingAuthor,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            let set =
                backend::run(move || settings::set_author(&path, &name, &email, missing.here))
                    .await;
            store.update(cx, |store, cx| match set {
                Ok(()) => store.snack("Git has your name and email now", cx),
                Err(err) => store.failed("Couldn't save your name and email", &err, cx),
            });
        })
        .detach();
    }
}

/// Where the author asked for is saved, in words.
fn saved_in(missing: MissingAuthor) -> &'static str {
    match missing.here {
        true => "this repository's Git config, as your user.useConfigOnly asks",
        false => "your global Git config, for every repository",
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
            self.restore_focus(window, cx);
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
                .size(dp(CELL))
                .flex()
                .items_center()
                .justify_center()
                .rounded(dp(4.))
                .text_size(dp(18.))
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
                .w(dp(7. * CELL + 6. * 4.))
                .flex()
                .flex_wrap()
                .gap(dp(4.))
                .children(cells)
        };
        let colour_cells: Vec<_> = (colours.iter().enumerate())
            .map(|(ix, colour)| {
                let mark = ProjectIcon::Glyph(colour).render(theme.scaled(18.), theme);
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
            .max_h(dp(5. * CELL + 4. * 4.))
            .overflow_y_scroll()
            .child(row(emoji_cells));
        let focused = picker.search.focus_handle(cx).is_focused(window);
        let card = popover(theme)
            .id("icon-picker")
            .p(dp(8.))
            .gap(dp(8.))
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
            });
        Some(floating(
            picker.position,
            card,
            cx,
            Self::set_icon_picker_closed,
        ))
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

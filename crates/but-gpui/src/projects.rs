//! Getting projects into shape: cloning a repository to start one, finding a project whose
//! folder moved, and picking the target branch the workspace integrates into.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, Context, PathPromptOptions, Window, actions, prelude::*};

use crate::app::AppView;
use crate::backend::{self, settings};
use crate::palette::Choice;
use crate::state::{Notice, ProjectId, Retry};
use crate::ui::text_input::TextInput;

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
        self.open_prompt(
            "Clone a repository",
            Some("Next, pick the folder to clone it into.".into()),
            "Choose folder…",
            input,
            None,
            None,
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
                    let fetch: Retry = (
                        "Fetch".into(),
                        Rc::new(move |store, cx| store.fetch(&fetched, cx)),
                    );
                    let notice = Notice::Error {
                        title: "No remote branches to pick from".into(),
                        detail: "Fetch to list the remote's branches, then choose again. A \
                                 project without a remote needs one added first."
                            .into(),
                        retry: Some(fetch),
                    };
                    store.update(cx, |store, cx| store.notify_user(notice, cx));
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

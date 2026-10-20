//! Opening a file in the user's editor, at the line they're on, and the project in their
//! terminal. The first time with no editor chosen, a menu asks which and remembers it.

use std::path::PathBuf;

use gpui::{App, Context, KeyBinding, Pixels, Point, Window, actions};

use crate::app::AppView;
use crate::backend::{self, settings};
use crate::overlays::MenuItem;
use crate::state::{ProjectId, Store};

actions!(open_in, [OpenInEditor, OpenInTerminal]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "e",
        OpenInEditor,
        Some("Detail && !TextInput"),
    )]);
}

/// The macOS terminal everyone has, until another is chosen.
const DEFAULT_TERMINAL: &str = "terminal";

/// What "Open in …" says for the chosen editor, or that it will ask.
pub fn editor_label(store: &Store) -> String {
    (store.prefs.editor.as_ref())
        .and_then(|id| settings::editors().into_iter().find(|(e, _)| e == id))
        .map_or_else(
            || "Open in editor…".to_owned(),
            |(_, name)| format!("Open in {name}"),
        )
}

impl AppView {
    /// Open `file` (relative to the project, or to its linked `worktree`; the project itself
    /// with `None`) in the chosen editor at `line`. With none chosen yet, offer the editors
    /// as a menu at `position`.
    pub fn open_in_editor(
        &mut self,
        project: &ProjectId,
        worktree: Option<std::sync::Arc<str>>,
        file: Option<String>,
        line: Option<u32>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(project) else {
            return;
        };
        let file = file.unwrap_or_else(|| ".".to_owned());
        match self.store.read(cx).prefs.editor.clone() {
            Some(editor) => self.run_editor(path, worktree, editor, file, line, cx),
            None => {
                let items = settings::editors()
                    .into_iter()
                    .map(|(id, name)| {
                        let (path, worktree, file) = (path.clone(), worktree.clone(), file.clone());
                        MenuItem::new(name.clone(), move |this, _, cx| {
                            this.store.update(cx, |store, cx| {
                                store.set_prefs(|prefs| prefs.editor = Some(id.clone()), cx);
                                store.snack(
                                    format!("Files now open in {name}, set in Settings → General"),
                                    cx,
                                )
                            });
                            let worktree = worktree.clone();
                            this.run_editor(
                                path.clone(),
                                worktree,
                                id.clone(),
                                file.clone(),
                                line,
                                cx,
                            );
                        })
                    })
                    .collect();
                self.open_menu(position, items, cx);
            }
        }
    }

    fn run_editor(
        &mut self,
        path: PathBuf,
        worktree: Option<std::sync::Arc<str>>,
        editor: String,
        file: String,
        line: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        let open =
            move || settings::open_in_editor(&path, worktree.as_deref(), &editor, &file, line);
        self.open_with("Couldn't open the editor", open, cx);
    }

    /// Run `open` off the UI thread, saying `failed`, with why, when it fails.
    fn open_with(
        &self,
        failed: &'static str,
        open: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            if let Err(err) = backend::run(open).await {
                store.update(cx, |store, cx| store.error(failed, format!("{err:#}"), cx));
            }
        })
        .detach();
    }

    pub fn open_in_terminal(&mut self, project: &ProjectId, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(path) = store.path(project) else {
            return;
        };
        let terminal = store
            .prefs
            .terminal
            .clone()
            .unwrap_or_else(|| DEFAULT_TERMINAL.to_owned());
        let open = move || settings::open_in_terminal(&path, &terminal);
        self.open_with("Couldn't open the terminal", open, cx);
    }

    /// The selected project in the terminal.
    pub fn open_selected_in_terminal(
        &mut self,
        _: &OpenInTerminal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.selected_project(cx) {
            self.open_in_terminal(&project, cx);
        }
    }
}

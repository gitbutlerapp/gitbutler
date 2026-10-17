//! The project page while a commit is checked out to edit: its files and how far along
//! their conflicts are, with saving the commit or giving up. The diff beside it is the
//! commit's changes as they are on disk, markers and all.

use gpui::{
    AnyElement, App, ClickEvent, Context, MouseButton, MouseDownEvent, div, prelude::*, px,
};

use super::Detail;
use crate::app::MenuItem;
use crate::backend::edit::{self, EditedFile, Editing};
use crate::state::{Overlay, ProjectId, SAVE_EDIT, STOP_EDITING};
use crate::theme::Theme;
use crate::ui::diff::split_path;
use crate::ui::{ButtonStyle, Icon, button, h_flex, list_row, plural, small_icon, v_flex};

impl Detail {
    /// The commit `project` is editing, if it's in edit mode.
    pub(super) fn editing<'a>(&self, project: &ProjectId, cx: &'a App) -> Option<&'a Editing> {
        self.store.read(cx).workspace(project)?.editing.as_ref()
    }

    /// Whether a conflicted file still counts as conflicted: its markers are there and the
    /// user hasn't called it resolved anyway.
    fn still_conflicted(&self, project: &ProjectId, editing: &Editing, file: &EditedFile) -> bool {
        let mark = (project.clone(), editing.commit, file.path.clone());
        file.resolved == Some(false) && !self.marked_resolved.contains(&mark)
    }

    fn unresolved(&self, project: &ProjectId, editing: &Editing) -> usize {
        (editing.files.iter())
            .filter(|file| self.still_conflicted(project, editing, file))
            .count()
    }

    pub(super) fn render_edit_side(
        &self,
        project: &ProjectId,
        editing: &Editing,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let about = v_flex()
            .flex_none()
            .p(px(16.))
            .gap(px(6.))
            .border_b_1()
            .border_color(theme.border_3)
            .text_size(px(12.))
            .text_color(theme.text_2)
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text_1)
                    .child(match editing.title.as_str() {
                        "" => "No message".to_owned(),
                        title => title.to_owned(),
                    }),
            )
            .child(format!(
                "Commit {} is checked out in the working directory.",
                editing.commit.short()
            ))
            .child(
                "Resolve its conflicts in any editor, or have an agent do it, then save to \
                 rewrite the commit. The commits above it move onto the result.",
            );
        let heading = |text: String| {
            div()
                .px(px(16.))
                .pt(px(12.))
                .pb(px(4.))
                .text_size(px(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text_3)
                .child(text)
        };
        // `mark` is for a file whose markers are still there: whether the user called it
        // resolved anyway, which its right-click menu changes.
        let commit = editing.commit;
        let row =
            |id: (&'static str, usize), path: &str, status: Option<bool>, mark: Option<bool>| {
                let (name, dir) = split_path(path);
                let (glyph, color, note) = match (status, mark) {
                    (Some(false), _) => (Icon::Warning, theme.danger, Some("conflicted")),
                    (Some(true), Some(true)) => (Icon::Tick, theme.safe, Some("marked resolved")),
                    (Some(true), _) => (Icon::Tick, theme.safe, Some("resolved")),
                    (None, _) => (Icon::File, theme.text_3, None),
                };
                let (project, file) = (project.clone(), path.to_owned());
                let (menu_project, menu_file) = (project.clone(), file.clone());
                list_row(id, false, theme)
                    .text_size(px(13.))
                    .child(small_icon(glyph).text_color(color))
                    .child(div().flex_none().child(name.to_owned()))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(dir.to_owned()),
                    )
                    .children(note.map(|note| {
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(color)
                            .child(note)
                    }))
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        let (project, file) = (project.clone(), file.clone());
                        let position = event.position();
                        this.app
                            .update(cx, |app, cx| {
                                app.open_in_editor(&project, None, Some(file), None, position, cx)
                            })
                            .ok();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            let (project, file) = (menu_project.clone(), menu_file.clone());
                            let position = event.position;
                            let open = MenuItem::new("Open in editor", {
                                let (project, file) = (project.clone(), file.clone());
                                move |app, _, cx| {
                                    app.open_in_editor(
                                        &project,
                                        None,
                                        Some(file.clone()),
                                        None,
                                        position,
                                        cx,
                                    )
                                }
                            });
                            let mut items = vec![open];
                            if let Some(marked) = mark {
                                let label = if marked {
                                    "Count as conflicted again"
                                } else {
                                    "Mark resolved"
                                };
                                items.push(MenuItem::new(label, move |app, _, cx| {
                                    let key = (project.clone(), commit, file.clone());
                                    app.detail.update(cx, |detail, cx| {
                                        if !detail.marked_resolved.remove(&key) {
                                            detail.marked_resolved.insert(key);
                                        }
                                        cx.notify();
                                    });
                                }));
                            }
                            this.app
                                .update(cx, |app, cx| app.open_menu(event.position, items, cx))
                                .ok();
                        }),
                    )
            };
        let files = editing.files.iter().enumerate().map(|(ix, file)| {
            let has_markers = file.resolved == Some(false);
            let status = file
                .resolved
                .map(|_| !self.still_conflicted(project, editing, file));
            let mark = has_markers.then(|| status == Some(true));
            row(("edited", ix), &file.path, status, mark)
        });
        let changed = editing
            .changed
            .iter()
            .enumerate()
            .map(|(ix, path)| row(("changed", ix), path, None, None));
        let nothing = |text: &'static str| {
            div()
                .px(px(16.))
                .py(px(4.))
                .text_size(px(12.))
                .text_color(theme.text_3)
                .child(text)
        };
        let unresolved = self.unresolved(project, editing);
        let footer = h_flex()
            .flex_none()
            .h(px(44.))
            .px(px(12.))
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.border_3)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(if unresolved > 0 {
                        theme.danger
                    } else {
                        theme.text_3
                    })
                    .child(match unresolved {
                        0 => "No conflicts left".to_owned(),
                        n => format!("{} still conflicted", plural(n, "file")),
                    }),
            )
            .child(
                button("cancel-edit", "Cancel")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(cx.listener({
                        let project = project.clone();
                        move |this, _, window, cx| this.cancel_edit(&project, window, cx)
                    })),
            )
            .child(
                button("save-edit", "Save")
                    .small()
                    .style(ButtonStyle::Pop)
                    .on_click(cx.listener({
                        let project = project.clone();
                        move |this, _, window, cx| this.save_edit(&project, window, cx)
                    })),
            );
        v_flex()
            .size_full()
            .child(about)
            .child(
                v_flex()
                    .id("edited-files")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .pb(px(8.))
                    .child(heading(format!(
                        "In this commit · {}",
                        plural(editing.files.len(), "file")
                    )))
                    .children(files)
                    .when(editing.files.is_empty(), |el| el.child(nothing("No files")))
                    .child(heading("Changed since you started".to_owned()))
                    .children(changed)
                    .when(editing.changed.is_empty(), |el| {
                        el.child(nothing("Nothing yet"))
                    }),
            )
            .child(footer)
            .into_any_element()
    }

    /// Rewrite the commit from the worktree, asking first while conflicts are left in it.
    fn save_edit(
        &mut self,
        project: &ProjectId,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let unresolved = (self.editing(project, cx)).map_or(0, |e| self.unresolved(project, e));
        let save = {
            let project = project.clone();
            move |store: &mut crate::state::Store, cx: &mut Context<crate::state::Store>| {
                store.run(
                    &project,
                    SAVE_EDIT,
                    Overlay::default(),
                    |path, _| edit::save(path),
                    |store, (), cx| store.snack_undoable("Commit saved", cx),
                    cx,
                )
            }
        };
        if unresolved == 0 {
            self.store.update(cx, save);
            return;
        }
        let store = self.store.clone();
        self.app
            .update(cx, |app, cx| {
                app.confirm_then(
                    format!(
                        "Save with conflict markers in {}?",
                        plural(unresolved, "file")
                    ),
                    "The markers are committed as text, and the commit no longer counts as \
                     conflicted.",
                    "Save anyway",
                    window,
                    cx,
                    move |_, cx| store.update(cx, save),
                )
            })
            .ok();
    }

    /// Go back to the workspace as it was, after asking: edits inside a conflicted file
    /// can't be told from its markers, so there's no knowing that nothing would be lost.
    fn cancel_edit(
        &mut self,
        project: &ProjectId,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let project = project.clone();
        let store = self.store.clone();
        self.app
            .update(cx, |app, cx| {
                app.confirm_then(
                    "Stop editing the commit?".into(),
                    "Changes made to its files are dropped, and it stays as it was.",
                    "Stop editing",
                    window,
                    cx,
                    move |_, cx| {
                        store.update(cx, |store, cx| {
                            store.run(
                                &project,
                                STOP_EDITING,
                                Overlay::default(),
                                |path, _| edit::cancel(path),
                                |store, (), cx| store.snack("Back to the workspace", cx),
                                cx,
                            )
                        })
                    },
                )
            })
            .ok();
    }
}

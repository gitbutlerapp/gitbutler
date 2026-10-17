//! The commit composer: which changes go in, the message, and the branch they land on.

use std::collections::{BTreeSet, HashMap, HashSet};

use gpui::{AnyElement, App, Context, Focusable as _, Toggled, Window, div, prelude::*, px};

use super::{ChecksChanged, Detail};
use crate::app::MenuItem;
use crate::backend::{
    self, CommitId, Destination, DiffBody, DiffSource, FileChange, Hunk, HunkKey, LineKind,
    Selected, ai,
};
use crate::state::{Overlay, ProjectId, Selection};
use crate::theme::Theme;
use crate::transfer::{Transfer, short_ref};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, button, h_flex, icon_button, shortcut, small_icon, v_flex};

impl Detail {
    pub(super) fn close_composer(&mut self, cx: &mut Context<Self>) {
        self.composing = false;
        // A message still being written would land in whatever the box shows next.
        self.writing = None;
        self.unchecked.clear();
        self.unchecked_hunks.clear();
        self.line_picks.clear();
        cx.emit(ChecksChanged);
        cx.notify();
    }

    fn checked_uncommitted(&self, cx: &App) -> Vec<FileChange> {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return Vec::new();
        };
        let Some(workspace) = self.store.read(cx).workspace(&project) else {
            return Vec::new();
        };
        workspace
            .uncommitted
            .iter()
            .filter(|file| !file.conflicted && !self.unchecked.contains(&file.path))
            .cloned()
            .collect()
    }

    /// The checked uncommitted changes: checked files, narrowed to their checked hunks and
    /// lines.
    pub fn selected_changes(&self, cx: &App) -> Vec<Selected> {
        self.checked_uncommitted(cx)
            .into_iter()
            .filter_map(|file| {
                let hunks = self.worktree_file_hunks(&file.path);
                let partial = self.file_check(&file.path) == Toggled::Mixed;
                // Hunks left out but not loaded to tell which: leave the file alone rather
                // than take all of it.
                if hunks.is_empty() && partial {
                    return None;
                }
                let picked: Vec<HunkKey> = hunks
                    .iter()
                    .flat_map(|hunk| match self.hunk_check(&file.path, hunk.key) {
                        Toggled::True => vec![hunk.key],
                        Toggled::Mixed => {
                            hunk.picked(&self.line_picks[&(file.path.clone(), hunk.key)])
                        }
                        Toggled::False => Vec::new(),
                    })
                    .collect();
                if picked.is_empty() && !hunks.is_empty() {
                    return None;
                }
                Some(Selected {
                    file,
                    hunks: partial.then_some(picked),
                })
            })
            .collect()
    }

    /// The hunks of `path` in the loaded worktree diff.
    fn worktree_file_hunks(&self, path: &str) -> &[Hunk] {
        self.diff
            .as_ref()
            .filter(|diff| diff.key.source == DiffSource::Worktree)
            .and_then(|diff| diff.files.iter().find(|f| f.file.path == path))
            .and_then(|file| match &file.body {
                DiffBody::Hunks { hunks, .. } => Some(hunks.as_slice()),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn worktree_hunks(&self, path: &str) -> Vec<HunkKey> {
        self.worktree_file_hunks(path)
            .iter()
            .map(|hunk| hunk.key)
            .collect()
    }

    /// See [`Detail::conflicted`].
    pub(super) fn refresh_conflicted(&mut self, cx: &App) {
        let store = self.store.read(cx);
        let conflicted = match (self.checks_project.as_ref()).and_then(|p| store.workspace(p)) {
            Some(ws) => (ws.uncommitted.iter())
                .filter(|f| f.conflicted)
                .map(|f| f.path.clone())
                .collect(),
            None => HashSet::new(),
        };
        self.conflicted = conflicted;
    }

    pub(super) fn file_check(&self, path: &str) -> Toggled {
        if self.unchecked.contains(path) || self.conflicted.contains(path) {
            return Toggled::False;
        }
        let partial = self.unchecked_hunks.iter().any(|(p, _)| p == path)
            || self.line_picks.keys().any(|(p, _)| p == path);
        if partial {
            Toggled::Mixed
        } else {
            Toggled::True
        }
    }

    pub(super) fn hunk_check(&self, path: &str, key: HunkKey) -> Toggled {
        let entry = (path.to_owned(), key);
        if self.unchecked.contains(path)
            || self.conflicted.contains(path)
            || self.unchecked_hunks.contains(&entry)
        {
            Toggled::False
        } else if self.line_picks.contains_key(&entry) {
            Toggled::Mixed
        } else {
            Toggled::True
        }
    }

    pub(super) fn hunk_checked(&self, path: &str, key: HunkKey) -> bool {
        self.hunk_check(path, key) != Toggled::False
    }

    /// Whether the changed line at `line` of a hunk goes in the next commit.
    pub(super) fn line_checked(&self, path: &str, key: HunkKey, line: usize) -> bool {
        match self.hunk_check(path, key) {
            Toggled::Mixed => self.line_picks[&(path.to_owned(), key)].contains(&line),
            check => check == Toggled::True,
        }
    }

    pub(super) fn toggle_hunk(&mut self, path: &str, key: HunkKey, cx: &mut Context<Self>) {
        if self.conflicted.contains(path) {
            return;
        }
        let entry = (path.to_owned(), key);
        if self.line_picks.remove(&entry).is_some() {
            // Partly in: all of it now.
        } else if self.unchecked.contains(path) {
            self.bring_back_only(path, key);
        } else if !self.unchecked_hunks.remove(&entry) {
            self.unchecked_hunks.insert(entry);
        }
        self.settle_checks(path, cx);
    }

    /// Put one changed line of a hunk in the next commit, or take it out. Into a hunk
    /// that's out, it brings just that line.
    pub(super) fn toggle_line(
        &mut self,
        path: &str,
        key: HunkKey,
        line: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(hunk) = self.worktree_file_hunks(path).iter().find(|h| h.key == key) else {
            return;
        };
        if self.conflicted.contains(path) {
            return;
        }
        let changed: BTreeSet<usize> = hunk
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind != LineKind::Context)
            .map(|(ix, _)| ix)
            .collect();
        let entry = (path.to_owned(), key);
        let mut picks = match self.hunk_check(path, key) {
            Toggled::True => changed.clone(),
            Toggled::Mixed => self.line_picks.remove(&entry).unwrap_or_default(),
            Toggled::False => BTreeSet::new(),
        };
        if !picks.remove(&line) {
            picks.insert(line);
        }
        if self.unchecked.contains(path) {
            self.bring_back_only(path, key);
        }
        self.unchecked_hunks.remove(&entry);
        if picks.is_empty() {
            self.unchecked_hunks.insert(entry);
        } else if picks != changed {
            self.line_picks.insert(entry, picks);
        }
        self.settle_checks(path, cx);
    }

    /// Put back a file that was out as a whole, with only hunk `key` of it in.
    fn bring_back_only(&mut self, path: &str, key: HunkKey) {
        self.unchecked.remove(path);
        for other in self.worktree_hunks(path) {
            if other != key {
                self.unchecked_hunks.insert((path.to_owned(), other));
            }
        }
    }

    /// A file none of whose hunks are in is out as a whole.
    fn settle_checks(&mut self, path: &str, cx: &mut Context<Self>) {
        let hunks = self.worktree_hunks(path);
        if hunks
            .iter()
            .all(|key| self.unchecked_hunks.contains(&(path.to_owned(), *key)))
        {
            self.unchecked_hunks
                .retain(|(hunk_path, _)| hunk_path != path);
            self.unchecked.insert(path.to_owned());
        }
        cx.emit(ChecksChanged);
        cx.notify();
    }

    pub fn focus_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composing = true;
        window.focus(&self.message.focus_handle(cx), cx);
        cx.notify();
    }

    /// Get `files` ready to commit to `branch`, after they were dropped on it.
    pub fn stage(
        &mut self,
        branch: String,
        files: &[Selected],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_target = Some(branch);
        if let Some(project) = self.selected_project(cx)
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let keep: HashMap<&str, Option<&[HunkKey]>> = files
                .iter()
                .map(|f| (f.file.path.as_str(), f.hunks.as_deref()))
                .collect();
            self.unchecked = workspace
                .uncommitted
                .iter()
                .filter(|f| !keep.contains_key(f.path.as_str()))
                .map(|f| f.path.clone())
                .collect();
            // Hunks stay out only where the staged change left them out.
            self.line_picks.clear();
            self.unchecked_hunks.retain(|(path, key)| {
                keep.get(path.as_str())
                    .is_some_and(|hunks| hunks.is_some_and(|picked| !picked.contains(key)))
            });
        }
        cx.emit(ChecksChanged);
        self.focus_message(window, cx);
    }

    pub fn start_reword(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Commit(project, id)) = self.selection(cx) else {
            return;
        };
        let Some(message) = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_commit(id))
            .map(|(_, commit)| commit.message.clone())
        else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Commit message", true, cx);
            input.set_text(message, cx);
            input
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, input, event, window, cx| match event {
                InputEvent::Submit => {
                    let message = input.read(cx).text().trim().to_owned();
                    this.rewording = None;
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        store.run(
                            &project,
                            "Edit message",
                            Overlay::default(),
                            move |path, remap| backend::reword(path, remap.get(id)?, &message),
                            |store, rewritten, cx| {
                                store.follow(&rewritten, cx);
                                store.snack_undoable("Message updated", cx)
                            },
                            cx,
                        )
                    });
                    window.focus(&this.focus, cx);
                }
                InputEvent::Cancel => {
                    this.rewording = None;
                    window.focus(&this.focus, cx);
                    cx.notify();
                }
                InputEvent::Changed => {}
            },
        );
        window.focus(&input.focus_handle(cx), cx);
        self.rewording = Some((input, subscription));
        cx.notify();
    }

    pub(super) fn effective_target(&self, project: &ProjectId, cx: &App) -> Option<String> {
        let workspace = self.store.read(cx).workspace(project)?;
        if let Some(target) = &self.commit_target
            && workspace.find_branch(target).is_some()
        {
            return Some(target.clone());
        }
        workspace
            .stacks
            .first()
            .and_then(|stack| stack.branches.first())
            .filter(|branch| !branch.ref_name.is_empty())
            .map(|branch| branch.ref_name.clone())
    }

    pub(super) fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return;
        };
        if self.writing.is_some() {
            self.store.update(cx, |store, cx| {
                store.snack("The message is still being written", cx)
            });
            return;
        }
        let message = self.message.read(cx).text().trim().to_owned();
        if message.is_empty() {
            self.store.update(cx, |store, cx| {
                store.snack("Write a commit message first", cx)
            });
            self.focus_message(window, cx);
            return;
        }
        let diff_shown = self.diff.as_ref().is_some_and(|diff| {
            diff.key.source == DiffSource::Worktree && diff.key.project == project
        });
        if !self.unchecked_hunks.is_empty() && !diff_shown {
            // Hunk choices are only known against a loaded diff.
            self.store.update(cx, |store, cx| {
                store.snack("Still loading the diff, try again in a moment", cx)
            });
            return;
        }
        // Files a pending commit or discard already takes, as after a quick second click.
        let store = self.store.read(cx);
        let files: Vec<Selected> = self
            .selected_changes(cx)
            .into_iter()
            .filter(|s| !store.hides_file(&project, &s.file.path))
            .collect();
        if files.is_empty() {
            self.store
                .update(cx, |store, cx| store.snack("Nothing checked to commit", cx));
            return;
        }
        let target = self.effective_target(&project, cx);
        let message_input = self.message.clone();
        let detail = cx.entity().downgrade();
        // The files leave the list and the commit shows on its branch right away.
        let overlay = Overlay {
            new_commit: target.clone().map(|branch| {
                let title = message.lines().next().unwrap_or_default().to_owned();
                (branch, title.into())
            }),
            ..Overlay::files(&files)
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Commit",
                overlay,
                move |path, _| {
                    let branch = match target {
                        Some(branch) => branch,
                        None => backend::create_branch(path, None)?,
                    };
                    let id = backend::commit(
                        path,
                        &Destination::Branch(branch.clone()),
                        &files,
                        &message,
                    )?;
                    Ok((branch, id))
                },
                move |store, (branch, (_id, left_out)), cx| {
                    message_input.update(cx, |input, cx| input.set_text("", cx));
                    detail
                        .update(cx, |detail, cx| detail.close_composer(cx))
                        .ok();
                    store.report_left_out(
                        &format!("Committed to {}", short_ref(&branch)),
                        &left_out,
                        cx,
                    );
                },
                cx,
            )
        });
    }

    /// The newest commit on the branch new commits go to, which Amend adds to.
    fn amend_target(&self, project: &ProjectId, cx: &App) -> Option<(CommitId, String)> {
        let target = self.effective_target(project, cx)?;
        let workspace = self.store.read(cx).workspace(project)?;
        let top = workspace.find_branch(&target)?.commits.first()?;
        Some((top.id, top.title.clone()))
    }

    /// Add the checked changes to the newest commit of the target branch, keeping its
    /// message.
    pub(super) fn amend_top(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return;
        };
        let Some((into, _)) = self.amend_target(&project, cx) else {
            return;
        };
        let files = self.selected_changes(cx);
        if files.is_empty() {
            self.store
                .update(cx, |store, cx| store.snack("Nothing checked to amend", cx));
            return;
        }
        self.store.update(cx, |store, cx| {
            Transfer::Amend {
                into,
                files: files.into(),
            }
            .perform(&project, store, cx);
        });
        self.close_composer(cx);
        window.focus(&self.focus, cx);
    }

    /// The checked changes as a unified diff, for the AI to read.
    fn checked_diff_text(&self) -> String {
        let Some(diff) = self
            .diff
            .as_ref()
            .filter(|d| d.key.source == DiffSource::Worktree)
        else {
            return String::new();
        };
        let mut text = String::new();
        for file in diff.files.iter() {
            let path = &file.file.path;
            if self.file_check(path) == Toggled::False {
                continue;
            }
            text.push_str(&format!("File: {path} ({})\n", file.file.kind.word()));
            let DiffBody::Hunks { hunks, .. } = &file.body else {
                text.push_str("Binary or too large to show.\n");
                continue;
            };
            for hunk in hunks.iter().filter(|h| self.hunk_checked(path, h.key)) {
                text.push_str(&hunk.header);
                text.push('\n');
                for (ix, line) in hunk.lines.iter().enumerate() {
                    // A line left out reads as it stays: a removal kept, an addition absent.
                    let picked = self.line_checked(path, hunk.key, ix);
                    text.push(match line.kind {
                        LineKind::Added if picked => '+',
                        LineKind::Added => continue,
                        LineKind::Removed if picked => '-',
                        LineKind::Removed | LineKind::Context => ' ',
                    });
                    text.push_str(&line.text);
                    text.push('\n');
                }
            }
            text.push('\n');
        }
        text
    }

    /// Write the commit message with AI, streaming it into the box. What was in the box
    /// guides it.
    pub(super) fn write_message(&mut self, cx: &mut Context<Self>) {
        let diff = self.checked_diff_text();
        if diff.trim().is_empty() || self.writing.is_some() {
            return;
        }
        let notes = self.message.read(cx).text().to_owned();
        let prompt = ai::commit_prompt(&notes, &diff);
        let input = self.message.clone();
        let task = cx.spawn(async move |this, cx| {
            let written = ai::write(ai::COMMIT_SYSTEM, prompt, |text| {
                let shown = text.trim_start().to_owned();
                input.update(cx, |input, cx| input.set_text(shown, cx));
            })
            .await;
            this.update(cx, |this, cx| {
                this.writing = None;
                match written {
                    Ok(message) => {
                        let message = backend::wrapped_message(message.trim());
                        this.message
                            .update(cx, |input, cx| input.set_text(message, cx));
                    }
                    Err(err) => {
                        this.message
                            .update(cx, |input, cx| input.set_text(notes, cx));
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't write the message", format!("{err:#}"), cx)
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.writing = Some(task);
        cx.notify();
    }

    pub(super) fn toggle_checked(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.conflicted.contains(path) {
            let message = "Mark it resolved from its menu to commit it";
            self.store.update(cx, |store, cx| store.snack(message, cx));
            return;
        }
        let check = self.file_check(path);
        self.unchecked_hunks
            .retain(|(hunk_path, _)| hunk_path != path);
        self.line_picks
            .retain(|(hunk_path, _), _| hunk_path != path);
        match check {
            Toggled::True | Toggled::Mixed => {
                self.unchecked.insert(path.to_owned());
            }
            Toggled::False => {
                self.unchecked.remove(path);
            }
        }
        cx.emit(ChecksChanged);
        cx.notify();
    }

    pub(super) fn render_composer(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            return div().into_any_element();
        };
        let checked = workspace
            .uncommitted
            .iter()
            .filter(|f| self.file_check(&f.path) != Toggled::False)
            .count();
        let target = self.effective_target(project, cx);
        let message_focused = self.message.focus_handle(cx).is_focused(window);
        let branches: Vec<(String, String)> = workspace
            .branches()
            .filter(|b| !b.ref_name.is_empty())
            .map(|b| (b.ref_name.clone(), b.name.clone()))
            .collect();
        v_flex()
            .flex_none()
            .p(px(12.))
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.border_3)
            .bg(theme.bg_1)
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child("Commit to")
                    .child(self.render_target_picker(project.clone(), target, branches, theme, cx)),
            )
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .id("message-box")
                            .min_h(px(84.))
                            .max_h(px(260.))
                            .overflow_y_scroll()
                            .pl(px(10.))
                            // Clear of the AI button in the corner.
                            .pr(px(30.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(if message_focused {
                                theme.pop
                            } else {
                                theme.border_2
                            })
                            .bg(theme.bg_1)
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .cursor_text()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.focus_message(window, cx)),
                            )
                            .child(self.message.clone()),
                    )
                    .child(
                        div().absolute().top(px(4.)).right(px(4.)).child(
                            icon_button("write-message", Icon::Ai)
                                .small()
                                .disabled(self.writing.is_some() || checked == 0)
                                .tooltip(if self.writing.is_some() {
                                    "Writing…"
                                } else {
                                    "Write the message with AI, guided by what's in the box"
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.write_message(cx))),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(if checked == 0 {
                                "Check files to commit".to_owned()
                            } else {
                                format!("{checked} of {} files", workspace.uncommitted.len())
                            }),
                    )
                    .child(
                        button("close-composer", "Cancel")
                            .small()
                            .style(ButtonStyle::Ghost)
                            .tooltip_with_key("Close without committing", Some("esc".into()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_composer(cx);
                                window.focus(&this.focus, cx);
                            })),
                    )
                    .when_some(self.amend_target(project, cx), |el, (_, title)| {
                        el.child(
                            button("amend", "Amend")
                                .small()
                                .style(ButtonStyle::Outline)
                                .disabled(checked == 0)
                                .tooltip_with_key(
                                    format!("Add the checked changes to \"{title}\""),
                                    shortcut(&super::AmendTop, cx),
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.amend_top(window, cx)),
                                ),
                        )
                    })
                    .child(
                        button("commit", "Commit")
                            .small()
                            .style(ButtonStyle::Pop)
                            .disabled(checked == 0)
                            .tooltip_with_key("Commit the checked changes", Some("⌘↩".into()))
                            .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_target_picker(
        &self,
        project: ProjectId,
        target: Option<String>,
        branches: Vec<(String, String)>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let label = target
            .as_deref()
            .map(short_ref)
            .map(str::to_owned)
            .unwrap_or_else(|| "a new branch".to_owned());
        let detail = cx.entity().downgrade();
        let app = self.app.clone();
        h_flex()
            .id("commit-target")
            .h(px(22.))
            .px(px(8.))
            .gap(px(4.))
            .rounded(px(4.))
            .bg(theme.fill_gray.opacity(0.08))
            .hover(|el| el.bg(theme.fill_gray.opacity(0.14)))
            .font_family(theme.mono_font.clone())
            .text_color(theme.blue)
            .child(small_icon(Icon::Branch).text_color(theme.blue))
            .child(label)
            .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
            .on_click(move |event, _, cx| {
                let mut items: Vec<MenuItem> = branches
                    .iter()
                    .map(|(ref_name, name)| {
                        let detail = detail.clone();
                        let ref_name = ref_name.clone();
                        MenuItem::new(name.clone(), move |_, _, cx| {
                            detail
                                .update(cx, |detail, cx| {
                                    detail.commit_target = Some(ref_name.clone());
                                    cx.notify();
                                })
                                .ok();
                        })
                    })
                    .collect();
                let project = project.clone();
                items.push(MenuItem::new("New branch…", move |this, window, cx| {
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        store.select(Some(Selection::Uncommitted(project)), cx)
                    });
                    this.new_branch(&crate::app::NewBranch, window, cx);
                }));
                let position = event.position();
                app.update(cx, |app, cx| app.open_menu(position, items, cx))
                    .ok();
            })
    }
}

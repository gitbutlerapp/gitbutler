//! The file lists beside the diff: uncommitted files, or the loaded diff's, flat or as a
//! tree, with what right-clicking them offers.

use gpui::{
    AnyElement, ClickEvent, Context, Div, ElementId, MouseButton, MouseDownEvent, Pixels, Point,
    SharedString, Toggled, div, prelude::*, px,
};

use super::file_tree::{self, TreeRow};
use super::panes::side_list;
use super::{Detail, FilterFiles, keeps_path};
use crate::app::AppView;
use crate::backend::{self, FileChange, FileDiff, Selected};
use crate::overlays::MenuItem;
use crate::state::{Cut, Overlay, ProjectId, Review, Selection};
use crate::theme::Theme;
use crate::ui::diff;
use crate::ui::{
    Icon, checkbox, dot, faint, h_flex, icon_button, list_row, muted, plural, relative_time,
    shortcut, small_icon, v_flex,
};

/// Where a file list's files come from.
#[derive(Clone)]
enum FileSource {
    Uncommitted(ProjectId),
    /// The loaded diff's files: a commit's, or a branch's with no commit.
    Diff(ProjectId, Option<backend::CommitId>),
}

/// What a file row shows besides the file itself.
#[derive(Default)]
struct FileRow {
    /// Where the file is in the loaded diff: a click scrolls there, and the diff's cursor
    /// there marks the row.
    diff_ix: Option<usize>,
    stats: Option<(u32, u32)>,
    /// Shown while composing a commit.
    checkbox: Option<Toggled>,
    /// Shown for uncommitted files.
    review: Option<Review>,
    /// Lets the row be dragged and right-clicked as these changes.
    drag_from: Option<(ProjectId, Option<backend::CommitId>)>,
    /// How deep the file sits in a tree; it then shows only its name.
    depth: Option<usize>,
}

/// How far each level of a file tree steps in.
const TREE_INDENT: f32 = 14.;

impl Detail {
    /// The uncommitted `files` of `project`, under how many there are, their line counts
    /// and how many still want a look.
    pub(super) fn render_uncommitted_files(
        &self,
        project: &ProjectId,
        files: &[FileChange],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let rows = self.file_list(
            "uncommitted-files",
            FileSource::Uncommitted(project.clone()),
            cx,
        );
        let header = if self.filtering {
            self.filter_header(files.iter().map(|f| f.path.as_str()), theme, cx)
        } else {
            let (added, removed) = self.worktree_diff().map_or((0, 0), |d| {
                d.files
                    .iter()
                    .map(FileDiff::stats)
                    .fold((0, 0), |(a, r), (fa, fr)| (a + fa, r + fr))
            });
            let to_review = self.store.read(cx).unreviewed(project, files);
            h_flex()
                .h(px(36.))
                .px(px(16.))
                .gap(px(8.))
                .flex_none()
                .text_size(px(12.))
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(match files.len() {
                            0 => "No changes".to_owned(),
                            n => plural(n, "file"),
                        }),
                )
                .child(diff::stats(added, removed, theme))
                .child(div().flex_1())
                .child(self.filter_button(cx))
                .child(self.tree_toggle(cx))
                .when(!files.is_empty(), |el| {
                    el.child(
                        div()
                            .text_color(if to_review > 0 {
                                theme.warn
                            } else {
                                theme.text_3
                            })
                            .child(match to_review {
                                0 => "All reviewed".to_owned(),
                                n => format!("{n} to review"),
                            }),
                    )
                })
        };
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(header)
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(rows)
                    .when(files.is_empty(), |el| {
                        el.child(
                            muted("Your working directory matches the last commit", theme)
                                .px(px(16.))
                                .py(px(8.)),
                        )
                    }),
            )
            .into_any_element()
    }

    /// The files of the loaded commit or branch diff, each a jump target and draggable
    /// when it belongs to a single commit.
    pub(super) fn render_diff_file_list(
        &self,
        project: &ProjectId,
        commit: Option<backend::CommitId>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(diff) = self.shown_diff() else {
            let note = if self.loading.is_some() {
                "Loading…"
            } else {
                ""
            };
            return muted(note, theme)
                .px(px(16.))
                .py(px(10.))
                .into_any_element();
        };
        let rows = self.file_list("diff-files", FileSource::Diff(project.clone(), commit), cx);
        let header = if self.filtering {
            let paths = diff.files.iter().map(|f| f.file.path.as_str());
            self.filter_header(paths, theme, cx)
        } else {
            h_flex()
                .h(px(36.))
                .px(px(16.))
                .gap(px(4.))
                .flex_none()
                .text_size(px(12.))
                .child(
                    div()
                        .flex_1()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("{} changed", plural(diff.files.len(), "file"))),
                )
                .child(self.filter_button(cx))
                .child(self.tree_toggle(cx))
        };
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(header)
            .child(rows)
            .into_any_element()
    }

    /// A file list, flat or as a tree as the user chose, drawn only as far as it's on
    /// screen: an agent can leave thousands of files.
    fn file_list(&self, id: &'static str, source: FileSource, cx: &Context<Self>) -> Div {
        let store = self.store.read(cx);
        let tree = store.file_tree;
        let terms = self.filter_terms(cx);
        // Each shown file's position in its source, as files a pending operation takes away
        // aren't shown.
        let (index, paths): (Vec<usize>, Vec<&str>) = match &source {
            FileSource::Uncommitted(project) => store
                .workspace(project)
                .map(|ws| {
                    ws.uncommitted
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| {
                            !store.hides_file(project, &f.path) && keeps_path(&terms, &f.path)
                        })
                        .map(|(pos, f)| (pos, f.path.as_str()))
                        .unzip()
                })
                .unwrap_or_default(),
            FileSource::Diff(..) => self
                .diff
                .as_ref()
                .map(|diff| {
                    diff.files
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| keeps_path(&terms, &f.file.path))
                        .map(|(pos, f)| (pos, f.file.path.as_str()))
                        .unzip()
                })
                .unwrap_or_default(),
        };
        let rows: Vec<TreeRow> = if tree {
            file_tree::tree_rows(&paths, &self.collapsed_dirs)
        } else {
            (0..paths.len())
                .map(|pos| TreeRow::File { pos, depth: 0 })
                .collect()
        };
        let scroll = match &source {
            FileSource::Uncommitted(_) => &self.uncommitted_files,
            FileSource::Diff(..) => &self.diff_files,
        };
        side_list(
            id,
            rows.len(),
            scroll,
            "files-scrollbar",
            cx,
            move |this, ix, theme, cx| match rows.get(ix)? {
                dir @ TreeRow::Dir { .. } => this.render_dir_row(dir, &source, theme, cx),
                TreeRow::File { pos, depth } => {
                    let depth = tree.then_some(*depth);
                    this.render_source_file(&source, index[*pos], depth, theme, cx)
                }
            },
        )
    }

    /// The `pos`th file of `source`, with what its list shows beside it.
    fn render_source_file(
        &self,
        source: &FileSource,
        pos: usize,
        depth: Option<usize>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        match source {
            FileSource::Uncommitted(project) => {
                let store = self.store.read(cx);
                let file = store.workspace(project)?.uncommitted.get(pos)?;
                let in_diff = self.worktree_diff().and_then(|diff| {
                    let ix = diff.files.iter().position(|f| f.file.path == file.path)?;
                    Some((ix, diff.files[ix].stats()))
                });
                let extras = FileRow {
                    diff_ix: in_diff.map(|(ix, _)| ix),
                    stats: in_diff.map(|(_, stats)| stats),
                    checkbox: self.composing.then(|| self.checks.file_check(&file.path)),
                    review: Some(store.review(project, file)),
                    drag_from: Some((project.clone(), None)),
                    depth,
                };
                Some(self.render_file_row(pos, file, extras, theme, cx))
            }
            FileSource::Diff(project, commit) => {
                let diff = self.diff.as_ref()?;
                let file = diff.files.get(pos)?;
                // A linked worktree's files are reviewed too.
                let review = (diff.key.review_scope())
                    .map(|scope| self.store.read(cx).review(&scope, &file.file));
                let extras = FileRow {
                    diff_ix: Some(pos),
                    stats: Some(file.stats()),
                    review,
                    drag_from: commit.map(|id| (project.clone(), Some(id))),
                    depth,
                    ..Default::default()
                };
                Some(self.render_file_row(pos, &file.file, extras, theme, cx))
            }
        }
    }

    /// The files under the folder `dir` in `source`, to act on together.
    fn folder_payload(&self, source: &FileSource, dir: &str, cx: &gpui::App) -> Option<Cut> {
        let prefix = format!("{dir}/");
        // Only what the folder shows: not files the filter or a pending operation hides.
        let terms = self.filter_terms(cx);
        let under = |path: &str| path.starts_with(&prefix) && keeps_path(&terms, path);
        let store = self.store.read(cx);
        let (project, from, files): (_, _, Vec<FileChange>) = match source {
            FileSource::Uncommitted(project) => (
                project.clone(),
                None,
                (store.workspace(project)?.uncommitted.iter())
                    .filter(|f| under(&f.path) && !store.hides_file(project, &f.path))
                    .cloned()
                    .collect(),
            ),
            FileSource::Diff(project, Some(commit)) => (
                project.clone(),
                Some(*commit),
                self.diff
                    .as_ref()?
                    .files
                    .iter()
                    .map(|f| f.file.clone())
                    .filter(|f| under(&f.path))
                    .collect(),
            ),
            FileSource::Diff(_, None) => return None,
        };
        let files: Vec<Selected> = files
            .into_iter()
            .map(|file| Selected { file, hunks: None })
            .collect();
        (!files.is_empty()).then(|| Cut::Files {
            project,
            from,
            files: files.into(),
        })
    }

    fn render_dir_row(
        &self,
        row: &TreeRow,
        source: &FileSource,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let TreeRow::Dir {
            path,
            name,
            depth,
            collapsed,
            files,
        } = row.clone()
        else {
            return None;
        };
        let row = list_row(ElementId::Name(format!("dir-{path}").into()), false, theme)
            .pl(px(10. + TREE_INDENT * depth as f32))
            .gap(px(6.))
            .text_color(theme.text_2)
            .child(
                small_icon(if collapsed {
                    Icon::ChevronRight
                } else {
                    Icon::ChevronDown
                })
                .text_color(theme.text_3),
            )
            .child(small_icon(Icon::Folder).text_color(theme.text_3))
            .child(div().flex_1().min_w(px(0.)).truncate().child(name))
            .when(collapsed, |el| el.child(faint(files.to_string(), theme)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let (source, path) = (source.clone(), path.clone());
                    move |this, event: &MouseDownEvent, _, cx| {
                        if let Some(payload) = this.folder_payload(&source, &path, cx) {
                            // A folder: no one file to open.
                            this.changes_menu(payload, None, event.position, cx)
                        }
                    }
                }),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.collapsed_dirs.remove(&path) {
                    this.collapsed_dirs.insert(path.clone());
                }
                cx.notify();
            }));
        Some(row.into_any_element())
    }

    /// A file list's header while the file filter is open: the filter, how many of the
    /// list's `paths` it keeps, and a way to close it.
    fn filter_header<'a>(
        &self,
        paths: impl Iterator<Item = &'a str>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let terms = self.filter_terms(cx);
        let (kept, total) = paths.fold((0, 0), |(kept, total), path| {
            (kept + usize::from(keeps_path(&terms, path)), total + 1)
        });
        h_flex()
            .h(px(36.))
            .pl(px(16.))
            .pr(px(8.))
            .gap(px(8.))
            .flex_none()
            .text_size(px(12.))
            .child(small_icon(Icon::Search).text_color(theme.text_3))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_size(px(13.))
                    .child(self.file_filter.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_color(theme.text_3)
                    .child(format!("{kept} of {total}")),
            )
            .child(
                icon_button("close-filter", Icon::Cross)
                    .small()
                    .tooltip("Close the filter")
                    .on_click(cx.listener(|this, _, window, cx| this.close_filter(window, cx))),
            )
    }

    fn filter_button(&self, cx: &Context<Self>) -> impl IntoElement {
        icon_button("filter-files", Icon::Search)
            .small()
            .tooltip_with_key("Filter files", shortcut(&FilterFiles, cx))
            .on_click(cx.listener(|this, _, window, cx| this.open_filter(window, cx)))
    }

    fn tree_toggle(&self, cx: &Context<Self>) -> impl IntoElement {
        let tree = self.store.read(cx).file_tree;
        let store = self.store.clone();
        icon_button(
            "file-tree",
            if tree { Icon::List } else { Icon::FolderTree },
        )
        .small()
        .tooltip(if tree { "Show as list" } else { "Show as tree" })
        .on_click(move |_, _, cx| {
            store.update(cx, |store, cx| store.set_file_tree(!tree, cx));
        })
    }

    fn render_file_row(
        &self,
        ix: usize,
        file: &FileChange,
        extras: FileRow,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (name, dir) = diff::split_path(&file.path);
        let path = file.path.clone();
        let diff_ix = extras.diff_ix;
        let current = diff_ix == Some(self.cursor.file);
        let reviewed = extras.review == Some(Review::Reviewed);
        // In a tree a file lines up with its directory's name, and the path is the tree.
        let indent = extras
            .depth
            .map_or(0., |depth| TREE_INDENT * depth as f32 + 18.);
        let dir = if extras.depth.is_some() { "" } else { dir };
        let mut row = list_row(("file", ix), current, theme)
            .aria_label(file.path.clone())
            .aria_description(file.kind.word())
            .pl(px(10. + indent))
            .when_some(extras.checkbox, |el, check| {
                let path = path.clone();
                el.child(
                    checkbox(("check", ix), check, theme)
                        // Checking a file doesn't scroll to it.
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.toggle_checked(&path, cx)
                        })),
                )
            })
            .child(if file.conflicted {
                small_icon(Icon::Warning)
                    .size(px(10.))
                    .text_color(theme.danger)
                    .into_any_element()
            } else {
                div()
                    .flex_none()
                    .w(px(10.))
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(diff::change_color(file.kind, theme))
                    .child(file.kind.letter())
                    .into_any_element()
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(6.))
                    .overflow_hidden()
                    // Reviewed files step back, as in Lite's file list.
                    .when(reviewed, |el| el.opacity(0.5))
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(160.))
                            .truncate()
                            .child(name.to_owned()),
                    )
                    .child(
                        div()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(12.))
                            .text_color(theme.text_3)
                            .child(dir.to_owned()),
                    ),
            )
            .when_some(extras.stats, |el, (added, removed)| {
                el.child(diff::stats(added, removed, theme))
            })
            // A conflicted file says so where the others say when they changed.
            .children(
                match (file.conflicted, file.modified) {
                    (true, _) => Some((theme.danger, "conflicted".to_owned())),
                    (false, Some(ms)) => Some((theme.text_3, relative_time(ms / 1000))),
                    (false, None) => None,
                }
                .map(|(color, text)| {
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(color)
                        .child(text)
                }),
            )
            .when_some(extras.review, |el, review| {
                el.child(match review {
                    Review::Reviewed => small_icon(Icon::Tick)
                        .text_color(theme.pop)
                        .into_any_element(),
                    Review::Changed => dot(theme.warn).mx(px(3.)).into_any_element(),
                    Review::NotReviewed => div().flex_none().w(px(12.)).into_any_element(),
                })
            })
            .when_some(diff_ix, |el, diff_ix| {
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        this.scroll_to_file(diff_ix, cx)
                    }),
                )
            });
        if let Some((project, from)) = extras.drag_from {
            let payload = |files: Vec<Selected>| Cut::Files {
                project: project.clone(),
                from,
                files: files.into(),
            };
            let whole = vec![Selected {
                file: file.clone(),
                hunks: None,
            }];
            // A file partly checked drags as its checked hunks and lines. Not at all while
            // they're unknown, rather than as all of it.
            let dragged = match extras.checkbox {
                Some(Toggled::Mixed) => (self.selected_changes(cx).into_iter())
                    .filter(|s| s.file.path == file.path)
                    .collect(),
                _ => whole.clone(),
            };
            if !dragged.is_empty() {
                let label: SharedString = file.path.clone().into();
                row = row.on_drag(
                    payload(dragged),
                    crate::transfer::drag_preview(self.store.clone(), label),
                );
            }
            let menu_payload = payload(whole);
            row = row.on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.changes_menu(menu_payload.clone(), Some(None), event.position, cx)
                }),
            );
        }
        row.into_any_element()
    }

    /// What right-clicking files, a folder or a hunk offers: moving them, taking them out,
    /// and for a single file (`line` where an editor should open it) opening or copying
    /// its path; a folder's menu has no `line`.
    pub(super) fn changes_menu(
        &mut self,
        payload: Cut,
        line: Option<Option<u32>>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Cut::Files {
            project,
            from,
            files,
        } = payload.clone()
        else {
            return;
        };
        let hunk = files.iter().any(|f| f.hunks.is_some());
        let what = match (hunk, files.len()) {
            (true, _) => "hunk",
            (false, 1) => "file",
            (false, _) => "files",
        };
        let cut = payload.clone();
        let mut items = vec![MenuItem::new("Cut", move |this: &mut AppView, _, cx| {
            this.cut(cut.clone(), cx)
        })];
        match from {
            None => {
                let conflicted: Vec<String> = (files.iter())
                    .filter(|f| f.file.conflicted)
                    .map(|f| f.file.path.clone())
                    .collect();
                if !conflicted.is_empty() {
                    let project = project.clone();
                    items.insert(
                        0,
                        MenuItem::new("Mark resolved", move |this, _, cx| {
                            let files = conflicted.clone();
                            this.store.update(cx, |store, cx| {
                                store.run(
                                    &project,
                                    "Mark resolved",
                                    Overlay::default(),
                                    move |path, _| backend::mark_resolved(path, files),
                                    |store, (), cx| store.snack_undoable("Marked resolved", cx),
                                    cx,
                                )
                            })
                        }),
                    );
                }
                let (review_project, review_files) = (project.clone(), files.clone());
                let (project, files) = (project.clone(), files.clone());
                items.extend([
                    MenuItem::new("Mark reviewed", move |this, _, cx| {
                        let changes: Vec<FileChange> =
                            review_files.iter().map(|f| f.file.clone()).collect();
                        this.store.update(cx, |store, cx| {
                            store.set_reviewed(&review_project, &changes, true, cx)
                        })
                    }),
                    MenuItem::new("Discard changes…", move |this, window, cx| {
                        this.discard(project.clone(), files.to_vec(), window, cx)
                    })
                    .danger(),
                ]);
            }
            Some(commit) => {
                let uncommitted = Selection::Uncommitted(project.clone());
                let (split_project, split_files) = (project.clone(), files.clone());
                let (drop_project, drop_files) = (project.clone(), files.clone());
                items.extend([
                    MenuItem::new(format!("Uncommit {what}"), move |this, window, cx| {
                        this.transfer(&payload, &uncommitted, Default::default(), window, cx);
                    }),
                    MenuItem::new("Split into a new commit", move |this, _, cx| {
                        this.split_off(split_project.clone(), commit, split_files.clone(), cx)
                    }),
                    MenuItem::new(
                        format!("Discard {what} from commit…"),
                        move |this, window, cx| {
                            this.discard_from_commit(
                                drop_project.clone(),
                                commit,
                                drop_files.clone(),
                                window,
                                cx,
                            )
                        },
                    )
                    .danger(),
                ]);
            }
        }
        if let [file] = &*files
            && let Some(line) = line
            && let Some(root) = self.store.read(cx).path(&project)
        {
            let relative = file.file.path.clone();
            let absolute = root.join(&relative);
            let full = absolute.display().to_string();
            let (open_project, open_file) = (project.clone(), relative.clone());
            let label = crate::open_in::editor_label(self.store.read(cx));
            items.extend([
                MenuItem::new(label, move |this, _, cx| {
                    let file = Some(open_file.clone());
                    this.open_in_editor(&open_project, None, file, line, position, cx)
                })
                .separated(),
                MenuItem::new("Copy path", move |this, _, cx| {
                    this.copy_text(relative.clone(), cx)
                }),
                MenuItem::new("Copy full path", move |this, _, cx| {
                    this.copy_text(full.clone(), cx)
                }),
                MenuItem::new("Reveal in Finder", move |_, _, cx| {
                    cx.reveal_path(&absolute)
                }),
            ]);
        }
        self.app
            .update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }
}

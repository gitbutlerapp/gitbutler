//! The conflict resolver: every conflict of a commit, its current and incoming sides next
//! to each other, and a pick for each. Picks apply together; unpicked conflicts stay.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    SharedString, Window, actions, div, prelude::*, px,
};

use crate::backend::conflicts::{self, ConflictHunk, Conflicts, Pick};
use crate::backend::{self, CommitId};
use crate::state::{Overlay, ProjectId, Store};
use crate::theme::Theme;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::{ButtonStyle, button, h_flex, plural, v_flex};

actions!(resolve, [ResolveConflicts]);

/// Lines of unconflicted context shown around each conflict.
const CONTEXT_LINES: usize = 3;

pub struct ResolveView {
    store: Entity<Store>,
    project: ProjectId,
    commit: CommitId,
    title: SharedString,
    focus: FocusHandle,
    conflicts: Option<Result<Conflicts, SharedString>>,
    /// Picks by file and 0-based hunk.
    picks: HashMap<(String, usize), Pick>,
}

impl EventEmitter<DismissEvent> for ResolveView {}

impl Focusable for ResolveView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ResolveView {
    pub fn new(
        store: Entity<Store>,
        project: ProjectId,
        commit: CommitId,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_commit(commit))
            .map(|(_, c)| c.title.clone())
            .unwrap_or_default();
        if let Some(path) = store.read(cx).path(&project) {
            cx.spawn(async move |this, cx| {
                let found = backend::run(move || conflicts::conflicts(&path, commit)).await;
                this.update(cx, |this, cx| {
                    this.conflicts = Some(found.map_err(|err| format!("{err:#}").into()));
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        Self {
            store,
            project,
            commit,
            title: title.into(),
            focus: cx.focus_handle(),
            conflicts: None,
            picks: HashMap::new(),
        }
    }

    fn total(&self) -> usize {
        match &self.conflicts {
            Some(Ok(found)) => found.files.iter().map(|f| f.hunks.len()).sum(),
            _ => 0,
        }
    }

    fn pick_all(&mut self, pick: Pick, cx: &mut Context<Self>) {
        if let Some(Ok(found)) = &self.conflicts {
            for file in &found.files {
                for ix in 0..file.hunks.len() {
                    self.picks.insert((file.path.clone(), ix), pick);
                }
            }
        }
        cx.notify();
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(found)) = self.conflicts.clone() else {
            return;
        };
        let picks: Vec<(String, usize, Pick)> = self
            .picks
            .iter()
            .map(|((path, ix), pick)| (path.clone(), *ix, *pick))
            .collect();
        if picks.is_empty() {
            return;
        }
        let commit = self.commit;
        self.store.update(cx, |store, cx| {
            store.run(
                &self.project,
                "Resolve",
                Overlay::default(),
                move |path, remap| conflicts::resolve(path, remap.get(commit)?, &picks, &found),
                |store, (rewritten, outcome), cx| {
                    store.follow(&rewritten, cx);
                    let message = match (outcome.remaining, outcome.emptied) {
                        (0, true) => "Resolved. Nothing of the commit's change is left".to_owned(),
                        (0, false) => "Resolved".to_owned(),
                        (left, _) => format!(
                            "Resolved {}, {left} left",
                            plural(outcome.resolved, "conflict")
                        ),
                    };
                    store.snack_undoable(message, cx)
                },
                cx,
            )
        });
        cx.emit(DismissEvent);
    }

    fn render_hunk(
        &self,
        path: &str,
        ix: usize,
        hunk: &ConflictHunk,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let picked = self.picks.get(&(path.to_owned(), ix)).copied();
        let context = |text: &str, last: bool| {
            let lines: Vec<&str> = text.lines().collect();
            let shown = if last {
                &lines[lines.len().saturating_sub(CONTEXT_LINES)..]
            } else {
                &lines[..lines.len().min(CONTEXT_LINES)]
            };
            (!shown.is_empty()).then(|| {
                div()
                    .px(px(10.))
                    .py(px(4.))
                    .text_color(theme.text_3)
                    .child(shown.join("\n"))
            })
        };
        let side = |label: &'static str, text: &str, pick: Pick| {
            let chosen = matches!((picked, pick), (Some(p), _) if p == pick || p == Pick::Both);
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .rounded(px(4.))
                .border_1()
                .border_color(if chosen { theme.pop } else { theme.border_3 })
                .overflow_hidden()
                .child(
                    div()
                        .px(px(10.))
                        .py(px(4.))
                        .bg(theme.fill_gray.opacity(0.06))
                        .font_family(theme.ui_font.clone())
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(label),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("{path}-{ix}-{label}")))
                        .max_h(px(220.))
                        .overflow_y_scroll()
                        .px(px(10.))
                        .py(px(6.))
                        .child(if text.is_empty() {
                            "(nothing)".to_owned()
                        } else {
                            text.trim_end_matches('\n').to_owned()
                        }),
                )
        };
        let choice = |pick: Pick, label: &'static str| {
            let path = path.to_owned();
            button(
                SharedString::from(format!("{path}-{ix}-{label}-pick")),
                label,
            )
            .small()
            .style(if picked == Some(pick) {
                ButtonStyle::Gray
            } else {
                ButtonStyle::Ghost
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                let key = (path.clone(), ix);
                if this.picks.get(&key) == Some(&pick) {
                    this.picks.remove(&key);
                } else {
                    this.picks.insert(key, pick);
                }
                cx.notify();
            }))
        };
        v_flex()
            .gap(px(6.))
            .font_family(theme.mono_font.clone())
            .text_size(px(12.))
            .line_height(px(18.))
            .children(context(&hunk.before, true))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(8.))
                    .child(side("Current", &hunk.current, Pick::Current))
                    .child(side("Incoming", &hunk.incoming, Pick::Incoming)),
            )
            .children(context(&hunk.after, false))
            .child(
                h_flex()
                    .gap(px(4.))
                    .font_family(theme.ui_font.clone())
                    .child(choice(Pick::Current, "Accept current"))
                    .child(choice(Pick::Incoming, "Accept incoming"))
                    .child(choice(Pick::Both, "Accept both")),
            )
            .into_any_element()
    }
}

impl Render for ResolveView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let total = self.total();
        let picked = self.picks.len();
        let body: AnyElement =
            match &self.conflicts {
                None => div()
                    .p(px(24.))
                    .text_color(theme.text_2)
                    .child("Loading the conflicts…")
                    .into_any_element(),
                Some(Err(err)) => div()
                    .p(px(24.))
                    .text_color(theme.danger)
                    .child(err.clone())
                    .into_any_element(),
                Some(Ok(found)) => v_flex()
                    .gap(px(20.))
                    .children(found.files.iter().map(|file| {
                        v_flex()
                            .gap(px(10.))
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .font_family(theme.mono_font.clone())
                                            .text_size(px(13.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(file.path.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.text_2)
                                            .child(plural(file.hunks.len(), "conflict")),
                                    ),
                            )
                            .children(file.hunks.iter().enumerate().map(|(ix, hunk)| {
                                self.render_hunk(&file.path, ix, hunk, &theme, cx)
                            }))
                    }))
                    .when(!found.manual.is_empty(), |el| {
                        el.child(
                            v_flex()
                                .gap(px(4.))
                                .text_size(px(12.))
                                .child(
                                    div()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child("Needs editing by hand"),
                                )
                                .children(found.manual.iter().map(|(path, reason)| {
                                    div()
                                        .text_color(theme.text_2)
                                        .child(format!("{path}: {reason}"))
                                })),
                        )
                    })
                    .into_any_element(),
            };
        modal(ModalSize::Large, &theme)
            .id("resolve")
            .track_focus(&self.focus)
            .h(px(640.))
            .child(
                v_flex()
                    .flex_none()
                    .gap(px(4.))
                    .px(px(20.))
                    .pt(px(18.))
                    .pb(px(14.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_size(px(15.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(format!("Resolve conflicts in \"{}\"", self.title)),
                            )
                            .when(total > 1, |el| {
                                el.child(
                                    button("all-current", "All current")
                                        .small()
                                        .style(ButtonStyle::Ghost)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.pick_all(Pick::Current, cx)
                                        })),
                                )
                                .child(
                                    button("all-incoming", "All incoming")
                                        .small()
                                        .style(ButtonStyle::Ghost)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.pick_all(Pick::Incoming, cx)
                                        })),
                                )
                            }),
                    )
                    .child(div().text_size(px(12.)).text_color(theme.text_2).child(
                        "Current is what the commit now sits on; incoming is the \
                                 commit's own change. Conflicts you don't pick stay for later.",
                    )),
            )
            .child(
                div()
                    .id("resolve-body")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(20.))
                    .child(body),
            )
            .child(
                h_flex()
                    .flex_none()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(20.))
                    .py(px(12.))
                    .border_t_1()
                    .border_color(theme.border_3)
                    .child(
                        button("resolve-cancel", "Cancel")
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        button(
                            "resolve-apply",
                            if picked == total || picked == 0 {
                                "Resolve".to_owned()
                            } else {
                                format!("Resolve {picked} of {total}")
                            },
                        )
                        .style(ButtonStyle::Gray)
                        .disabled(picked == 0)
                        .on_click(cx.listener(|this, _, _, cx| this.apply(cx))),
                    ),
            )
    }
}

impl crate::app::AppView {
    /// Open the resolver for the selected commit, when it's conflicted.
    pub fn resolve_conflicts(
        &mut self,
        _: &ResolveConflicts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(crate::state::Selection::Commit(project, commit)) =
            self.store.read(cx).selection.clone()
        else {
            return;
        };
        let store = self.store.clone();
        let view = cx.new(|cx| ResolveView::new(store, project, commit, cx));
        self.open_modal(view, window, cx);
    }
}
